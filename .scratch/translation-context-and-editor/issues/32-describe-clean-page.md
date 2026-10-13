# 32: Describe the page without its text

**What to build:** The description call sends the original page, text included (`input.images.get(.., "source")` in `describe`, `crates/koharu-pipeline/src/stages/translation.rs`). The model then summarises the dialogue instead of describing the scene (the prompt already forbids transcribing it, and the model ignores that). Send the page with its text cleaned out instead.

- **Order:** inpainting does not read the translation (its mask comes from detection or the painted mask), so it can run first. In `scheduler.rs`, `prerequisite()` returns one stage; Translation waits for OCR only. Make Translation also wait for Inpainting when both stages are in the run, so a Full run goes Detection, OCR, Inpainting, Translation. Pages still overlap in the pipeline. A page whose inpainting is skipped (already done) or fails must NOT block translation: translation proceeds and the description falls back to the original page. Check what the scheduler does today when a prerequisite fails or is skipped and cover it with a test.
- **Image:** when the page has cleanup layer(s) (`RasterLayerKind::Cleanup`, a full-page transparent layer composited above the source), send the source with those layers composited. Otherwise send the source. Find how the cleanup layer's pixels are stored and reuse an existing compositor (the renderer or the PSD export), do not write a second one. Paint layers are not part of the cleaned page.
- **Prompt:** in the describe prompt (`crates/koharu-translator/src/prompt.rs`, near "do not translate or transcribe the dialogue"), also say to ignore all text, speech bubbles and sound effects, and to describe only who is there, where, what they are doing and their expressions.
- **Used by:** the describe block inside Translate and `Operation::DescribePage` (ticket 30). Lone Translation or DescribePage runs do not inpaint; they use the page's existing cleanup layer if there is one.
- **Unchanged:** the translation request keeps sending the original page. Inpainting skip rules and the forced lone-inpaint rule stay as they are.

Out of scope: blanking text regions on pages that have not been cleaned; a setting to choose the image.

**Blocked by:** 30 Describe this page (merged)

**Status:** done

- [x] A Full run orders Detection, OCR, Inpainting, Translation for a page; a failed or skipped inpainting does not block translation
- [x] The description request carries the source with the cleanup layers composited when they exist, else the source
- [x] The describe prompt tells the model to ignore text and describe only the visuals
- [x] The translation request still carries the original page
- [x] Scheduler test for the new prerequisite and the failure/skip cases; a test that the description image differs from the source when a cleanup layer exists and equals it when none does
