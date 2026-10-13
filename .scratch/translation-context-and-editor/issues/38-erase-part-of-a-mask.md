# 38: Erase part of a painted mask

**What to build:** Paint mode on a region (`components/editor/RegionsOverlay.tsx`) can only add to the stored mask. The only way to fix a small mistake is "Clear mask" (the eraser-icon button), which drops the whole stored mask. Add a way to erase part of it, and make Clear all hard to hit by accident.

The strip today is Apply, Undo stroke, Clear mask, Cancel. The user chose this layout:

`[Apply] [Undo stroke] [Brush / Eraser toggle] [Clear all] [Cancel]`

- **Brush/Eraser toggle:** one button with `aria-pressed`. Off (default, and reset every time Paint mode opens): strokes add to the mask as now. On: strokes remove mask. Use the `Eraser` icon on it, with a label like "Erase from mask" (real translations in all nine locales).
- **Clear all:** the old Clear mask action under a new icon. It must not be `Eraser` (now the toggle) or `Trash2` (delete region, same strip); pick a lucide-react icon that is installed and tell me which. When there is something to lose (a stored mask or unapplied strokes) it asks for confirmation first; use the repo's existing confirm dialog primitive if there is one, do not add a dependency. When there is nothing to lose it stays disabled, as today.
- **Strokes carry their kind and size.** The overlay's `painting.strokes` becomes a list of `{ points, diameter, erase }`; `diameter` is the brush size at the moment the press started. Today `brush.diameter` is applied to every stroke at Apply, so changing the size mid-session resizes the earlier strokes; fix that here. The preview draws strokes in order on the same canvas: add strokes with the tint, erase strokes with `destination-out`, so paint, erase, paint looks right live and the stored mask can be erased too.
- **Undo stroke** undoes erase strokes the same way. Cmd/Ctrl+Z in Paint mode still undoes only the last stroke. Apply stays disabled with no strokes. Do not rebind Alt or add shortcuts.
- **Backend:** change `paint_region_mask` (`crates/koharu-app/src/commands/canvas.rs`) to take the new stroke shape instead of `strokes: Vec<Vec<Point>>, diameter`. Keep `replace` (the `cleared` flag). It rasterises the strokes in order onto the stored mask (or a blank one when `replace`): add strokes set pixels to masked, erase strokes set them to unmasked. Keep the existing validation (positive finite diameter, finite points), per stroke. Regenerate `protocol.ts`.
- **Empty result:** if erasing leaves no masked pixel at all, treat it as no mask (remove the `PaintedMask` component), not an all-black mask that inpaints nothing. Say in your report what you did.
- The mask stays user-authored: no `SHARED_WITH_PIPELINE`. A painted mask still survives region move and resize, and is visible only in Paint mode.

Tests, written first:
- Rust: an erase stroke removes only the pixels it covers; paint, erase, paint in that order gives the expected pixels; erasing on an empty or missing mask changes nothing and does not create a component; each stroke uses its own diameter; `replace` followed by an erase works; fully erasing removes the component; invalid diameter or points are rejected per stroke.
- Frontend (through the real overlay, mounted in the real canvas layout): the toggle switches stroke kind and `aria-pressed`; the command gets strokes with the right `erase` flags and per-stroke diameters after changing the size mid-session; Clear all asks for confirmation and does nothing on cancel; Undo stroke removes an erase stroke; the toggle resets on re-entering Paint mode.

Out of scope: a non-rectangular eraser, a zoomed brush cursor, keyboard shortcuts. Observation to report, not fix: Clear all followed by Apply with no new stroke is disabled today (`applyMask` needs `strokes.length`), so clearing needs a new stroke to take effect.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Brush/Eraser toggle erases part of the stored mask and of unapplied strokes, with live preview
- [ ] Clear all is a separate button with a different icon and a confirmation when something would be lost
- [ ] Each stroke keeps its own brush size
- [ ] Rust and frontend tests above pass; `protocol.ts` regenerated; nine locales updated
