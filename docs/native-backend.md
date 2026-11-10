# Windows: what runs native, what still runs netsh

The elevated copy runs these in-process (`crates/octet/src/win.rs`, `ics.rs`, `net.rs`):

| Operation | API |
| --- | --- |
| Static or DHCP primary address | WMI `Win32_NetworkAdapterConfiguration.EnableStatic` / `EnableDHCP`, then typed-in addresses (DHCP) and default routes deleted through `MSFT_NetIPAddress` / `MSFT_NetRoute` in the active and persistent stores |
| Advanced addresses and gateways | `MSFT_NetIPAddress.Create` / `MSFT_NetRoute.Create` (route metric 0 = automatic), deleted per store |
| Enable / disable adapter | WMI `Win32_NetworkAdapter.Enable` / `Disable`, by interface index |
| Interface metric, incl. automatic | IP Helper `SetIpInterfaceEntry` now; the Tcpip `InterfaceMetric` registry value for after a restart (deleted = automatic: no API empties it) |
| DNS, DHCP release / renew, flush | IP Helper / DHCP client API (unchanged) |
| Internet Sharing | ICS COM `INetSharingManager` / `INetSharingConfiguration` |

Still netsh: **DHCP/static coexistence** only. No documented public API found yet (not in `MSFT_NetIPInterface`).

Checked live on the Npcap loopback adapter (2026-11-10), each step in both stores: static address + gateway + DNS (~350 ms), Advanced add and remove (~100 ms), back to DHCP with an extra address and coexistence on (~420 ms), last extra removed with coexistence off, metric manual and automatic surviving a disable/enable, enable/disable. Internet Sharing not yet checked (needs a real Ethernet adapter).
