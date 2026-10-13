# 29: Mark a page as up to date

**What to build:** A page marked stale (its Term list or Character sheet changed after it was translated) can only be cleared today by re-running its translation. Let the user dismiss the marker instead.
- Backend: a command that removes `StalePage` from one page or from a list of pages, as one undoable commit. `StalePage` is `SHARED_WITH_PIPELINE`, so the authorship check already allows a user edit. Pages that are not stale are skipped without error.
- Page rail (`PageRail.tsx`): each stale page gets a "Mark as up to date" action (its stale badge or the page's menu, whichever the rail already uses for per-page actions), and the "Retranslate N stale pages" row (around line 319) gets a sibling "Mark all as up to date". The bulk action goes through the same command.
- A page translated again later is marked stale again by the existing rules; this does not change them.

Out of scope: a project setting to stop pages from being marked stale, and per-term "does not affect old pages" flags.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] One page can be marked up to date from the rail; the badge disappears
- [ ] "Mark all as up to date" clears every stale page, one undo step
- [ ] Undo brings the marker back
- [ ] A page that was not stale is a no-op
- [ ] Rust test for the command (single, many, non-stale page, undo); component test through the real rail path; nine locales for the new strings
