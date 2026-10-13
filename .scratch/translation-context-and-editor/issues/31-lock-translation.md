# 31: Lock a translation

**What to build:** A hand-edited translation survives re-runs, but a generated one the user is happy with cannot be pinned. Add a per-box lock.
- A user-authored marker component on the text box (NOT `SHARED_WITH_PIPELINE`, so no pipeline run can add or remove it).
- Inspector: a lock toggle on the selected box (icon button, clear locked/unlocked state, accessible label). One undo step per toggle. Locked boxes show the lock in the Inspector only; no new canvas chrome.
- Page runs and bulk runs skip locked boxes: only the unlocked boxes of the page are translated. Locked boxes still go to the model as context with their current translation, the same way card 15 sends the page's other segments as context for a one-box run, so reuse that path (an entity-scoped run over the unlocked boxes) rather than adding a new one. A page whose boxes are all locked is skipped without error, and bulk runs count it in the existing skip toast.
- Translate on a locked box (one-box translate from tickets 15/28) is disabled with a hint to unlock first. Editing a locked box's text by hand is still allowed.
- Deleting a box deletes its lock; a locked box is not affected by the stale marker either way (stale stays a page-level flag).

Out of scope: lock all on a page, locking from the canvas, locking regions or the Term list.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Toggling the lock in the Inspector is one undo step and survives reload
- [ ] A full page run and a bulk run leave locked boxes untouched but send them as context
- [ ] One-box translate is disabled on a locked box
- [ ] A page with every box locked is skipped without error
- [ ] The lock component is user-authored (a pipeline run cannot clear it)
- [ ] Rust tests through the real run path (lock, run, text unchanged, others changed, undo), component test, nine locales
