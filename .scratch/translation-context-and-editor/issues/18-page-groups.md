# 18: Page groups

**What to build:** Pages can be put into named groups (for example story arcs) in the page rail. Groups are collapsible, can be renamed and deleted (deleting a group keeps its pages), and pages move between groups by drag or a context menu. A page belongs to at most one group; ungrouped pages stay where they are. The run scope selector gains "This group" next to the current page / all pages options.

Groups are organisation only: the Character sheet, Term list and hints stay project-wide (spec decision), so a group never changes what the model sees.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Create, rename, delete a group; deleting keeps its pages
- [ ] Move a page into, out of, and between groups; page order within the rail is preserved
- [ ] Groups collapse and expand, and the state survives reopening the project
- [ ] Run scope "This group" runs the chosen stages on exactly that group's pages
- [ ] Projects saved before groups existed load unchanged (`#[serde(default)]`, revisioned component)
- [ ] Stage and component tests cover grouping, scope and loading an old project

**Open question (not in v1):** per-group Character sheet / Term list, if arcs need different names. Add only if project-wide entries start to clash.
