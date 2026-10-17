# Simple IP v2 visual system, CP1

## Context

A user sits at a Windows laptop in a normally lit office, comparing current IPv4 values with a saved lab configuration. A light surface supports repeated reading of compact numeric values; the tool should feel still and legible.

## Composition

One resizable native window, initially 820×620 logical pixels. A restrained top toolbar, a 190-logical-pixel profile sidebar, a scrollable working area, and a persistent status strip. Use one comparison matrix: Setting / Current / Desired, with current values and editable controls on the same rows. All data cells are left aligned, with fixed label/current column widths and the editor using the remaining width. The memo and primary action must both be visible at the initial size. Avoid nested cards and per-field decorative icons.

## Palette

Seed selected by impeccable's palette script: hue 3.4 degrees. Brand color is a restrained ruby used on the primary action and selected profile. All source tokens are OKLCH; convert to sRGB at the renderer boundary.

```css
--background: oklch(1.000 0.000 0);
--surface: oklch(0.975 0.000 0);
--ink: oklch(0.240 0.012 3.4);
--muted: oklch(0.460 0.010 3.4);
--border: oklch(0.870 0.006 3.4);
--primary: oklch(0.490 0.180 3.4);
--selection: oklch(0.955 0.025 3.4);
--accent: oklch(0.330 0.055 205);
```

## Typography and controls

Use bundled Noto Sans Thai as the primary proportional font, with installed Segoe UI and egui fallback. Noto is distributed under the SIL Open Font License; Microsoft fonts are not bundled. Labels, current values and edited values share a proportional 14-logical-pixel font and the same vertical text alignment. Window identity is 18 pixels; inputs/buttons are 28 pixels; profile rows are 32 pixels. Use 4/8/12 spacing and no duplicated second line for profile mode. Grid cells reserve their full width while aligning actual text at the left edge. Pair familiar stroke symbols with text labels on primary actions and profile modes. Use a shared 12-pixel drawing area, consistent 1.5-pixel strokes, and fixed left placement. Automatic IP uses a circular arrow; manual IP uses a pencil. Link states use a filled dot or crossed ring plus explicit text. Keep numeric comparison rows free of decorative symbols.

## States

The sample-data banner stays visible. Draft changes are marked in the list and editor. Preview builds a change plan without changing even the sample current snapshot. Save stores a profile in session only, explicitly stated in the status message. Search is literal and Unicode-safe. Native menu/IME and accessibility remain verification items, not assumed capabilities.




## Task-focused comparison refinement
Current values use muted reference text; desired fields use normal ink. Numeric differences have a diamond plus legend, with right padding reserved in the input. Unsaved status shares the heading row and never shifts the form. Current values support click-to-copy. Preview anchors the right side of the bottom action strip. See docs/checkpoints/CP1-UX-layout.md for audit, rationale and verification limits.

Icons use the bundled Phosphor Regular family at 16 logical pixels, isolated from Noto text. Consistent leading spacing separates glyphs and captions. Warning triangles plus field outlines represent validation failure; diamonds exclusively represent differences from the current snapshot. Draft state uses a pencil in both the list and comparison heading.

Numeric comparisons normalize subnet/prefix; mode changes have difference markers aligned at the shared right edge. Restore saved belongs to the profile sidebar and requires explicit discard confirmation. Preview uses a Setting/Before/After table and target identity. Clickable validation details wrap and focus the corresponding editor; changing a draft revalidates outstanding errors. Local storage failures remain visible. Drafts and committed profiles are persisted separately.

IPv4 inputs use four equally sized octet editors separated by fixed dots, with a single outline. All address rows reserve the same prefix slot and marker gutter so octet positions align vertically. Subnet includes a linked CIDR prefix editor; the data model retains canonical string drafts, including incomplete edits.
