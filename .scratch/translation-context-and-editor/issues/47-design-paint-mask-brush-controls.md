# 47: Design: one brush for the toolbar and Paint mode

**What to build:** A design document only. Do not change source.

**The problem (from the user):** The toolbar already has brush tools (`draw`, `eraser`, `remove`; `isBrushTool` in `lib/store.ts`). Their size slider and colour well show only while one of those tools is active (`ToolBar.tsx`, `hasBrush`). Paint mode on a region (`RegionsOverlay.tsx`) draws with the same global `brush.diameter` from the store, but its own strip (Apply, Undo stroke, Clear mask, Cancel) shows no size and the toolbar slider is hidden unless the user happens to have a brush tool selected. So the user could not discover that the toolbar brush size controls the mask stroke. Two controls that are one thing need to read as one thing.

Questions the design must answer, each with 2 or 3 options and a mockup (ASCII or an HTML file, like `design/36-run-menu.md`):
- Where does the size control live while Paint mode is open? (Show the toolbar's size popover whenever Paint mode is on regardless of tool; put a size control in the Paint strip; both bound to the same value.)
- Which toolbar tool is active in Paint mode, and does opening Paint mode change it? Today Paint mode is entered from a region and the active tool is whatever it was.
- Is colour shown? A mask stroke is tinted by the overlay, not by `brush.color`, so the colour well must be hidden or explained in Paint mode.
- How does this fit with ticket 38's Brush/Eraser toggle in the Paint strip, and the toolbar `eraser` tool, which also erases (different target, same icon)? Name the overlap and say which one wins.
- Does Paint mode need its own brush size, separate from the canvas brush, or is one global size the right model? Ticket 38 already stores each stroke's diameter, so a separate value is possible.
- Cursor: the brush cursor (`showBrushCursor`) is tied to `isBrushTool(tool)`; say whether Paint mode shows it.

Deliverable: `.scratch/translation-context-and-editor/design/47-paint-brush-controls.md` with the current state (cite file and line), the options, a recommendation, and what each tickets-to-be would touch. Leave a `Decision:` line for the user to fill. The user chooses the placement; do not pick it for them.

**Blocked by:** 38

**Status:** ready-for-agent

- [ ] Design doc written with the current state, options with mockups, and a recommendation
- [ ] The overlap with ticket 38 and the toolbar eraser is named and resolved in each option
- [ ] No source files changed
