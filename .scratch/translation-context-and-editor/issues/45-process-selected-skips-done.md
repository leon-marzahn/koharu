# 45: Process Selected Pages skips Done pages

**What to build:** Bulk runs (project-wide, Process Stale Pages) leave Done pages alone through `packages/koharu/lib/bulk.ts` (`bulkScope`, with the `skippedDone` toast). Process Selected Pages and the toolbar scopes do not, so a Done page in a selection still gets run. Make them behave the same.
- Route the selection through `bulkScope` so the skip rule lives in one place; update the comment at the top of `bulk.ts`, which says explicit selections still run Done pages.
- Skipping is announced with the existing toast. If every selected page is Done, nothing runs and the toast says so.
- One-page runs from the Inspector (Describe, Translate one box) are explicit actions on that page and are not touched.

Tests, written first: vitest for `bulkScope` with a selection containing Done and non-Done pages and one containing only Done pages; component test through the real Process menu for Process Selected Pages.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Process Selected Pages and the toolbar scopes skip Done pages and toast the count
- [ ] An all-Done selection runs nothing
- [ ] The skip rule has one home (`lib/bulk.ts`)
- [ ] vitest tests above pass
