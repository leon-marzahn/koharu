# 23: Re-painting a region mask must work

Follow-up to 22. Three problems found by using it.

**1. Re-painting does not change the cleanup layer.** Manual inpainting (`inpainting.rs`, `prepare`) composites the existing cleanup layer over the source and inpaints inside the mask, then merges the result into the existing cleanup layer. A region that was first cleaned with the 2 px box (Draw region) keeps those smeared pixels outside a smaller painted mask, and re-inpainting an already-clean area changes nothing. Applying a painted mask must replace the clean for that region: inside the region's padded box (`padded_box`), restore the original page pixels (clear the cleanup layer there) first, then inpaint only the mask. Carry the box on `InpaintingMask` (new optional field) so the brush tool and other callers behave as before.

**2. Authorship conflict after resizing a region.** Repro: draw a region, let OCR and translation run, resize the region (`setRegionGeometry`), then paint a mask. Write the failing test first, through the real commands in `crates/koharu-app` (`draw_region`, `process_region`-like pipeline output, `set_region_geometry`, `set_painted_mask`, then the manual inpaint commit), find which `Error::Authorship` fires and fix the cause, not the symptom. Also: `processing failed` errors log only the outermost context (the log shows `flux2-klein failed` with no cause). Log and report the full cause chain (`{error:#}`), in `crates/koharu-app/src/commands/processing.rs` and wherever the error is sent to the UI.

**3. Paint mode shows nothing that is saved, and cannot undo.**
- All strokes are sent as one point list, so lifting the pen and pressing elsewhere draws a connecting line. Send strokes separately (`Vec<Vec<Point>>`) and paint each on its own.
- Entering Paint mode shows the region's stored mask as an overlay (add a way to read it, for example `get_painted_mask(region) -> Option<bytes>`), plus the new strokes in progress.
- Undo (button, and Cmd/Ctrl+Z while in the mode) removes the last new stroke. "Clear" drops the stored mask and new strokes, so Apply stores only what is painted afterwards.
- Apply stores the displayed mask (stored mask unioned with the new strokes, or only the new strokes after Clear) and re-cleans as in 1. Escape discards the new strokes and changes nothing. The stored mask replaces, not appends to, the previous one.

**Blocked by:** none (22 is merged)

**Status:** done

- [ ] Applying a smaller mask on a region first cleaned with the box restores the original pixels inside the box but outside the mask (test on the real stage and cleanup layer)
- [ ] The brush tool and the dropdown Inpaint behave as before (existing tests pass)
- [ ] Resize a region with a translated text box, then paint a mask: no authorship error, with a test through the real edit path
- [ ] Errors show their full cause chain in the log and the UI
- [ ] Separate strokes are not joined; Paint mode shows the stored mask; undo, clear and Escape work; Apply keeps what is displayed; the stored mask survives reloading the project
- [ ] New strings in every locale; component tests cover the mode
