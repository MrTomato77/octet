# CP1: Native Rust interface prototype

Date: 17 October 2026

## Status

Implementation and automated checks are complete for the mock-data prototype. OS-level hardware input/IME and real display-DPI switching remain open verification items. The complete IP manager and subsequent checkpoints are not implemented yet.

## Implemented

- Rust workspace with pure `simple-ip-core` and native `simple-ip-v2` app.
- A single window with profiles, sample adapters, current values, IPv4/DNS draft fields, memo and a pinned action strip.
- English/Thai labels and locally installed Windows fonts without redistribution of Microsoft font files.
- New session profiles, inline rename, session Save, literal search and drafts preserved across profile changes.
- Validation for contiguous subnet masks/prefixes, host/gateway addresses, manual DNS including a local resolver, duplicate/empty DNS, and static IP plus automatic DNS restriction.
- Preview compares the frozen sample target with desired configuration and does not mutate it.
- Light/dark themes, explicit sample-data labels, keyboard shortcuts and compact scrolling.
- Optional developer-only capture feature with deterministic fixture presets and settled native framebuffer capture.

## Verification performed

| Check | Result | Boundary |
|---|---|---|
| Windows MSVC debug/release build | Passed | Host Windows 11 x64; not a clean-machine installer test |
| Unit/session/UI tests | 16 passed | 9 domain, 3 session, 3 UI input/layout, 1 contrast |
| Formatting | Passed | `cargo fmt --all -- --check` |
| Clippy | Passed with warnings denied | Workspace, all targets and all features |
| Density matrix | Passed | English/Thai, 900×650 logical viewport at simulated 1.0/1.25/1.5/2.0 pixel density |
| Thai input | Passed in process | Injected Unicode text reaches focused search; not a Windows IME hardware test |
| Enter behavior | Passed in process | Enter does not preview; Ctrl+Enter previews without changing saved/sample state |
| Readability | Passed calculated checks | Body/background ≥7:1, muted/surface and white/primary ≥4.5:1, both themes |
| Native render captures | Inspected | Own-app framebuffer; English/Thai, compact, sample static/error/preview/theme presets |
| Native accessibility inventory | Available | Controls and values exposed through AccessKit; no screen-reader conformance claim |
| Windows activation/input automation | Unavailable | Session denied GetCursorPos with 0x80070005; not counted as passed |
| Original source preservation | Passed | All 44 source hashes match the original manifest |

The screenshot presets initialize sample state for visual checks; they do not count as clicking those controls through Windows. UI-density tests do not change OS display settings.

## Data and network boundaries

No network adapter is queried or modified. No privileged helper exists. No Apply/Renew/Release calls exist. No application network requests, shell-command execution or OS network mutation code are present in `crates/`. Profiles are kept in RAM and reset when the app exits; persistence is intentionally deferred to CP3.

Read access is limited to installed font files and ordinary GUI/runtime facilities. The locked Cargo dependencies were acquired for the build. Rustup installed the exact pinned 1.98.0 toolchain; the existing MSVC environment successfully linked the app.

## Files and decisions

New implementation: root Cargo workspace/toolchain/lockfile, `crates/core`, `crates/app`, `PRODUCT.md`, `DESIGN.md`, project README and these milestone/decision notes. The AutoIt tree and historical analysis are unchanged. No Git commit or network change was made.

GUI decision: `docs/decisions/0001-cp1-gui.md`.

## Remaining work

1. Validate hardware Thai typing/IME and keyboard navigation in an interactive Windows session.
2. Validate real OS DPI transitions and screen-reader behavior before claiming those support levels.
3. CP2: add Windows read-only snapshots with GUID identity, field-level unknown states and background work.
4. CP3: add persistent profiles and migration/import preview with error-safe writes.
5. CP4 onwards: operation planner/elevation/dry-run and controlled lab validation before network writes.

Do not mistake the release-mode CP1 executable for a write-enabled release of the full network tool. A release compiler profile describes optimization, not product readiness.
