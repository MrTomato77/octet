<div align="center">

# Octet

**Switch IPv4 and DNS on Windows in seconds. Then see who else is on the network.**

A fast, modern take on [Simple IP Config](https://github.com/KurtisLiggett/Simple-IP-Config), written in Rust.

[![Release](https://img.shields.io/github/v/release/MrTomato77/octet?style=flat-square&color=3b82f6)](https://github.com/MrTomato77/octet/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/MrTomato77/octet/total?style=flat-square&color=22c55e)](https://github.com/MrTomato77/octet/releases)
![Windows 10 | 11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078d4?style=flat-square&logo=windows)
[![License: MIT](https://img.shields.io/badge/license-MIT-gray?style=flat-square)](LICENSE)

<img src="assets/ip.png" width="440" alt="Octet on the IP & DNS tab: a Wi-Fi adapter on Office-5G, automatic IP and DNS">

**[Download Octet.exe](https://github.com/MrTomato77/octet/releases/latest)** &nbsp;·&nbsp; one portable file &nbsp;·&nbsp; no installer &nbsp;·&nbsp; no telemetry

</div>

---

## Five tabs, one adapter

<table>
<tr>
<td width="50%" valign="top">

### IP & DNS
DHCP or manual in one click. Every field is checked as you type: a gateway outside the subnet, the network or broadcast address, and before Apply, another device already using the address (ARP). After Apply, Octet pings the gateway and DNS for you.

</td>
<td width="50%" valign="top">

### Advanced
<img src="assets/advanced.png" alt="Advanced tab: addresses and gateways on one adapter">

Several addresses and gateways on one adapter, with gateway and interface metrics. Extra addresses can sit beside a DHCP address too.

</td>
</tr>
<tr>
<td valign="top">

### Ping
<img src="assets/ping.png" alt="Ping tab: a live latency graph to the gateway with average, max, jitter and loss">

A live graph to your gateway, your DNS or any address, with average, max, jitter and loss. It goes out through the adapter on screen, not whichever one has the default route.

</td>
<td valign="top">

### Scan
<img src="assets/scan.png" alt="Scan tab: every address on a /23 as a grid, in use and free">

Every address on the subnet as a grid, filled in as the sweep reaches it. Hover a cell for maker, MAC and response time. Click a free one to take it.

</td>
</tr>
<tr>
<td valign="top">

### Info
<img src="assets/info.png" alt="Info tab: computer name, adapter, link, MAC, Wi-Fi signal">

Everything about the adapter in one place: link, Wi-Fi signal and security, addresses, IPv6, DHCP lease, DNS. Click a row to copy it, or copy it all for a ticket. Your public IP is looked up only when you click.

</td>
<td valign="top">

### And the small things
- Follows Windows: cable, Wi-Fi and address changes show up on their own.
- Release / renew, flush DNS, enable / disable adapters, DNS presets.
- Copy or paste a whole setup as one line:<br>`192.168.1.50/24 gw 192.168.1.1 dns 1.1.1.1, 8.8.8.8`
- Ctrl+1 … Ctrl+5 to switch tabs.
- English and ไทย.

</td>
</tr>
</table>

## Fast, all the way down

The backend is Rust talking to Windows directly: IP Helper for addresses, DNS and DHCP, ICMP for ping, WinHTTP for the one web request. No `ipconfig`, no PowerShell, no console windows flashing by. A DNS change takes milliseconds.

## Download

Get `Octet.exe` from [Releases](https://github.com/MrTomato77/octet/releases/latest) and run it.

- **Windows 11:** runs as is.
- **Windows 10:** needs the Microsoft Edge WebView2 Runtime. Most PCs already have it.
- **Not code-signed yet:** SmartScreen may say *"Windows protected your PC"* the first time. Choose *More info*, then *Run anyway*. Each release has `Octet.exe.sha256` to check the download:

  ```powershell
  (Get-FileHash .\Octet.exe).Hash
  ```

## Administrator permission

Octet opens without admin rights. It asks through UAC only when you Apply, enable / disable an adapter, or release / renew, and only that one change runs elevated (see [SECURITY.md](SECURITY.md)). Changing things often? Set *Settings → Run as admin* to *Always* and it asks once at start.

## Privacy

No telemetry, no server of its own. What it shows comes from Windows itself. On your network it sends only what you'd expect: an ARP check before Apply, pings to the gateway and DNS after it, pings and ARP while you Ping or Scan, and a reverse name lookup for the gateway. Device makers come from the IEEE list built into the app; MAC addresses are never looked up online. The only request to the internet is your public IP, through [ipinfo.io](https://ipinfo.io), and only when you click it.

## Build

```powershell
cargo build --release -p octet   # target/release/octet.exe
```

Releases are built by [GitHub Actions](.github/workflows/release.yml) on each `v*` tag. The interface is one file, `ui/index.html`; open it over http in a browser and it runs on sample data and changes nothing.

## License

[MIT](LICENSE). Bundled icons, logos and crates are listed in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
Inspired by Simple IP Config by Kurtis Liggett (GPL-3.0); Octet shares no code with it.
