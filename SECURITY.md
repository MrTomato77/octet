# Security

Octet changes network settings with administrator rights, so security reports matter.

**Report privately** through GitHub: Security tab, "Report a vulnerability" on
https://github.com/MrTomato77/octet. Please don't open a public issue for it.

## How elevation works

- The window runs without administrator rights, unless you set *Settings → Run as admin* to
  *Always*: then Octet asks once at start and the whole app runs elevated.
- Otherwise Apply, enable/disable and DHCP release/renew start a second copy of `Octet.exe` through UAC.
  It receives one request on its command line, validates it again, runs `netsh` or `ipconfig`
  directly (no shell), and exits. Its only output, error text, goes into a file the normal copy
  created beforehand: the elevated copy never creates a file and never writes through a link.
- Only fixed command shapes are possible; addresses are parsed as IPv4 before use.
- `Octet.exe` loads its DLLs from System32 only, so a DLL planted next to it (in Downloads, say)
  is never picked up, elevated or not.
- Updates come from this repository's releases, only when you ask. Octet installs one only if
  its Ed25519 signature checks out against the key built into the app, and only if it is newer
  than the copy running.
- No server of its own, no telemetry. What reaches your network and the internet is listed under
  *Privacy* in the [README](README.md).
