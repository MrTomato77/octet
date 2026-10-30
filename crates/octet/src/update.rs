//! Updates for a portable exe: the whole new Octet.exe from the latest GitHub release, checked
//! against our Ed25519 signature (made in CI) before it replaces this one. Nothing runs unsigned.
//! On macOS the same, for the whole Octet.app (in the release's Octet.dmg).

use crate::net::https_get;

/// The release key's public half. The private half signs every release in CI (OCTET_SIGNING_KEY).
const KEY: [u8; 32] = [
    0x6a, 0xc3, 0x26, 0xf6, 0x11, 0xa3, 0x86, 0xce, 0x65, 0x40, 0x34, 0xf0, 0xbe, 0x16, 0xc8, 0xd9,
    0xae, 0xfe, 0x39, 0x0d, 0x77, 0xcd, 0xf5, 0xc5, 0xfc, 0x0b, 0x68, 0xbd, 0x5b, 0xbd, 0x32, 0x98,
];
const REPO: &str = "MrTomato77/octet";
/// This system's file in each release; its signatures are beside it (".sig", ".vsig").
#[cfg(target_os = "macos")]
const ASSET: &str = "Octet.dmg";
#[cfg(not(target_os = "macos"))]
const ASSET: &str = "Octet.exe";

/// "0.1.10" > "0.1.9": numbers, not text.
fn newer(theirs: &str, ours: &str) -> bool {
    let n = |v: &str| v.trim_start_matches('v').split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    n(theirs) > n(ours)
}

/// The latest release when it is newer than this one: its version, and the first few points of its notes.
pub fn check() -> Result<Option<serde_json::Value>, String> {
    let body = https_get("api.github.com", &format!("/repos/{REPO}/releases/latest")).ok_or("Could not reach GitHub")?;
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().ok_or("No release found")?;
    Ok(newer(tag, env!("CARGO_PKG_VERSION")).then(|| serde_json::json!({ "version": tag.trim_start_matches('v'), "notes": notes(v["body"].as_str().unwrap_or("")) })))
}

/// Up to three bullet points from release notes (Markdown), as plain text.
fn notes(md: &str) -> Vec<String> {
    md.lines().filter_map(|l| l.trim_start().strip_prefix("- ").or_else(|| l.trim_start().strip_prefix("* ")))
        .map(|l| l.replace("**", "").replace('`', "").trim().to_string()).filter(|l| !l.is_empty()).take(3).collect()
}

fn verify(key: &[u8; 32], msg: &[u8], sig: &[u8]) -> bool {
    let (Ok(key), Ok(sig)) = (ed25519_compact::PublicKey::from_slice(key), ed25519_compact::Signature::from_slice(sig)) else { return false };
    key.verify(msg, &sig).is_ok()
}

/// What the ".vsig" signs: `octet-update`, NUL, the version as `check` gives it (the tag without
/// its leading 'v', e.g. "0.2.0"), NUL, then the file's bytes. Binding the version means a release
/// can't be relabelled: an old signed file put up as a newer version fails, so the newer-only
/// check can trust the version. release.yml builds the same bytes before signing.
fn message(version: &str, file: &[u8]) -> Vec<u8> {
    [b"octet-update\0", version.as_bytes(), b"\0", file].concat()
}

/// Downloads `version`, checks its signature, and puts it in this copy's place. The new one runs
/// from the next start on.
pub fn install(version: &str, progress: impl FnMut(f32)) -> Result<(), String> {
    // Only a newer release, named as releases are: an older one is signed too, and installing it on request
    // would bring its old bugs back; and nothing but digits and dots goes into the download's path.
    if version.is_empty() || !version.chars().all(|c| c.is_ascii_digit() || c == '.') || !newer(version, env!("CARGO_PKG_VERSION")) {
        return Err("Not a newer version of Octet".into());
    }
    let path = format!("/{REPO}/releases/download/v{version}/{ASSET}");
    // Only the version-bound ".vsig", never the old ".sig" (the file alone, still published for
    // copies from before ".vsig"): a ".sig" would let an old signed file pass as this version, so a
    // release without a good ".vsig" is not installed, whatever else it offers.
    let sig = https_get("github.com", &format!("{path}.vsig")).ok_or("Could not download the signature")?;
    let file = crate::net::https_get_with("github.com", &path, progress).ok_or("Could not download the update")?;
    if !verify(&KEY, &message(version, &file), &sig) {
        return Err("The download is not signed by Octet's key: not installed".into());
    }
    place(&file)
}

