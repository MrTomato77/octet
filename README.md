# Octet

Switch IPv4 and DNS settings on Windows in seconds. A fast, modern take on [Simple IP Config](https://github.com/KurtisLiggett/Simple-IP-Config), built with Rust and Tauri.

> **Status: 0.1, first test build.** Reads this machine's adapters and applies IPv4/DNS changes through `netsh`, asking for administrator permission (UAC) only when you press Apply, enable/disable, or release/renew. Try it on an adapter you can afford to lose first.

## Download

Grab `Octet.exe` from [Releases](https://github.com/MrTomato77/octet/releases). One portable file, no installer. Needs Windows 10/11 with WebView2 (built into Windows 11).

## What it does

- Switch an adapter between DHCP and manual IP / DNS, with live validation.
- Shows what each address is (private, public, APIPA, CGNAT) and who runs your DNS.
- Release / renew DHCP, flush DNS, enable / disable adapters, DNS provider presets.
- Copy or paste a whole configuration as one line: `192.168.1.50/24 gw 192.168.1.1 dns 1.1.1.1, 8.8.8.8`.
- No telemetry, no network calls of its own. Everything it shows comes from Windows.

## Build

```powershell
cargo build --release -p octet   # target/release/octet.exe
```

The interface is `ui/index.html`; opened in a browser (served over http) it runs on sample data.

Licensed under the [MIT License](LICENSE). Inspired by Simple IP Config by Kurtis Liggett (GPL-3.0); no code is copied from it.
