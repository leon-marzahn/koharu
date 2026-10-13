# 25: Paint mode shows nothing

**Problem:** Pressing "Paint mask" on a region shows nothing at all: not the area you can paint in, not the stored mask, and (reported earlier) not what you have just painted staying visible. The user expects, when Paint mode is on and only then: the paintable area clearly marked, the stored mask drawn on it, and new strokes drawn as they are made. Tests pass because jsdom does not compute CSS masks or layout, so the tests prove nothing about what is on screen.

Two earlier fixes (tickets 22, 23, and a later commit "show the stored mask whenever the region is selected") did not solve it. Do not assume the cause; find it with real evidence.

**What to build:**
- Find out what the user actually sees. Check each layer in order and report which one fails: (1) `region-paint-surface` is rendered and has a visible size and position (`paintFrame`, `cssFrame`, `transform: undefined`, `overflow-hidden`, stacking order against the other overlays and the canvas); (2) `commands.getPaintedMask` returns bytes for a region that has a stored mask (`crates/koharu-app/src/commands/canvas.rs`, check the log at `~/Library/Logs/Koharu/koharu.log`); (3) the PNG decodes and is drawn. The stored mask is currently drawn with CSS `mask-image: url(blob:...)` and `mask-mode: luminance` in `RegionsOverlay.tsx`; the embedded Chromium may not honour that, so prefer drawing it deterministically: decode with `createImageBitmap`, tint it onto a `<canvas>`, no CSS masks.
- The paintable area must be visible in Paint mode (a tinted fill and an outline), strokes included.
- Show the paint area and stored mask ONLY while Paint mode is on. Remove the always-on display added in "fix(regions): show the stored mask whenever the region is selected" (the `!painting && region && maskFrame && maskNode` block), but keep it refreshed after Apply.
- Verify with real rendering, not jsdom: run the frontend in a real Chromium (Playwright or the Tauri dev build if available) and look at a screenshot, or at the very least assert on actual canvas pixels. State exactly what you verified and what you could not.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Root cause named, with evidence
- [ ] Paint mode shows the paintable area, the stored mask, and live strokes (checked in a real browser)
- [ ] Nothing is drawn when Paint mode is off
- [ ] Tests cover the final behaviour
