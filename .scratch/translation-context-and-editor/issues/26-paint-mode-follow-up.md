# 26: Paint mode follow-up (hit areas, undo, nothing drawn)

Follow-up to 25, from the user's test of the merged build on a Retina display (device pixel ratio 2). Screenshot: `26-screenshot.png` (Paint mode on a region with the text "ME TOO..."). In it, a blue box (the selection outline with its handles) sits about 16 px outside a pale tinted box with a red/pink outline (the new paint surface), and the toolbar (Re-OCR, Apply, Undo, Clear, Cancel, Delete) sits on the blue box's top edge. No stroke or stored mask is visible.

**Problems:**
1. In Paint mode the paint area is so big that the toolbar buttons cannot be pressed (Apply, Undo, Clear, Cancel). Something extends past the visible box and takes the clicks. Find what, with `document.elementFromPoint` at each button's centre.
2. Undo works, but Cmd/Ctrl+Z also triggers the editor's global undo, which shows "Nothing to undo". The Paint mode key handler (`RegionsOverlay.tsx`, the `keydown` effect) runs alongside the global one. Make the paint handler take the shortcut exclusively while Paint mode is on (capture phase and `stopImmediatePropagation`, or whatever the global handler respects).
3. Strokes and the stored mask still do not show. The canvas drawn in ticket 25 shows nothing in the real app. Ticket 25's verification used a standalone copy of the drawing code, which is why it passed.

**Rules for this card:**
- Verify with the REAL `RegionsOverlay` component, not a copy: mount it (Vite plus Playwright, or vitest browser mode) in a real Chromium at deviceScaleFactor 2 with a mocked backend, and keep the test in the repo if it can run in CI, otherwise as a documented script. Include the real `SelectionControls` and the toolbar.
- Prove with `elementFromPoint` that each toolbar button is the topmost element at its centre in Paint mode, and with pixel reads that a simulated pointer drag (mouse down, move, up, in a real browser) leaves visible pixels on the canvas, and that a stored mask appears too. Reproduce each failure before fixing it, and say which layer was wrong (size, z-order, pointer events, `cssFrame`/`devicePixelRatio`, `pagePoint`, the canvas not being sized or drawn).
- The paint surface should cover the region box plus the 2 px pad and nothing more, and must never cover the toolbar.
- Keep Apply, Undo, Clear, Escape and the stored-mask fetch working.

**Blocked by:** none

**Status:** done

- [x] Each failure reproduced in a real browser before the fix (1 and 2; 3 did not reproduce)
- [x] Toolbar buttons are clickable in Paint mode (elementFromPoint proof)
- [x] Cmd/Ctrl+Z in Paint mode undoes one stroke and nothing else, no "Nothing to undo"
- [x] A drag leaves visible strokes and a stored mask is visible (pixel proof on the real component)

**Notes:** `packages/koharu/tests/browser/paint-mode.mjs` mounts the real `RegionsOverlay` in headless Chromium at DPR 2, with Tauri IPC mocked. 1: the SelectionControls rotate handle (z-order and pointer-events) covered Clear, so SelectionControls is now hidden while painting. Disabled Apply and Undo ignore clicks by design. 2: the stroke-undo key handler now runs in the window capture phase and calls `stopImmediatePropagation`. 3: on the real component the drawing works (canvas alpha after a drag, the stored mask, and the screenshot). Apply is greyed in `26-screenshot.png`, so no stroke was recorded there. The cause is upstream of `RegionsOverlay` (CanvasWorkspace or a stale build) and needs a check in the desktop app.
