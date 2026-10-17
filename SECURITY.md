# Security

Octet changes network settings with administrator rights, so security reports matter.

**Report privately** through GitHub: Security tab, "Report a vulnerability" on
https://github.com/MrTomato77/octet. Please don't open a public issue for it.

## How elevation works

- The window itself never runs as administrator.
- Apply, enable/disable and DHCP release/renew start a second copy of `Octet.exe` through UAC.
  It receives one request on its command line, validates it again, runs `netsh` or `ipconfig`
  directly (no shell), and exits.
- Only fixed command shapes are possible; addresses are parsed as IPv4 before use.
- Octet makes no network requests of its own.
