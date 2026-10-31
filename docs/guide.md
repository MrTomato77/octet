# Octet in detail

What the [README](../README.md) leaves out: installing on each system, admin rights, what Octet sends over the network, and building it.

## Windows

Get `Octet.exe` from [Releases](https://github.com/MrTomato77/octet/releases/latest) and run it. It is one portable file, with no installer.

- **Windows 11:** runs as is.
- **Windows 10:** needs the Microsoft Edge WebView2 Runtime. Most PCs already have it.
- **Not code-signed yet:** SmartScreen may say *"Windows protected your PC"* the first time. Choose *More info*, then *Run anyway*. Each release has `Octet.exe.sha256` to check the download:

  ```powershell
  (Get-FileHash .\Octet.exe).Hash
  ```

## macOS

Get `Octet.dmg` from the same release, open it and drag Octet onto Applications. It runs on Apple silicon and Intel, macOS 11 or later.

- **Not notarized yet:** the first time, macOS says it can't check Octet. Open *System Settings → Privacy & Security* and choose *Open Anyway*.
- **Wi-Fi name:** macOS gives the network's name only to apps with Location permission, so Octet asks once. Say no and everything else still works; only the name stays hidden.
- **What differs from Windows:** each change asks for your password (there is no *Run as admin*). There is no Advanced tab, because macOS keeps one address per network service. There is no Release, because macOS can only renew a lease. *Register in DNS* is Windows-only.

## Administrator permission

Octet opens without admin rights. It asks only when you Apply, enable / disable an adapter, or release / renew, and only that one change runs elevated (see [SECURITY.md](../SECURITY.md)): through UAC on Windows, your password on macOS. Changing things often on Windows? Set *Settings → Run as admin* to *Always* and it asks once at start.

## Privacy

No telemetry, no server of its own. What it shows comes from the system itself.

On your network it sends only what you'd expect:

- an ARP check before Apply, and pings to the gateway and DNS after it;
- pings and ARP while you Ping or Scan;
- a reverse name lookup for the gateway.

Device makers come from the IEEE list built into the app; MAC addresses are never looked up online.

The internet is reached only when you ask: your public IP (Info) and the speed test, both through Cloudflare ([speed.cloudflare.com](https://speed.cloudflare.com)), out through the adapter on screen. Updates are looked for on GitHub only when you press the button in Settings, or when Octet starts if you turn on *Auto-check* (off unless you do). A download is installed only if its signature checks out and it is newer than the copy you run.

## How it works

The backend is Rust. On Windows it talks to the system directly: IP Helper for addresses, DNS and DHCP, ICMP for ping, WinHTTP for update checks, and TLS sockets pinned to the adapter on screen for the speed test and public IP. No `ipconfig`, no PowerShell, no console windows flashing by; a DNS change takes milliseconds. On macOS, changes go through `networksetup`.

The interface is one file, `ui/index.html`. Open it over http in a browser and it runs on sample data and changes nothing.

## Build

```powershell
cargo build --release -p octet   # target/release/octet.exe
```

On a Mac, `crates/octet/macos/bundle.sh` builds `target/release/Octet.app` and `Octet.dmg` (`--native` for this Mac's architecture only, which is faster).

Releases are built by [GitHub Actions](../.github/workflows/release.yml) on each `v*` tag.
