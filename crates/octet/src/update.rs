//! Updates for a portable exe: the whole new Octet.exe from the latest GitHub release, checked
//! against our Ed25519 signature (made in CI) before it replaces this one. Nothing runs unsigned.

use crate::net::https_get;

/// The release key's public half. The private half signs every release in CI (OCTET_SIGNING_KEY).
const KEY: [u8; 32] = [
    0x6a, 0xc3, 0x26, 0xf6, 0x11, 0xa3, 0x86, 0xce, 0x65, 0x40, 0x34, 0xf0, 0xbe, 0x16, 0xc8, 0xd9,
    0xae, 0xfe, 0x39, 0x0d, 0x77, 0xcd, 0xf5, 0xc5, 0xfc, 0x0b, 0x68, 0xbd, 0x5b, 0xbd, 0x32, 0x98,
];
const REPO: &str = "MrTomato77/octet";

/// "0.1.10" > "0.1.9": numbers, not text.
fn newer(theirs: &str, ours: &str) -> bool {
    let n = |v: &str| v.trim_start_matches('v').split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    n(theirs) > n(ours)
}

/// The latest release's version when it is newer than this one.
pub fn check() -> Result<Option<String>, String> {
    let body = https_get("api.github.com", &format!("/repos/{REPO}/releases/latest")).ok_or("Could not reach GitHub")?;
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().ok_or("No release found")?;
    Ok(newer(tag, env!("CARGO_PKG_VERSION")).then(|| tag.trim_start_matches('v').to_string()))
}

fn verify(exe: &[u8], sig: &[u8]) -> bool {
    let (Ok(key), Ok(sig)) = (ed25519_compact::PublicKey::from_slice(&KEY), ed25519_compact::Signature::from_slice(sig)) else { return false };
    key.verify(exe, &sig).is_ok()
}

/// Downloads `version`, checks its signature, and puts it in this exe's place (this one becomes
/// Octet.exe.old, removed on the next start). The new one runs from the next start on.
pub fn install(version: &str, progress: impl FnMut(f32)) -> Result<(), String> {
    let path = format!("/{REPO}/releases/download/v{version}/Octet.exe");
    let sig = https_get("github.com", &format!("{path}.sig")).ok_or("Could not download the signature")?;
    let exe = crate::net::https_get_with("github.com", &path, progress).ok_or("Could not download the update")?;
    if !verify(&exe, &sig) {
        return Err("The download is not signed by Octet's key: not installed".into());
    }
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let (new, old) = (me.with_extension("exe.new"), me.with_extension("exe.old"));
    std::fs::write(&new, &exe).map_err(|e| format!("Could not save the update: {e}"))?;
    let _ = std::fs::remove_file(&old);
    // Windows lets a running exe be renamed, not overwritten.
    std::fs::rename(&me, &old).map_err(|e| format!("Could not replace Octet.exe: {e}"))?;
    if let Err(e) = std::fs::rename(&new, &me) {
        let _ = std::fs::rename(&old, &me);
        return Err(format!("Could not replace Octet.exe: {e}"));
    }
    Ok(())
}

/// Left over from the last update.
pub fn tidy() {
    if let Ok(me) = std::env::current_exe() {
        let _ = std::fs::remove_file(me.with_extension("exe.old"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(newer("v0.1.10", "0.1.9"));
        assert!(newer("0.2.0", "0.1.99"));
        assert!(!newer("v0.1.7", "0.1.7"));
        assert!(!newer("0.1.6", "0.1.7"));
    }

    #[test]
    fn only_our_signature_passes() {
        // "octet" signed with the release key (openssl pkeyutl -sign -rawin), as CI signs Octet.exe.
        let hex = "f54ae0ac4035a8dc998658a1c59f08cce77dffc575daae5c4fc72634d1858ec2ab56c182a5c031e516352232d695c5014becb10a0981215946efec00951b0007";
        let sig: Vec<u8> = (0..64).map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()).collect();
        assert!(verify(b"octet", &sig), "our key's signature passes");
        assert!(!verify(b"octets", &sig), "the same signature on other bytes fails");
        assert!(!verify(b"anything", &[0u8; 64]), "a zero signature fails");
        assert!(!verify(b"anything", &[1u8; 10]), "a short one fails");
    }
}
