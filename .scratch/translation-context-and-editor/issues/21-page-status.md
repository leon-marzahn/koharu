# 21: Page status

**What to build:** Each page has a workflow status the user sets by hand: Todo, In progress, Done. New pages are Todo. It is a label for the user's own workflow ("this translation is perfect, leave it alone"), separate from the computed Stale marker from ticket 10, and the two can show together.

- The status is set from the page rail (context menu and the page's row), shows as a small marker on each page, and the rail can be filtered by status.
- Stored per page as a revisioned component. Pages from projects saved before this existed read as Todo.
- The pipeline never writes it, and nothing changes it automatically (editing a Done page does not reset it).
- Done pages are skipped by bulk runs ("All pages", "Retranslate N stale pages", "Retranslate all pages", and the "This group" scope once ticket 18 exists). Running on the current page, or on pages the user selects by hand, still works. The bulk action says how many Done pages it skipped.

**Blocked by:** none (ticket 18 only adds one more scope to the skip rule)

**Status:** ready-for-agent

- [ ] Set a page's status from the rail; the marker shows and survives reopening the project
- [ ] Filter the rail by status
- [ ] Projects saved before this load with every page as Todo (`#[serde(default)]` or an absent component, with a load test)
- [ ] Bulk runs skip Done pages and report the count; an explicit page selection runs them anyway
- [ ] A page can be Done and Stale at once; both markers are visible
- [ ] Setting a status is one undo step
- [ ] The pipeline cannot overwrite it: a test through the real edit path with a pipeline write confirms the authorship check applies
- [ ] New strings are in every locale; component tests cover set, filter and the skip rule

**Open question:** whether Done should also stop the stale marker from being set. Left as is: the marker is information, the skip rule is what protects the page.
