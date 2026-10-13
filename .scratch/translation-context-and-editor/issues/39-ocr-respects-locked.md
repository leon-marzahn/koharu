# 39: OCR respects Locked

**What to build:** Bulk OCR passes re-read every text region. `crates/koharu-pipeline/src/stages/ocr.rs` (the target loop around line 126) only skips source text whose origin is `User`; it never checks `Locked`, while translation does (`stages/translation.rs`, `translatable_boxes`). A locked box must keep its source text through every OCR run.
- Find which entity carries `Locked` for a box (the text layer, as in `translatable_boxes`) and skip the OCR target when it is locked. Follow the relation from the recognised content to the layer; do not add a second lock concept.
- Explicit one-box Re-OCR on a locked box: report in your summary what happens today and what you chose (skipping silently is wrong; a locked box that the user explicitly re-reads should either run or say why not). Do not invent UI; if it needs a toast or message, use an existing one.

Tests, written first: Rust test that a locked box's `SourceText` is unchanged after an OCR stage run while an unlocked sibling is updated; a locked box with generated (non-User) source text is also left alone.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] OCR never rewrites the source text of a locked box
- [ ] Unlocked boxes on the same page still update
- [ ] Rust test above passes
