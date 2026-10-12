# 08: Proposals: updates to existing entries

**What to build:** The model can propose changes to existing characters and terms (for example a newly noticed speech quirk). Pending update rows show per-field old → new diffs and use the same approve / edit / reject flow.

**Blocked by:** 07 Proposals: new entries and review

**Status:** ready-for-agent

- [ ] Output schema accepts updates by entry id with only changed fields
- [ ] Updates for unknown ids are dropped
- [ ] Panel shows per-field diffs on update rows
- [ ] Approving applies only the changed fields
- [ ] Tests cover schema, application and diff display
