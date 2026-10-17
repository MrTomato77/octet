use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, Deserialize)]
pub struct State {
    pub version: u32,
    pub thai: bool,
    pub dark: bool,
    pub size: [f32; 2],
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 2,
            thai: false,
            dark: false,
            size: [700.0, 470.0],
        }
    }
}

fn decode(bytes: &[u8]) -> Result<State, String> {
    let mut state: State = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    // Version 1 also held profiles; its preferences still load and the profiles are dropped.
    if !(1..=2).contains(&state.version) {
        return Err("Unsupported storage version".into());
    }
    if !state.size.iter().all(|v| v.is_finite() && *v > 0.0) {
        state.size = [700.0, 470.0];
    }
    state.size[0] = state.size[0].clamp(700.0, 3840.0);
    state.size[1] = state.size[1].clamp(470.0, 2160.0);
    Ok(state)
}

pub struct Store {
    pub state: State,
    pub error: Option<String>,
    pub path: PathBuf,
    blocked: bool,
    last: Vec<u8>,
}

impl Store {
    pub fn open_default() -> Self {
        let path = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("SimpleIPv2")
            .join("state.json");
        Self::open(path)
    }

    pub fn open(path: PathBuf) -> Self {
        let read = match std::fs::read(&path) {
            Ok(bytes) => decode(&bytes).map(|state| (state, bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if path.with_extension("json.bak").exists() {
                    std::fs::read(path.with_extension("json.bak"))
                        .map_err(|e| e.to_string())
                        .and_then(|bytes| decode(&bytes).map(|state| (state, Vec::new())))
                } else {
                    Ok((State::default(), vec![]))
                }
            }
            Err(e) => Err(e.to_string()),
        };
        match read {
            Ok((state, last)) => Self {
                state,
                path,
                last,
                error: None,
                blocked: false,
            },
            Err(error) => Self {
                state: State::default(),
                path,
                last: vec![],
                error: Some(error),
                blocked: true,
            },
        }
    }

    pub fn write(&mut self, state: State) -> Result<(), String> {
        if self.blocked {
            return Err("Existing storage was not loaded; original file is preserved".into());
        }
        let bytes = serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?;
        if bytes == self.last {
            return Ok(());
        }
        if self.last.is_empty() && self.path.exists() {
            self.error =
                Some("Another window created storage. Reopen this window before saving.".into());
            return Err(self.error.clone().unwrap());
        }
        if !self.last.is_empty()
            && let Ok(current) = std::fs::read(&self.path)
            && current != self.last
        {
            self.error =
                Some("Storage changed outside this window. Reopen the app before saving.".into());
            return Err(self.error.clone().unwrap());
        }
        let result = replace(&self.path, &bytes);
        if let Err(error) = &result {
            self.error = Some(error.clone());
        }
        if result.is_ok() {
            self.last = bytes;
            self.state = state;
            self.error = None;
        }
        result
    }

    pub fn recover_backup(&mut self) -> Result<(), String> {
        let bytes =
            std::fs::read(self.path.with_extension("json.bak")).map_err(|e| e.to_string())?;
        let state = decode(&bytes)?;
        if self.path.exists() {
            let suffix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos();
            std::fs::rename(
                &self.path,
                self.path.with_extension(format!("json.corrupt-{suffix}")),
            )
            .map_err(|e| e.to_string())?;
        }
        self.blocked = false;
        self.last.clear();
        self.write(state)
    }
}

fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    std::fs::create_dir_all(path.parent().ok_or("Missing storage directory")?)
        .map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    let backup = path.with_extension("json.bak");
    let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    if path.exists() {
        std::fs::copy(path, &backup).map_err(|e| e.to_string())?;
    }
    // Windows rename cannot replace an existing destination. Keep the backup first.
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    if let Err(e) = std::fs::rename(&temp, path) {
        if backup.exists() {
            let _ = std::fs::copy(&backup, path);
        }
        return Err(e.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_change_and_unsupported_schema_are_not_overwritten() {
        let path = path();
        let mut first = Store::open(path.clone());
        first.write(State::default()).unwrap();
        let mut other = Store::open(path.clone());
        first
            .write(State {
                thai: true,
                ..State::default()
            })
            .unwrap();
        assert!(
            other
                .write(State {
                    dark: true,
                    ..State::default()
                })
                .is_err()
        );
        let future = State {
            version: 100,
            ..State::default()
        };
        let bytes = serde_json::to_vec(&future).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let mut store = Store::open(path.clone());
        assert!(store.error.is_some());
        assert!(store.write(State::default()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn missing_primary_uses_backup_and_legacy_preferences_load() {
        let path = path();
        let mut store = Store::open(path.clone());
        store.write(State::default()).unwrap();
        store
            .write(State {
                thai: true,
                ..State::default()
            })
            .unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(Store::open(path.clone()).error.is_none());
        let legacy = br#"{"version":1,"profiles":[],"drafts":{},"selected":0,"thai":true,"dark":false,"size":[900.0,700.0]}"#;
        let state = decode(legacy).unwrap();
        assert!(state.thai);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    fn path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("simple-ip-test-{}", uuid_for_test()))
            .join("state.json")
    }
    fn uuid_for_test() -> String {
        // Tests run in parallel; a clock value alone can collide.
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("{}-{n}", std::process::id())
    }
    #[test]
    fn round_trip_preserves_preferences() {
        let path = path();
        let mut store = Store::open(path.clone());
        let state = State {
            thai: true,
            dark: true,
            size: [900.0, 700.0],
            ..State::default()
        };
        store.write(state.clone()).unwrap();
        let loaded = Store::open(path.clone());
        assert!(loaded.error.is_none());
        assert!(loaded.state.thai && loaded.state.dark);
        assert_eq!(loaded.state.size, state.size);
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn corrupt_file_is_preserved_until_explicit_backup_recovery() {
        let path = path();
        let mut store = Store::open(path.clone());
        store.write(State::default()).unwrap();
        store
            .write(State {
                thai: true,
                ..State::default()
            })
            .unwrap();
        std::fs::write(&path, b"broken").unwrap();
        let mut store = Store::open(path.clone());
        assert!(store.write(State::default()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        store.recover_backup().unwrap();
        assert!(Store::open(path.clone()).error.is_none());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
