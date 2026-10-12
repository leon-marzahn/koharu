# 14: Regions: delete and draw

**What to build:** Deleting a region also removes its text box, so nothing invisible is left to block re-detection. Drawing a new region creates a user region and runs OCR and translation on just that region.

**Blocked by:** 13 Regions view: show, move, resize

**Status:** done

- [x] Deleting a region cascades to its text box and is undoable
- [x] Drawing a region creates a user-origin text region
- [x] OCR and translation run only on the drawn region
- [x] Other regions and text are untouched
- [x] Stage and component tests cover the above
