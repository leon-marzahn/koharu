# 30: Describe this page from the Inspector

**What to build:** The Scene Description (ticket 11) and the appearance proposals (ticket 12) only run inside Translate today. Add a way to run them for one page without translating, the way ticket 15 added a per-box Translate.

- A ghost button under the Scene Description field in the Inspector (same style as the per-box Translate), no new button in the top bar. It describes the current page only.
- It runs only the description request (page image plus Character sheet). It writes the page's Scene Description and the appearance proposals through the existing pending-character merge. It touches no translation, no stale marker, no page status.
- Disabled, with a hint, when no description model is set (the Description row in the model dropdown) or when the description is locked (ticket 31).
- The result replaces the unlocked description as one undo step. A failure is shown to the user (toast), not only logged.
- Backend: a new `Operation` (like `TranslateBox`) handled in the translation processor by running only its describe block. Do not duplicate the describe code.

Out of scope: describing several pages at once, a progress bar.

**Blocked by:** 31 Lock text

**Status:** done

- [x] The ghost button under the Scene Description field describes this page only; nothing is translated
- [x] Appearance proposals from the run appear as pending characters
- [x] Disabled with a reason when no model is set or the description is locked
- [x] A failure is shown, and a successful run is one undo step
- [x] Rust test (the operation runs describe only), component test through the real button path, nine locales (real translations)
