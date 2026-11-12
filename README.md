<div align="center">

# Octet

**Switch IPv4 and DNS in seconds. Then see who else is on the network.**

A free, open-source **IP and DNS changer** for Windows and macOS: static IP, DHCP and DNS in one click, plus a network scanner.<br>
A fast, modern take on [Simple IP Config](https://github.com/KurtisLiggett/Simple-IP-Config), written in Rust. **[Website](https://thomasrieger.github.io/octet-web/)**

[![Release](https://img.shields.io/github/v/release/octet-app/octet?style=flat-square&color=3b82f6)](https://github.com/octet-app/octet/releases/latest)
[![Downloads](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Foctet-app%2Foctet%2Fbadges%2Fdownloads.json&style=flat-square)](https://github.com/octet-app/octet/releases)
![Windows 10 | 11](https://img.shields.io/badge/Windows-10%20%7C%2011-0078d4?style=flat-square&logo=windows)
![macOS 11+](https://img.shields.io/badge/macOS-11%2B-555?style=flat-square&logo=apple)
[![License: MIT](https://img.shields.io/badge/license-MIT-gray?style=flat-square)](LICENSE)

<img src="assets/ip.png" width="440" alt="Octet on the IP & DNS tab: a Wi-Fi adapter on Office-5G, automatic IP and DNS">

**[Download](https://github.com/octet-app/octet/releases/latest)** &nbsp;·&nbsp; `Octet.exe` or `Octet.dmg` &nbsp;·&nbsp; no installer &nbsp;·&nbsp; no telemetry

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
<td width="50%" valign="top" align="center">
<img src="assets/share.png" alt="Share tab: Wi-Fi shared to Ethernet, with a MikroTik RB5009 that joined"><br>
<b>Share</b><br>
<sub>Give a router or another PC this laptop's internet</sub><br><br>
</td>
<td width="50%" valign="top" align="center">
<img src="assets/speed.png" alt="Speed test: the needle at 85 Mbps mid-download, with latency bars either side"><br>
<b>Speed test</b><br>
<sub>Download, upload and latency under load, out this adapter</sub><br><br>
</td>
</tr>
</table>

- Follows the system: cable, Wi-Fi and address changes show up on their own.
- Release and renew, flush DNS, enable / disable adapters, DNS presets, and a speed test.
- Copy or paste a whole setup as one line: `192.168.1.50/24 gw 192.168.1.1 dns 1.1.1.1, 8.8.8.8`
- Ctrl+1 to Ctrl+6 switch tabs (⌘1 to ⌘6 on a Mac). English and ไทย.

## Good to know

- **First run:** Octet isn't code-signed or notarized yet, so Windows SmartScreen and macOS warn about an unknown publisher. On Windows choose *More info → Run anyway*; on macOS, *System Settings → Privacy & Security → Open Anyway*.
- **Is the download genuine?** Every file is built by [GitHub Actions](.github/workflows/release.yml) from this repository, and comes with a `.sha256` file. Check it before you run it:
  - Windows (PowerShell): `(Get-FileHash .\Octet.exe).Hash` should match the first word of `Octet.exe.sha256` (case aside)
  - macOS: `shasum -a 256 Octet.dmg` should print the same line as `Octet.dmg.sha256`

  Updates from inside Octet are checked against a signature (`.sig`) before they install. Prefer to build it yourself? See [docs/guide.md](docs/guide.md).
- **Admin rights:** asked only for the change itself (Apply, enable / disable, release / renew), never just to open the app: UAC on Windows, your password on macOS.
- **Privacy:** no telemetry. The internet is reached only for your public IP and the speed test (through Cloudflare), for updates when you ask, and for feedback when you press Send (your message, the Octet version and the system, posted as a GitHub issue through a small relay in [worker/](worker/)).

Details on each of these, the macOS differences, and how to build: **[docs/guide.md](docs/guide.md)**.

## Star history

**Thank you** to everyone who uses Octet, reports a bug, suggests an idea or tries a new build. Every star tells us the app is worth making better.

<div align="center">
<a href="https://www.star-history.com/#octet-app/octet&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=octet-app/octet&type=Date&theme=dark">
    <img alt="Star history of octet-app/octet" src="https://api.star-history.com/svg?repos=octet-app/octet&type=Date" width="600">
  </picture>
</a>
</div>

## License

[MIT](LICENSE). Bundled icons, logos and crates are listed in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
Inspired by Simple IP Config by Kurtis Liggett (GPL-3.0); Octet shares no code with it.
