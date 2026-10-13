# 22: Paint a mask for a region

**What to build:** A manually drawn region has no detector mask, so inpainting it today uses a plain padded rectangle (`process_region` in `crates/koharu-app/src/commands/canvas.rs`). That erases artwork behind text that is not in a flat bubble, and the Inpaint stage run from the dropdown ignores such regions entirely (it skips pages that already have a cleanup layer, and builds its mask only from the detector's text mask). Let the user paint the mask for a region themselves.

- Selecting a region shows a "Paint mask" control next to Re-OCR and Delete. It enters a brush mode scoped to that region: strokes are clipped to the region's bounds (padded a little), shown as an overlay, and committed with "Apply" (cancel with Escape).
- Applying runs the existing manual-mask inpainting path (the one the brush tool uses: `Processing.inpainting_mask` plus `Scope::Region` and `Operation::Only { Inpainting }`) with the painted mask instead of the rectangle.
- The painted mask is stored on the region (a revisioned component, `#[serde(default)]`, user-authored), so "Inpaint" on that region later reuses it, and moving or resizing the region does not silently discard it. Decide in the ticket whether the mask follows the region (stored relative to its box) or is cleared on resize; pick the simplest that does not leave a stale mask over the wrong pixels.
- The Inpaint stage, when it runs over a page that has user regions with stored masks, includes those masks in its scope (so re-running the stage does clean them). Regions without a stored mask are left alone.
- Draw region keeps cleaning with the rectangle by default; add a setting or a second step only if the rectangle turns out to be wrong too often in practice.

**Blocked by:** 20 (the Inpaint action it extends) and 14 (merged)

**Status:** needs-triage (design questions above)

- [ ] Paint mask mode on a selected region; strokes clipped to the region; Apply and Escape work and cancel leaves no canvas edit active
- [ ] Applying uses the stored painted mask through the manual inpainting path and merges into the cleanup layer
- [ ] The mask persists on the region, loads from projects saved before it existed, and is written as user authorship (test through the real edit path)
- [ ] Re-running Inpaint over the page re-cleans regions that have a stored mask and leaves others alone
- [ ] Moving or resizing a region follows the decision made in the ticket, with a test
- [ ] New strings in every locale; tests cover the mode, clipping, apply and the stage scope
