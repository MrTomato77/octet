# Octet

Switch IPv4 and DNS settings on Windows in seconds. A fast, modern take on [Simple IP Config](https://github.com/KurtisLiggett/Simple-IP-Config), built with Rust and Tauri.

> **Status: 0.1, first test build.** Try it on an adapter you can afford to lose first.

## Download

Get `Octet.exe` from [Releases](https://github.com/MrTomato77/octet/releases). It is one portable file with no installer.

- **Windows 11:** runs as is.
- **Windows 10:** needs the Microsoft Edge WebView2 Runtime. Most PCs already have it; if Octet doesn't open, install it from Microsoft.
- **Not code-signed yet:** SmartScreen may warn the first time. Choose *More info*, then *Run anyway*. Each release lists the file's SHA-256 so you can check the download.

## What it does

- Switch an adapter between DHCP and manual IP / DNS, with live validation and pre-checks (gateway outside the subnet, address equal to the gateway, network or broadcast address).
- Shows what each address is (private, public, APIPA, CGNAT) and who runs your DNS.
- Release / renew DHCP, flush DNS, enable / disable adapters, DNS provider presets.
- Copy or paste a whole configuration as one line: `192.168.1.50/24 gw 192.168.1.1 dns 1.1.1.1, 8.8.8.8`.
- English and Thai.

## Administrator permission

Octet opens without admin rights. It asks through UAC only when you press Apply, enable / disable an adapter, or release / renew, and only that one change runs elevated (see [SECURITY.md](SECURITY.md)).
Changing settings many times in a row? Right-click `Octet.exe`, *Run as administrator*, and it won't ask again for that session.

## Privacy

Octet makes no network requests of its own and has no telemetry. Everything it shows comes from Windows (`Get-NetAdapter`, CIM). WebView2, the Microsoft component that draws the window, keeps itself updated through Windows.

## Build

```powershell
cargo build --release -p octet   # target/release/octet.exe
```

Release builds come from [GitHub Actions](.github/workflows/release.yml) on each `v*` tag. The interface is `ui/index.html`; served over http in a browser, it runs on sample data and changes nothing.

## License

[MIT](LICENSE). Bundled icons, logos and Rust crates are listed in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
Octet is a new Rust/HTML program inspired by Simple IP Config by Kurtis Liggett (GPL-3.0); it shares no code with it.
