# 30: Describe pages without translating

**What to build:** The scene description (ticket 11) and the appearance proposals (ticket 12) only run inside a translation run today, so describing a page means translating it too. Add a run that only describes.
- A **Describe** button next to Translate and Inpaint in `InferenceControl.tsx` (same style, an icon that is not Play), honouring the same run scope (this page, selected pages, group, all). Disabled with a hint when no description model is set (the Description row in the same dropdown).
- It sends only the description request per page (page image plus Character sheet): writes the page's `SceneDescription` and the appearance proposals through the existing pending-character merge, and touches no translation, no stale marker, no page status.
- A page with a hand-written description (`Origin::User`) is skipped and counted in a toast, like the bulk skip of Done pages. A page with a generated description is described again, because pressing the button is the explicit rerun.
- A failed description for one page is reported (toast with count), not swallowed, and does not stop the other pages.
- Backend: a new `Operation` (like `TranslateBox`), handled in the translation processor by running only its describe block. Do not duplicate the describe code.
- The existing "clear the text in the Inspector to describe again" rule on full translate runs stays.

Out of scope: a stored per-page "described" status, a progress bar beyond the existing run progress.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Describe button runs only the description for the scoped pages; no translation is written
- [ ] Appearance proposals from those runs appear as pending characters
- [ ] User-written descriptions are kept and counted; generated ones are replaced
- [ ] No description model set: button disabled with a reason
- [ ] One page failing does not stop the rest, and the failure count is shown
- [ ] Rust tests (operation runs describe only, skips user-owned), component test through the real button path, nine locales
