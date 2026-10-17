# Octet

Switch IPv4 and DNS settings on Windows in seconds. A fast, modern take on [Simple IP Config](https://github.com/KurtisLiggett/Simple-IP-Config), built with Rust and Tauri.

> **Status: early.** The interface is being designed in `ui/demo.html` (open it in a browser; sample data, touches nothing). The app is moving from egui to Tauri; the earlier egui build is described below and archived on the `egui-archive` branch.

Licensed under the [MIT License](LICENSE). Inspired by Simple IP Config by Kurtis Liggett (GPL-3.0); no code is copied from it.

---

A native Rust IPv4/DNS settings tool for Windows. Current milestone: **CP1 interface prototype**, using sample data only.

## What works now

- Single form: pick an adapter, compare current values with desired IPv4/DNS settings.
- English/Thai interface.
- DHCP/manual IP and DNS controls, validated subnet/gateway/DNS values.
- Copy sample current values as manual settings; preview the configuration difference.
- Light/dark themes, compact-window scrolling and a persistent primary action.

## Current boundary

Adapters are read from Windows at startup and on Refresh (F5), read-only, via one local PowerShell/CIM query plus the interface registry key for static DNS. If that read fails, sample adapters are shown and the status bar says so. There are no saved profiles: the form starts from the selected adapter's current configuration. This build still has no elevation helper, Apply, Renew or Release. Only language, theme and window size persist, in %LOCALAPPDATA%/SimpleIPv2/state.json; version-1 files keep those preferences and drop their profiles. The previous file is backed up. Corrupt or newer-format files are preserved and writes are blocked with a visible notice; a valid backup can be restored explicitly. There are no network probes or external requests in application code. To print what the app reads on this machine: `cargo test -p simple-ip-v2 reads_this_machine -- --ignored --nocapture`.

Windows reads belong to CP2. The planning document is `docs/RUST-IMPLEMENTATION-PLAN.md`; evidence and remaining checks are in `docs/checkpoints/CP1.md`.

## Run

Open the executable in `dist/cp1/simple-ip-v2.exe`, or build from source:

```powershell
cargo run -p simple-ip-v2
cargo run -p simple-ip-v2 -- --thai --compact
```

Release build:

```powershell
cargo build --release -p simple-ip-v2
```

Requires the pinned Rust 1.98.0 Windows MSVC toolchain and a compatible Windows SDK/linker. Tested build environment: Windows 11 x64. Noto Sans Thai is embedded under the SIL Open Font License, with installed Segoe UI and egui fallback; Microsoft fonts are not bundled.

## Shortcuts

| Shortcut | Action |
|---|---|
| Ctrl+Enter | Preview changes only |
| F5 | Refresh sample snapshot without replacing the draft |

Ordinary Enter does not apply changes.

## Verification

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The UI tests inject input into egui in process. They do not replace hardware keyboard/Windows IME or screen-reader tests. The density matrix tests logical viewport layout, without changing the machine's display settings.

Optional developer framebuffer capture:

```powershell
cargo build -p simple-ip-v2 --features capture
New-Item -ItemType Directory -Path '.\work' -Force
$env:SIMPLE_IP_CAPTURE_TO = 'D:\Simple IP v2\work\preview.png'
$env:SIMPLE_IP_CAPTURE_SCENARIO = 'lab'
cargo run -p simple-ip-v2 --features capture -- --thai
```

The capture feature waits for layout to settle, saves the app framebuffer and closes the preview process. Presets `lab`, `dhcp`, `preview`, `invalid`, `dark` and combinations such as `dark-preview` use sample state; they do not operate Windows UI or change the network. Remove those environment variables when finished. The normal release excludes capture tooling.

## Source reference

`source/` is the original third-party AutoIt reference. Its files remain unchanged. New Rust implementation lives in `crates/`; it contains no copied application assets or translated AutoIt implementation. Licensing the new project for external distribution is a separate project decision; dependency notices accompany the CP1 artifact.


