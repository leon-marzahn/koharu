# 07: Proposals: new entries and review

**What to build:** The translation call can return proposals for new characters and terms. They appear as pending entries in the Project panel, where you approve, edit or reject them. Pending entries are excluded from prompts unless the project setting "Use pending entries in prompts" is on, in which case they are sent marked tentative. Rejected proposals are remembered and never proposed again for that source. A badge shows the pending count.

**Blocked by:** 05 Character sheet (manual), 06 Term list (manual)

**Status:** ready-for-agent

- [ ] Translation output schema accepts optional proposals; malformed or empty proposals never fail the translation
- [ ] Proposals never apply themselves
- [ ] Approve, edit and reject work; rejections are kept in a collapsed list and restorable
- [ ] A rejected source is dropped from future proposals
- [ ] Pending entries are excluded by default and sent as tentative when the setting is on
- [ ] Badge shows pending count
- [ ] Prompt-payload, stage and component tests cover the above
