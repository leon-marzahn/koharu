# 04: Previous-page hints

**What to build:** Translating a page sends source/translation pairs from the last N translated pages before it in reading order, using current text so manual edits win. N defaults to 2 and is configurable in pipeline settings. Pages without translations are skipped and do not count toward N.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Hints appear in the translation request in reading order
- [ ] User-edited translations are what gets sent
- [ ] Untranslated pages are skipped and not counted
- [ ] N is configurable and defaults to 2
- [ ] Prompt-payload and stage tests cover the above