/// The new exe takes this one's place; this one becomes Octet.exe.old, removed on the next start.
#[cfg(not(target_os = "macos"))]
fn place(exe: &[u8]) -> Result<(), String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    let (new, old) = (me.with_extension("exe.new"), me.with_extension("exe.old"));
    std::fs::write(&new, exe).map_err(|e| format!("Could not save the update: {e}"))?;
    let _ = std::fs::remove_file(&old);
    // Windows lets a running exe be renamed, not overwritten.
    std::fs::rename(&me, &old).map_err(|e| format!("Could not replace Octet.exe: {e}"))?;
    if let Err(e) = std::fs::rename(&new, &me) {
        let _ = std::fs::rename(&old, &me);
        return Err(format!("Could not replace Octet.exe: {e}"));
    }
    Ok(())
}

/// The Octet.app this runs from (…/Octet.app/Contents/MacOS/octet); None when run some other way.
#[cfg(target_os = "macos")]
fn bundle() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let app = exe.parent()?.parent()?.parent()?;
    (app.extension()? == "app").then(|| app.to_path_buf())
}

/// Copies Octet.app out of the disk image `dmg` into the folder `into`. True when it is all there.
#[cfg(target_os = "macos")]
fn unpack(dmg: &std::path::Path, into: &std::path::Path) -> bool {
    use std::process::Command;
    let mnt = into.join("mnt");
    let attached = Command::new("/usr/bin/hdiutil").args(["attach", "-quiet", "-nobrowse", "-readonly", "-noautoopen", "-mountpoint"])
        .arg(&mnt).arg(dmg).status().is_ok_and(|s| s.success());
    if !attached {
        return false;
    }
    let copied = Command::new("/usr/bin/ditto").arg(mnt.join("Octet.app")).arg(into.join("Octet.app")).status().is_ok_and(|s| s.success());
    let _ = Command::new("/usr/bin/hdiutil").args(["detach", "-quiet", "-force"]).arg(&mnt).status();
    copied && into.join("Octet.app/Contents/MacOS/octet").is_file()
}

/// The disk image holds Octet.app, which takes the place of the bundle this runs from; the old one
/// becomes Octet.app.old, removed on the next start. Copied out here, so it carries no quarantine flag.
#[cfg(target_os = "macos")]
fn place(dmg: &[u8]) -> Result<(), String> {
    use std::fs;
    let app = bundle().ok_or("Updates install into Octet.app, and this copy isn't one")?;
    let tmp = app.with_extension("app.new");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| format!("Could not save the update: {e}"))?;
    let archive = tmp.join(ASSET);
    fs::write(&archive, dmg).map_err(|e| format!("Could not save the update: {e}"))?;
    let new = tmp.join("Octet.app");
    if !unpack(&archive, &tmp) {
        let _ = fs::remove_dir_all(&tmp);
        return Err("The update is not a complete Octet.app: not installed".into());
    }
    let old = app.with_extension("app.old");
    let _ = fs::remove_dir_all(&old);
    fs::rename(&app, &old).map_err(|e| format!("Could not replace Octet.app: {e}"))?;
    if let Err(e) = fs::rename(&new, &app) {
        let _ = fs::rename(&old, &app);
        return Err(format!("Could not replace Octet.app: {e}"));
    }
    let _ = fs::remove_dir_all(&tmp);
    Ok(())
}

