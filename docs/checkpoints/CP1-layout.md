# CP1 layout revision: compact comparison matrix

Date: 17 October 2026

## Request

The user rejected the first prototype's density and inconsistent text alignment. This revision changes layout and typography only; it does not add network access or persistence.

## Changes

- Replaced separate current/draft sections with Setting / Current / Desired rows.
- Current and desired values use the same font family and 14-pixel size. Left alignment is applied to actual text, with reserved column widths.
- Default viewport: 1040×800 → 820×620 logical pixels, approximately 39% less area.
- Sidebar: 236 → 190 pixels; profile rows: 60 → 32 pixels; controls: 32 → 28 pixels.
- Removed duplicate profile mode lines and consolidated the adapter selector with Refresh.
- Memo and the pinned primary action both fit at the default size, without scrolling the settings area.

## Verification

17 workspace tests passed; formatting and Clippy with warnings denied passed. The new regression checks English/Thai at simulated density 1.0/1.25/1.5/2.0 in an 820×620 viewport. It verifies stable column starts, vertical row centers, actual painted text origins for current/draft values, a shared right edge for the editor/memo, and the complete memo above the action strip.

Native framebuffer images of English/Thai and dark mode were inspected. These are rendered fixture presets, not OS-level mouse/IME tests. Remaining hardware/OS verification boundaries from CP1 still apply.

All 44 original AutoIt files remain unchanged. No Git commit and no network changes were made.

Thai typography: embedded unmodified Noto Sans Thai (SIL OFL 1.1). Grid row gaps are 3 logical pixels to preserve full memo visibility with Noto metrics. Font source and checksum are recorded in assets/fonts/README.txt.

Symbol pass: original vector strokes identify New, Save, Refresh, Copy, DHCP, Preview and profile IP modes. Text labels and hover explanations remain. Connection state uses both shape and text. No added asset/font dependency. Verified Thai/English layout and density tests: 17 passed; clippy all targets/features passed. Captured Thai light/dark and English previews for visual inspection.
