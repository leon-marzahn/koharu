# 41: One compositor for the cleaned page

**What to build:** Ticket 32 sends the cleaned page to the describe call. The overlay logic that composites the page's Cleanup layer over the source now exists twice: once in the inpainting stage (`stages/inpainting.rs`, `prepare`, `image::imageops::overlay(&mut composite, cleanup, 0, 0)`) and once in the describe path. Make one function own it and have both call it.
- Check whether the PSD export composes the same layers, and whether it agrees (visibility, order, opacity). Report the result; fix a disagreement only if it is a one-line change, otherwise write it up.
- No behaviour change for inpainting or describe.

Tests: existing inpainting and describe tests must pass; add one test that the shared function composites a cleanup layer over a source pixel-for-pixel.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] One function composites the cleaned page; inpainting and describe both call it
- [ ] PSD export compared and the result reported
- [ ] Existing tests pass, one new compositing test
