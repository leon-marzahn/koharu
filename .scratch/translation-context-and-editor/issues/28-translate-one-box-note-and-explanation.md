# 28: Translate one box with a note, and explain the translation

**What to build:** On top of ticket 15's one-box translate, let the user steer a single translation and see why the model chose it. Example from the user: a sound effect `イリイラレ (iri irare)` was first read as wordplay on frustration; after explaining, the model found `入り入られ`, "entering and being entered", and the user settled on "Coming and Going". They want that exchange inside Koharu, with any model they configured (the Agent tab only supports Codex, and is not touched here).

- **Note.** Next to the one-box Translate button, a small popover with a text area ("Translate with note"). The note is one-shot: it is sent with that request only, and the popover keeps it until closed so it can be refined and re-run. It is not saved in the project.
- **Iteration.** The request for a box with a note also carries the box's current translation as "previous attempt", so the model can improve on it instead of starting over.
- **Explanation.** For a one-box request the model also returns a short `explanation` of why it translated the line that way, in the project's target language. It is shown in a dismissible panel by the box (the Inspector, next to the text). It is not stored in the project; reloading loses it (`ponytail:` comment naming that ceiling and the upgrade path, a stored component).
- **Robustness.** The translation is applied even when the explanation is missing, empty or `null` (model output may contain `null` where the schema says string). A full-page run is unchanged: no explanation requested, no extra tokens.
- Applying the translation stays one undo step and overwrites hand-edited text, exactly as in ticket 15.

Out of scope: storing explanations, chat history, a per-box "always include" note, Term list changes (a name learned from this can be added through the existing Term list).

**Blocked by:** 15 Translate one box

**Status:** ready-for-agent

- [ ] A note typed in the popover reaches the prompt for that box only, with the previous translation as context
- [ ] The model's explanation appears next to the box and can be dismissed
- [ ] A missing, empty or `null` explanation still applies the translation
- [ ] Full-page runs are byte-for-byte the same request as before
- [ ] Undo restores the previous text in one step
- [ ] Prompt and parse tests (Rust), component tests through the real UI path (vitest), nine locales for the new strings
