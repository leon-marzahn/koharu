# 11: Scene description stage

**What to build:** An optional pipeline stage before translation, once per page, with its own model chosen from any configured provider. It describes who is present, what is happening and the mood, using the Character sheet to name characters. The description is stored on the page, editable, reused until you rerun it, and sent with translation. Skipped when no description model is set, leaving today's behavior unchanged.

**Blocked by:** 05 Character sheet (manual)

**Status:** ready-for-agent

- [ ] Separate description model selection in settings
- [ ] Stage runs once per page and skips pages that already have a description unless rerun is requested
- [ ] Character sheet is sent to the description model
- [ ] Description is editable and a user edit is not overwritten by a rerun
- [ ] Translation request includes the description
- [ ] Stage is skipped when no model is configured
- [ ] Stage tests with a stub model cover run, skip and reuse