/// Left over from the last update.
pub fn tidy() {
    #[cfg(target_os = "macos")]
    if let Some(app) = bundle() {
        let _ = std::fs::remove_dir_all(app.with_extension("app.old"));
        let _ = std::fs::remove_dir_all(app.with_extension("app.new"));
    }
    #[cfg(not(target_os = "macos"))]
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
        assert_eq!(super::notes("## Changes\n- **VPN:** works\n  * `Scan` faster\n- \ntext\n- a\n- b"), ["VPN: works", "Scan faster", "a"]);
        assert!(!newer("v0.1.7", "0.1.7"));
        assert!(!newer("0.1.6", "0.1.7"));
    }

    /// Nothing is downloaded: each is refused before any request.
    #[test]
    fn installs_only_newer_releases() {
        for v in ["0.1.0", env!("CARGO_PKG_VERSION"), "", "9.9.9/../../x", "1.0.0-rc"] {
            assert_eq!(install(v, |_| {}), Err("Not a newer version of Octet".into()), "{v:?}");
        }
    }

    /// Copies the app out of a built disk image: `crates/octet/macos/bundle.sh --native`, then
    /// `cargo test -p octet unpacks_the_dmg -- --ignored`.
    #[test]
    #[ignore]
    #[cfg(target_os = "macos")]
    fn unpacks_the_dmg() {
        let dmg = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/release/Octet.dmg");
        let into = std::env::temp_dir().join(format!("octet-unpack-{}", std::process::id()));
        std::fs::create_dir_all(&into).unwrap();
        assert!(unpack(&dmg, &into), "Octet.app comes out of {}", dmg.display());
        assert!(!into.join("mnt/Octet.app").exists(), "the image is detached afterwards");
        std::fs::remove_dir_all(&into).unwrap();
    }

    #[test]
    fn only_our_signature_passes() {
        // "octet" signed with the release key (openssl pkeyutl -sign -rawin), as CI signs Octet.exe.
        let hex = "f54ae0ac4035a8dc998658a1c59f08cce77dffc575daae5c4fc72634d1858ec2ab56c182a5c031e516352232d695c5014becb10a0981215946efec00951b0007";
        let sig: Vec<u8> = (0..64).map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()).collect();
        assert!(verify(&KEY, b"octet", &sig), "our key's signature passes");
        assert!(!verify(&KEY, b"octets", &sig), "the same signature on other bytes fails");
        assert!(!verify(&KEY, b"anything", &[0u8; 64]), "a zero signature fails");
        assert!(!verify(&KEY, b"anything", &[1u8; 10]), "a short one fails");
    }

    fn unhex<const N: usize>(hex: &str) -> [u8; N] {
        std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
    }

    /// A throwaway key (not the release key) and a ".vsig" made with release.yml's own lines, for
    /// the file "octet" at tag v0.2.0:
    ///   openssl genpkey -algorithm ed25519 -out key.pem
    ///   printf 'octet' > Octet.dmg; GITHUB_REF_NAME=v0.2.0
    ///   version=$(printf '%s' "$GITHUB_REF_NAME" | sed 's/^v*//')
    ///   { printf 'octet-update\0%s\0' "$version"; cat Octet.dmg; } > Octet.dmg.msg
    ///   openssl pkeyutl -sign -inkey key.pem -rawin -in Octet.dmg.msg -out Octet.dmg.vsig
    ///   openssl pkey -in key.pem -pubout -outform DER | tail -c 32   (the public key)
    /// The Windows job's PowerShell lines give the same signature byte for byte.
    const TEST_KEY: &str = "16331a1d7d6e9eaa69c4c83dd908399cb695ceac5ac8ae15cff2f7dd9612ac16";
    const TEST_VSIG: &str = "f37a8bfc3ef1965c66c2efe44ce9e905dd488f552268902edf672400c4ed5f24a0c6b504ab26b86b66672ebc732273011fcf768452ec4904bb1b8bffb55d100f";

    #[test]
    fn message_is_what_ci_signs() {
        // The bytes of the .msg file above.
        assert_eq!(message("0.2.0", b"octet"), b"octet-update\x000.2.0\x00octet");
    }

    #[test]
    fn version_signature_binds_the_version() {
        let (key, sig) = (unhex::<32>(TEST_KEY), unhex::<64>(TEST_VSIG));
        assert!(verify(&key, &message("0.2.0", b"octet"), &sig), "the right version passes");
        assert!(!verify(&key, &message("0.2.1", b"octet"), &sig), "another version fails");
        assert!(!verify(&key, &message("0.2.0", b"octets"), &sig), "other bytes fail");
    }

    #[test]
    fn an_old_style_signature_is_not_enough() {
        // A ".sig" over the file alone, as CI still makes, offered where a ".vsig" belongs.
        let key = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new([7; 32]));
        let old = key.sk.sign(b"octet", None);
        assert!(verify(&key.pk, b"octet", old.as_ref()), "it is a good signature of the file");
        assert!(!verify(&key.pk, &message("0.2.0", b"octet"), old.as_ref()), "but not of the version");
    }
}
