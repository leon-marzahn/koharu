# 27: Snap to container edges and to other boxes

**What to build:** Extend the drag snap from ticket 17 (`snapMoveToBubble` in `packages/koharu/lib/geometry.ts`, wired in `CanvasWorkspace.tsx`, guides in `CanvasOverlay.tsx`). While dragging text boxes:
- The dragged boxes' left, right, top and bottom edges also snap to the matching edges of the bubble containing them (so a box sits flush against its container), as well as the centre snap that exists.
- The edges and centres of the dragged boxes snap to the left, right, top, bottom and centre lines of the other text boxes on the same page, so several boxes can be lined up vertically or horizontally.
- Same 8 screen px threshold at every zoom, one snap per axis (the closest candidate wins, ties go to the container centre), a faint guide line for each active snap (a line along the shared edge or centre), Cmd held disables all of it. Alt stays reserved by the canvas.
- A moved group still snaps as one union box (ticket 17's rule). The boxes being dragged never snap to themselves.
- Only text boxes snap, as before. Boxes outside any bubble still snap to other boxes, but not to a container.

Resize handles: if the canvas transform hook exposes the resize frame in the same place as move (`updateTransform`), also snap the moving edge to the same lines. If it does not, leave resize alone and say so in the summary; it is a separate follow-up.

Out of scope: align/distribute buttons for a multi-selection, a settings toggle, grid snapping.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Dragging a box near its bubble's edge snaps it flush, with a guide
- [ ] Boxes line up with other boxes' edges and centres, horizontally and vertically, with guides
- [ ] Threshold is 8 screen px at any zoom; Cmd disables everything
- [ ] The centre snap from ticket 17 still works and its tests still pass
- [ ] Group moves and boxes outside bubbles behave as described
- [ ] Unit tests for the snap function; component test through the real drag path (real kind `dev.koharu.region.bubble`)
