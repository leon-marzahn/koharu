# 14: Regions: delete and draw

**What to build:** Deleting a region also removes its text box, so nothing invisible is left to block re-detection. Drawing a new region creates a user region and runs OCR and translation on just that region.

**Blocked by:** 13 Regions view: show, move, resize

**Status:** ready-for-agent

- [ ] Deleting a region cascades to its text box and is undoable
- [ ] Drawing a region creates a user-origin text region
- [ ] OCR and translation run only on the drawn region
- [ ] Other regions and text are untouched
- [ ] Stage and component tests cover the above
