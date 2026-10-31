<div align="center">

# Octet

**Switch IPv4 and DNS in seconds. Then see who else is on the network.**

A fast, modern take on [Simple IP Config](https://github.com/KurtisLiggett/Simple-IP-Config), written in Rust. For Windows and macOS.

[![Release](https://img.shields.io/github/v/release/MrTomato77/octet?style=flat-square&color=3b82f6)](https://github.com/MrTomato77/octet/releases/latest)
[![Downloads](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FMrTomato77%2Foctet%2Fbadges%2Fdownloads.json&style=flat-square)](https://github.com/MrTomato77/octet/releases)
![Windows 10 | 11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078d4?style=flat-square&logo=windows)
![macOS 11+](https://img.shields.io/badge/macOS-11%2B-555?style=flat-square&logo=apple)
[![License: MIT](https://img.shields.io/badge/license-MIT-gray?style=flat-square)](LICENSE)

<img src="assets/ip.png" width="440" alt="Octet on the IP & DNS tab: a Wi-Fi adapter on Office-5G, automatic IP and DNS">

**[Download](https://github.com/MrTomato77/octet/releases/latest)** &nbsp;·&nbsp; `Octet.exe` or `Octet.dmg` &nbsp;·&nbsp; no installer &nbsp;·&nbsp; no telemetry

</div>

---

**IP & DNS** &nbsp;DHCP or manual in one click. Every field is checked as you type, and before Apply Octet asks the network whether someone already has the address. After Apply it pings the gateway and DNS for you.

<table>
<tr>
<td width="50%" valign="top" align="center">
<img src="assets/advanced.png" alt="Advanced tab: addresses and gateways on one adapter"><br>
<b>Advanced</b><br>
<sub>Several addresses and gateways, with metrics</sub><br><br>
</td>
<td width="50%" valign="top" align="center">
<img src="assets/ping.png" alt="Ping tab: a live latency graph to the gateway"><br>
<b>Ping</b><br>
<sub>Live latency, jitter and loss, out this adapter</sub><br><br>
</td>
</tr>
<tr>
<td width="50%" valign="top" align="center">
<img src="assets/scan.png" alt="Scan tab: every address on a /23 as a grid"><br>
<b>Scan</b><br>
<sub>Who is on the subnet, with maker and MAC</sub><br><br>
</td>
<td width="50%" valign="top" align="center">
<img src="assets/info.png" alt="Info tab: adapter details ready to copy"><br>
<b>Info</b><br>
<sub>Everything about the adapter, one click to copy</sub><br><br>
</td>
</tr>
<tr>
<td colspan="2" align="center">
<img src="assets/speed.png" width="440" alt="Speed test: the needle at 85 Mbps mid-download, with latency bars either side"><br>
<b>Speed test</b><br>
<sub>Download, upload and latency under load, out this adapter</sub><br><br>
</td>
</tr>
</table>

- Follows the system: cable, Wi-Fi and address changes show up on their own.
- Renew (and release, on Windows), flush DNS, enable / disable adapters, DNS presets, and a speed test.
- Copy or paste a whole setup as one line: `192.168.1.50/24 gw 192.168.1.1 dns 1.1.1.1, 8.8.8.8`
- Ctrl+1 to Ctrl+5 (⌘ on a Mac) switch tabs. English and ไทย.

## Good to know

- **First run:** Octet isn't code-signed or notarized yet. On Windows choose *More info → Run anyway*; on macOS, *System Settings → Privacy & Security → Open Anyway*.
- **Admin rights:** asked only for the change itself (Apply, enable / disable, release / renew), never just to open the app: UAC on Windows, your password on macOS.
- **Privacy:** no telemetry and no server of its own. The internet is reached only for your public IP and the speed test (through Cloudflare), and for updates when you ask.

Details on each of these, the macOS differences, and how to build: **[docs/guide.md](docs/guide.md)**.

## Star history

**Thank you** to everyone who uses Octet, reports a bug, suggests an idea or tries a new build. Every star tells us the app is worth making better.

<div align="center">
<a href="https://www.star-history.com/#MrTomato77/octet&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=MrTomato77/octet&type=Date&theme=dark">
    <img alt="Star history of MrTomato77/octet" src="https://api.star-history.com/svg?repos=MrTomato77/octet&type=Date" width="600">
  </picture>
</a>
</div>

## License

[MIT](LICENSE). Bundled icons, logos and crates are listed in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
Inspired by Simple IP Config by Kurtis Liggett (GPL-3.0); Octet shares no code with it.
