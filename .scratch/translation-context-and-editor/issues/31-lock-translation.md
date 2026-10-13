# 31: Lock text so Translate cannot overwrite it

**What to build:** One protection rule replaces the implicit "hand-edited text is kept": a **lock**. Translate redoes everything that is not locked; editing something by hand locks it.

- **Component:** a user-authored `Locked` marker (NOT `SHARED_WITH_PIPELINE`, so no pipeline run can add or remove it), put on a text box or on a page's Scene Description.
- **Auto-lock:** a hand edit of a box's translation, or of a page's Scene Description, adds `Locked` in the same commit, so edit plus lock is one undo step. Do it in the backend commands that perform those edits, not in the frontend.
- **Lock/unlock by hand:** a lock toggle (icon button, visible locked/unlocked state, accessible label, one undo step) on the selected box in the Inspector and next to the Scene Description field. Editing a locked field by hand is still allowed.
- **Translate** (top bar, page and bulk runs): translates every unlocked box, whether or not it was hand-edited before, and describes the page again unless its description is locked. This replaces the old rules "keep hand-edited translations" (`write_translations`, `overwrite_user`) and "describe only pages without a description" (`should_describe`, merged in `f29f37d`). Locked boxes still go to the model as context with their current translation, the same way card 15 sends the other segments as context; reuse that path. A page whose boxes are all locked is skipped without error and counted in the existing bulk skip toast (its description is still redone unless locked).
- **One-box translate** (tickets 15/28) is disabled on a locked box, with a hint to unlock first.
- **Existing projects:** boxes and descriptions the user already hand-edited have no `Locked` marker. Backfill once on project open: lock every text box whose translation has `Origin::User` and every page whose Scene Description has `Origin::User`. Guard it with a project-level marker so it runs exactly once (otherwise unlocking and reopening would re-lock). Backfill is one commit.
- Deleting a box deletes its lock. Stale stays a page-level flag.

Out of scope: lock-all on a page, locking from the canvas, locking regions or the Term list.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Hand-editing a box or a Scene Description locks it; edit plus lock is one undo step
- [ ] The toggle locks and unlocks, survives reload, one undo step each
- [ ] Translate redoes unlocked boxes (including previously hand-edited ones) and the unlocked description; locked ones are untouched but sent as context
- [ ] One-box translate is disabled on a locked box
- [ ] A page with every box locked is skipped without error
- [ ] The lock is user-authored: a pipeline run cannot add or clear it
- [ ] Backfill locks existing hand-edited text exactly once
- [ ] Rust tests through the real run path (edit locks, run, locked text unchanged, unlocked changed, undo, backfill once), component tests, nine locales (real translations)
