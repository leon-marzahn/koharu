# 09: Learn from corrections

**What to build:** Lines you corrected by hand are flagged user_corrected in the hints. The translation call may propose Term list or Character sheet entries from the difference, through the same approval flow.

**Blocked by:** 04 Previous-page hints, 07 Proposals: new entries and review

**Status:** ready-for-agent

- [ ] Hints carry a user_corrected flag for hand-edited lines
- [ ] The prompt tells the model to treat those lines as authoritative and may propose entries from them
- [ ] Proposals from corrections arrive as pending entries
- [ ] Prompt-payload and stage tests cover the above
