# 22: Inpaint drawn regions, and paint a mask for a region

**Problem:** The Inpaint stage builds its mask only from the detector's text mask (`crates/koharu-pipeline/src/stages/inpainting.rs`, `prepare`). A region the user draws (ticket 14) has no detector mask, so on a page the detector found nothing on, Inpaint has nothing to clean and does nothing, with or without an existing cleanup layer. Today the only cleaning of a drawn region is `process_region` in `crates/koharu-app/src/commands/canvas.rs`, which fills a padded rectangle once, right after drawing.

**What to build:**

1. The Inpaint stage covers user-drawn text regions. For every text region with user origin that is inside the run's scope, its mask is added to the stage's mask: the region's painted mask if it has one (below), else its bounding box padded by 2 px (what `process_region` uses; share the helper). So "Inpaint" (page, selection, or "all") cleans drawn regions even when the detector found nothing. Regions the detector made are unchanged.
2. A "Paint mask" control next to Re-OCR and Delete on the selected region (`RegionsOverlay.tsx`). It enters a brush mode scoped to that region: strokes are clipped to the region's bounds (padded a little) and shown as an overlay; "Apply" commits (Escape cancels, and a cancel or an empty stroke must leave no canvas edit active). Applying runs the existing manual-mask inpainting path (`Processing.inpainting_mask` plus `Scope::Region` and `Operation::Only { Inpainting }`) with the painted mask.
3. The painted mask is stored on the region as a user-authored, revisioned component (`#[serde(default)]`, no `SHARED_WITH_PIPELINE`; store the image the way raster layers store theirs, look at `components/layers.rs`). The stage in 1 uses it instead of the rectangle. Decision: moving or resizing the region clears the painted mask (the box fallback applies again), because a mask stored for the old box would sit over the wrong pixels.
4. `process_region` (Draw region, which cleans right after OCR and translation) keeps working and uses the same mask rule.

The explicit "only inpainting" run already overrides the skip for pages that have a cleanup layer (`forces_inpainting` in `execution.rs`); keep that. Full runs keep the skip.

**Blocked by:** none (14 and 20 are merged)

**Status:** ready-for-agent

- [ ] On a page where the detector found no text, drawing a region and pressing Inpaint (page scope) cleans that region (test through the real stage with a user-origin region and an empty detector mask)
- [ ] Detector regions and pages without user regions behave exactly as before (existing inpainting tests still pass)
- [ ] Regions outside the run's scope (selection or entity scope) are not cleaned
- [ ] Paint mask mode: strokes clipped to the region; Apply uses the painted mask; Escape and an empty stroke cancel with no canvas edit left active
- [ ] The painted mask persists, loads from projects saved before it existed, is user-authored (a pipeline write is rejected, test through the real edit path), and is one undo step
- [ ] Moving or resizing a region clears its painted mask
- [ ] New strings in every locale; component tests cover the mode, clipping and apply
