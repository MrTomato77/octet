# CP1 task-focused layout review

Mode: preserve. Audience: Windows users repeatedly choosing an adapter, selecting a profile, comparing IPv4 settings, then checking a change preview. This is a task-based design hypothesis; no external user study was performed.

Design read: compact native desktop utility, restrained existing ruby palette, Noto Sans Thai. DESIGN_VARIANCE 3, MOTION_INTENSITY 1, VISUAL_DENSITY 7. Native eframe controls remain the implementation; this is not an official Fluent UI implementation. Web marketing, SEO, hero imagery, React package and browser performance rules are inapplicable. Existing icons are retained for consistency; this pass adds only semantic difference markers.

## Audit of previous version

Preserve: single page, aligned three-column comparison, fixed bottom actions, keyboard shortcuts, Noto font, light/dark themes, clear fixture indication. Tokens and component sizes are recorded in DESIGN.md. Source backup before edits: work/layout-before/app.rs.

Problems observed in the previous application captures and source:
- Repeated session/sample explanations competed with settings.
- Adding an unsaved label inserted a row and shifted the form during editing.
- Current values and editable values had similar emphasis.
- Users had to compare every numeric field to locate changes.
- Copying the whole adapter did not serve users needing only one value.
- The primary action did not have a distinct right-hand anchor.

## Changes

- Keep all field names, order and identifiers; preserve sidebar/adapter/compare/action sequence.
- Place unsaved status at the right end of the existing comparison heading, reserving its vertical space.
- Render current reference values in the existing accessible muted color. Desired controls retain normal ink.
- Show a diamond inside an enabled address field when its parsed IPv4 value differs from the current value. Reserve right-side input padding. Provide a visible legend and tooltip. Empty gateway removes an assigned current gateway and is therefore marked.
- Current values can be clicked to copy to the clipboard, with a copy cursor and bilingual tooltip. This copies text only; it does not overwrite the profile.
- Anchor the primary preview action to the right. Secondary tools stay in the same fixed bottom strip.
- Replace repeated sidebar paragraphs with one session-storage note and an explanatory tooltip.

## Verification and limits

17 workspace tests passed. Extended existing geometry regression to assert that an edited draft does not shift the comparison or memo in Thai/English at density 1, 1.25, 1.5, 2. Clippy all targets and features passed. Native app framebuffer captures inspected for Thai/English, light/dark and unsaved state. Visual inspection caught and corrected a heading-status overlap.

Original source remains untouched. CP1 remains fixtures only, Save remains session only, and no real network changes are performed. Clipboard delivery through the host operating system and real user task timing have not been manually measured. Difference markers cover editable numeric fields, not a complete semantic network change plan; Preview remains the complete comparison.

## Symbol refinement
Replaced hand-drawn action icons with bundled Phosphor Regular (MIT). Added search, adapter type (wired/Wi-Fi), current/desired headings, language/theme/help, unsaved pencil, validation triangle and error outline. Warning replaces the difference diamond for failed numeric fields and the profile name. Details remain in validation feedback and tooltips. Error mapping reuses core validation, and editing clears stale errors through existing invalidation. Shape and text convey state without depending on color. No new network functionality. 17 tests and all-feature clippy passed. Thai/English light, dark, dirty and invalid captures inspected; symbol/text spacing refined after visual review.
