# 42: Persist one-box translate explanations

**What to build:** Ticket 28 shows the model's explanation for a one-box translation but does not store it (it has a `ponytail:` comment naming this ceiling). Reloading or switching boxes loses it. Store it.
- A component on the text layer (or the entity that holds the translation; pick where `Translation` lives and say why) holding the explanation text and its authorship. It is generated data: normal pipeline authorship rules, so a later translation of that box replaces or clears it. A full-page run that retranslates the box must not leave a stale explanation behind; clear it.
- The Inspector panel reads the stored value; dismiss removes the component as one undo step. Hand-editing the translation does not clear it.
- Regenerate `protocol.ts`; remove the `ponytail:` comment and the transient client state it replaced.

Tests, written first: Rust (stored on one-box translate, replaced on a second, cleared by a full-page retranslate, survives hand edit, undo of dismiss); vitest through the real Inspector path (shows after reload of the scene, dismiss removes it).

**Blocked by:** 28

**Status:** ready-for-agent

- [ ] Explanation survives reload
- [ ] Retranslating the box replaces or clears it
- [ ] Dismiss is one undo step
- [ ] Rust and vitest tests pass; `protocol.ts` regenerated
