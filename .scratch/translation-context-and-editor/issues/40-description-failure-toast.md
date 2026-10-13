# 40: Description failure during Translate toasts

**What to build:** When the scene description fails inside a Translate run, `stages/translation.rs` (around line 558) only logs `tracing::warn!` and translates without it. The Inspector's Describe button shows a toast on failure; Translate must too.
- Translation still goes on without the description (a failed description only costs context). Keep that.
- Surface the failure to the user as a toast with the error message, once per page. Reuse the existing progress/event path from ticket 33 (`Progress::Describing`) or the existing error toast path; add a variant only if neither can carry it. Update the `ponytail:` comment there, it names the old behaviour.
- A bulk run over many pages must not spam: one toast per failed page is fine, but say in your summary how many you can get in a 20-page run.

Tests, written first: Rust test that a failing describe call still produces the translation patch and emits the failure event; vitest for the toast through the real event handler.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] A description failure during Translate shows a toast with the reason
- [ ] The translation still completes
- [ ] Rust and vitest tests pass; nine locales if a new string is needed
