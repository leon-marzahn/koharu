# 15: Translate one box

**What to build:** A translate button on the selected box translates only that box, using the same context pipeline as a full run (the page's other segments, Character sheet, Term list, hints, and scene description when present). It overwrites even hand-edited text, with undo. Built on the shared request builder, so the extra context sources are added to the same list rather than re-implemented.

**Blocked by:** 04 Previous-page hints, 05 Character sheet (manual), 06 Term list (manual)

**Status:** ready-for-agent

- [ ] Only the selected segment is translated
- [ ] Request contains the same context sources as a full run
- [ ] Hand-edited text is overwritten and undo restores it
- [ ] Scene description is included when one exists
- [ ] Stage and component tests cover scope, context and undo
