# 08: Proposals: updates to existing entries

**What to build:** The model can propose changes to existing characters and terms (for example a newly noticed speech quirk). Pending update rows show per-field old → new diffs and use the same approve / edit / reject flow.

**Blocked by:** 07 Proposals: new entries and review

**Status:** done

- [x] Output schema accepts updates by entry id with only changed fields
- [x] Updates for unknown ids are dropped
- [x] Panel shows per-field diffs on update rows
- [x] Approving applies only the changed fields
- [x] Tests cover schema, application and diff display
