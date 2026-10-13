# 24: Region selection survives move and resize, and can be cleared

**Problem:** In the regions view, selecting a region and then moving it (dragging the box) or resizing it (edge handles) deselects it, so Re-OCR, Paint mask and Delete disappear until you click it again. Reported on every region. Also, clicking empty canvas does not deselect a region (text layers do); `selected` in `RegionsOverlay.tsx` is only cleared by Delete. Backend `set_region_geometry` keeps the same entity id, so the cause is on the frontend (`commit` -> `setRegionGeometry` -> `refresh`): a remount of the overlay, a replaced page object, `synchronize_canvas` publishing, or `selectLayers([])` interplay.

**What to build:** Reproduce first with a failing test through the real components (render `CanvasOverlay` with the regions view on, click a region, move it, then resize it with `[data-resize-handle="e"]`, let the refreshed page arrive with the same region id and new geometry, assert the selection controls and the region buttons are still shown). Fix the cause. If it is a remount, keep the selected region id where it survives one (for example the Zustand store next to `selectLayers`). Then add deselect: clicking empty canvas, pressing Escape (when not painting), or selecting a text layer clears the region selection. Handle press and release on a region must not count as an empty click.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Selecting a region, then moving it, keeps it selected (test)
- [ ] Selecting a region, then resizing it, keeps it selected (test)
- [ ] Clicking empty canvas, Escape (not painting) and selecting a text layer deselect the region (tests)
- [ ] Escape still cancels paint mode first
