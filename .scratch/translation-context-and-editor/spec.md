---
status: ready-for-agent
scope: local fork
---

# Translation context and editor improvements

## Problem Statement

Translating a long work page by page, the model forgets what it learned earlier: a character's corrected name, that one character ends sentences with "-nano" and another with "-dazo", who is speaking to whom. Every page starts cold. Most vision-capable translation models refuse adult content, so the page image often can't be used for context either. Corrections the user makes by hand don't carry forward.

The editor slows careful, per-page work down: no keyboard nudging, no ±1 font size, detection regions are invisible and uneditable (a page whose text layers were deleted can't be re-detected from the UI), no way to translate a single box, layers can only be reordered one step at a time, no copy/paste of boxes or styles, no snapping of text to bubble centers.

## Solution

**Translation track.** Each project gets a **Character sheet** and a **Term list** that travel with every translation request, plus automatic **Previous-page hints** from the pages just before. The translation model proposes new and updated entries, including ones learned from the user's corrections; the user approves, edits, or rejects them in a Project panel. Editing an entry marks affected pages stale for retranslation. An optional **Scene description** stage uses a separately chosen (NSFW-capable) vision model to describe each page and identify characters by appearance, and that description is sent to translation.

**Editor track.** Arrow-key nudging, font size steppers, a center mark on the selected box, drag-to-reorder layers, a Regions view to move/resize/delete/draw detection regions, a per-box translate button, copy/paste of whole boxes and of styles, and snapping a box's center to its bubble's center.

## User Stories

### Character sheet
1. As a translator, I want a per-project Character sheet, so that names and speech habits stay consistent across the whole work.
2. As a translator, I want each character entry to hold a source name, target name, speech style, appearance, and notes, so that the model knows how each character sounds and looks.
3. As a translator, I want to add, edit, and delete characters by hand, so that I stay in control even without proposals.
4. As a translator, I want every Character sheet entry sent with every translation request, so that a character's voice is kept even on pages where they aren't named.
5. As a translator, I want the Character sheet sent to the Scene description model, so that it can name characters by appearance instead of "a girl with twin tails".
6. As a translator, I want the Character sheet saved inside the project, so that it survives restarts and travels with the project file.
7. As a translator, I want Character sheet edits to be undoable, so that a mistaken edit is cheap.

### Term list
8. As a translator, I want a per-project Term list of source → target terms with an optional note, so that recurring names, places, and words render the same everywhere.
9. As a translator, I want only Term list entries whose source appears in the page's segments sent, so that a long list doesn't crowd the prompt.
10. As a translator, I want to add, edit, and delete terms by hand, so that I can fix terminology immediately.
11. As a translator, I want the Term list saved inside the project, so that it travels with the work.

### Previous-page hints
12. As a translator, I want source/translation pairs from the last N translated pages sent automatically, so that dialogue continues naturally across page breaks.
13. As a translator, I want N to default to 2 and be configurable in pipeline settings, so that I can trade context for cost.
14. As a translator, I want hints to use the current translation, so that my manual corrections are what the model sees.
15. As a translator, I want pages without translations skipped and not counted toward N, so that empty pages don't waste the window.
16. As a translator, I want lines I corrected marked as user-corrected in the hints, so that the model treats them as authoritative.

### Proposals
17. As a translator, I want the translation call to propose new characters and terms it encounters, so that I don't have to build the lists from scratch.
18. As a translator, I want the translation call to propose updates to existing entries (e.g. a newly noticed speech quirk), so that entries improve over time.
19. As a translator, I want the translation call to propose entries learned from differences between its output and my corrected lines, so that a fix I make once is remembered.
20. As a translator, I want the Scene description model to propose appearance for characters it can't match, so that new characters get identifiable.
21. As a translator, I want proposals for the same character from the description and translation models merged into one pending entry, so that I review each character once.
22. As a translator, I want never to be re-proposed an entry I rejected, so that I'm not nagged by the same bad suggestion every page.
23. As a translator, I want rejected entries kept in a collapsed list where I can restore them, so that a misclick is recoverable.
24. As a translator, I want pending entries excluded from prompts by default, so that unreviewed guesses don't spread.
25. As a translator doing a batch run, I want a project setting "Use pending entries in prompts" that sends pending entries marked tentative, so that later pages still benefit before I've reviewed.
26. As a translator, I want rejecting an entry to stop it being sent immediately, so that a wrong guess doesn't keep influencing output.
27. As a translator, I want proposals never to apply themselves, so that nothing changes my lists without approval.

### Project panel
28. As a translator, I want a Project panel with Characters and Term list tabs, so that I can review and edit both in one place.
29. As a translator, I want approved and pending entries listed separately with Approve / Edit / Reject on pending rows, so that review is quick.
30. As a translator, I want proposed updates shown as per-field old → new diffs, so that I know exactly what changes.
31. As a translator, I want a pending-count badge on the panel's entry point, so that I notice proposals without hunting for them.
32. As a keyboard user, I want the panel and its actions fully keyboard-operable with labelled controls, so that review doesn't require a mouse.

### Stale pages
33. As a translator, I want editing a name or term to mark pages whose source contains that name or term as stale, so that I know what to retranslate.
34. As a translator, I want editing a character's speech style, appearance, or notes to mark pages containing that character's source name as stale, so that style fixes get applied where the character likely speaks.
35. As a translator, I want stale pages visibly marked in the page rail, so that I can see them at a glance.
36. As a translator, I want a "Retranslate N stale pages" action, so that I can apply fixes in one go.
37. As a translator, I want a "Retranslate all pages" action, so that I can cover cases the stale proxy misses.
38. As a translator, I want nothing to retranslate automatically, so that I control cost and never lose work silently.
39. As a translator, I want retranslation to keep translations I edited by hand, as it does today, so that stale retranslation never destroys my corrections.

### Scene description
40. As a translator of adult manga, I want to choose a separate description model from any configured provider, so that I can use a model that accepts sexual content while translating with another.
41. As a translator, I want the description stage to run before translation, once per page, so that translation has scene context.
42. As a translator, I want the description to cover who's present, what's happening, and the mood, naming characters from the Character sheet where it can, so that the translator knows who speaks to whom.
43. As a translator, I want the description stored on the page and editable, so that I can correct a misreading.
44. As a translator, I want the description reused until I rerun it, so that I don't pay for it twice.
45. As a translator, I want the stage skipped when no description model is configured, so that today's behavior (attaching the image to vision-capable translation models) is unchanged.

### Editor
46. As an editor, I want arrow keys to move the selected box by 1 page pixel and Shift+arrow by 10, so that I can place text precisely.
47. As an editor, I want a burst of consecutive nudges to be one undo step, so that undo isn't tedious.
48. As an editor, I want − / + buttons beside the font size dropdown, so that I can step size by 1.
49. As an editor, I want ↑/↓ in the font size field to change it by 1, so that I can adjust from the keyboard.
50. As an editor, I want a faint "+" at the center of the selected box, never rendered into output, so that I can judge centering.
51. As an editor, I want to drag layers to reorder them, so that big moves are one gesture.
52. As a keyboard user, I want the existing ↑/↓ reorder buttons kept, so that reordering stays accessible.
53. As an editor, I want a toggleable Regions view showing detection regions, so that I can see what detection found.
54. As an editor, I want to move and resize a region and have its text box refit, so that I can fix a bad detection.
55. As an editor, I want deleting a region to remove its text box, so that no invisible leftovers block re-detection.
56. As an editor, I want to draw a new region and have OCR and translation run on just that region, so that I can recover text detection missed.
57. As an editor, I want a translate button on a single selected box, so that I can retry one line without touching the rest.
58. As an editor, I want single-box translation to send the page's other segments, Character sheet, Term list, hints, and scene description as context, so that quality matches a full run.
59. As an editor, I want single-box translation to overwrite even hand-edited text, with undo, because I asked for it explicitly.
60. As an editor, I want Cmd+C / Cmd+V to copy whole text boxes (source, translation, style, size, position) within and across pages, so that I can reuse lettering for undetectable text.
61. As an editor, I want a pasted box offset 10 px on the same page and at the same position on another page, so that it's visible and aligned.
62. As an editor, I want copied boxes not to carry their detection region, so that pastes don't create phantom regions.
63. As an editor, I want Copy style (Cmd+Shift+C) and Paste style (Cmd+Shift+V) for font, size, color, stroke, alignment, and writing mode, so that a box on a newly drawn region can match its neighbours.
64. As an editor, I want Paste style to apply to every selected box, so that I can restyle several at once.
65. As an editor, I want Copy/Paste style buttons in the Inspector too, so that the feature is discoverable.
66. As an editor, I want a dragged box's center to snap to its bubble's center within about 8 screen px with a faint guide, so that centering is effortless.
67. As an editor, I want holding Cmd to disable snapping while dragging, so that I can place a box just off-center.

## Implementation Decisions

### Build order
Each step is usable on its own:
1. Editor quick wins: nudge, font ±1, center mark, drag reorder.
2. Context plumbing: Previous-page hints, Character sheet and Term list with manual editing, Project panel.
3. Proposals: new and update proposals, pending/tentative handling, rejections, learning from corrections, stale marking.
4. Scene description stage, including appearance proposals.
5. Region editing and single-box translation.
6. Copy/paste box and style, center snap.

### Storage
- Character sheet, Term list, pending proposals, rejections, and the "Use pending entries in prompts" setting are **project-level components** in the scene document (the project already supports project-scoped components). They save with the project, participate in undo, and need no sidecar file.
- Scene description is a **page-level component** with the same origin/provenance model used for generated vs user text, so a user-edited description isn't overwritten by a rerun.
- Stale is **derived page state** recorded when an entry is edited (the set of affected pages), cleared when the page is retranslated.

### Translation request
- The translation request gains: Character sheet entries, matching Term list entries, Previous-page hints (the existing, currently unpopulated, context field, extended with a user-corrected flag), the page's scene description, and tentative pending entries when enabled.
- Term matching is a plain substring match of each term's source against the page's segment sources.
- The structured output schema gains an optional `proposals` field next to `translations`: new characters, new terms, and updates to existing entries (by entry id, with only changed fields). Malformed or empty proposals never fail the translation; translations are applied regardless.
- Proposals whose source matches a rejected entry are dropped before storage.
- The translation stage collects hints from the last N translated pages before the current one in reading order, using current translations (user text wins).
- Single-box translation reuses the same request builder with the page's other segments supplied as context and only the selected segment translated.

### Scene description stage
- A new pipeline stage ordered after OCR and before translation, with its own model selection reusing the existing provider/model selection mechanism (any configured provider, including OpenAI-compatible with custom headers).
- Input: page image and Character sheet. Output: a short description and optional appearance proposals for unmatched characters.
- Skipped when no description model is configured, or when the page already has a description (rerun is explicit).
- Appearance proposals merge into a pending character proposal from the translation call when they refer to the same new character; otherwise they create a pending character with appearance only.

### Stale marking
- Name or term edit → pages whose source text contains the old or new source string.
- Speech style / appearance / notes edit → pages whose source contains the character's source name.
- Retranslate actions run the translation stage on stale pages (or all pages); the existing rule of never overwriting user-edited translations stays.

### Editor
- Nudge moves in page pixels; consecutive nudges within a short idle window coalesce into one undo step.
- Font steppers change size by exactly 1 and clamp to the existing allowed range.
- Center mark is an overlay-only element, excluded from rendering and export.
- Drag reorder follows the native drag pattern already used for page reordering; existing ↑/↓ buttons remain.
- Regions view is an overlay toggle. Region move/resize refits text boxes related to the region. Region delete removes the region with cascade, taking its text box. A drawn region is a user-origin text region; OCR and translation can be scoped to it via the existing selected-layers processing scope.
- Copy box serializes the text layer (source, translation, style, geometry) to an in-app clipboard; paste creates a new user-origin text layer without a region relation.
- Copy style is a separate in-app clipboard holding typography and layout fields only.
- Snap targets the center of the bubble region containing the box's center; threshold is in screen pixels so it feels the same at every zoom. Cmd held disables it (Alt is already used by the canvas).

## Testing Decisions

Good tests exercise external behavior through the highest existing seam and assert on outputs (prompt payloads, committed scene state, rendered UI), not on internal helpers.

Seams, in order of preference:
1. **Translator prompt payload**: build a translation request and assert on the serialized prompt input and output schema. Covers Character sheet inclusion, Term list substring filtering, hints with user-corrected flags, tentative pending entries, scene description, and parsing of `proposals` (including malformed proposals not failing translation). Prior art: the existing prompt payload tests in the translator crate.
2. **Pipeline stage on an in-memory session**: create a project session, add pages and components, run a stage's process step, commit the patch, and assert on the resulting snapshot. Covers hint collection across pages, proposal storage and rejection filtering, stale marking, scene description stage skip/run rules, region delete cascade, and single-box translation scope. Prior art: the detection stage tests that build pages on a session (including the orphaned-region test added in this fork).
3. **Frontend component tests (vitest)**: covers nudge and undo coalescing, font steppers, Project panel approve/edit/reject flows and badge, drag reorder, copy/paste box and style, snap threshold and Cmd override, and the center mark's absence from export. Prior art: the existing editor component and canvas tests.

Remote model calls are never hit in tests; the prompt seam stops at the request payload and stage tests use a stub translator.

## Out of Scope

- Splitting a text layer into per-character layers (upstream issue #974). Revisit after copy/paste and reorder land.
- A global (cross-project) Term list.
- A whole-work study note produced by reading every page.
- A separate correction/review pass that proposes line fixes.
- LLM explanation of translations and retry-with-correction; when specced, it gets "add to Character sheet / Term list" shortcuts.
- Upstreaming; this is a local fork, and features may be offered upstream individually later.

## Further Notes

- The fork already carries: `stream: false` on OpenAI-compatible requests, configurable request headers for OpenAI-compatible providers, and a detection fix so pages with only orphaned text regions are re-detected.
- Previous-page hints and Term list stay separate concepts: hints are automatic short-term continuity of whole lines; the Term list is reviewed long-term terminology. Corrections bridge them only via proposals.
- Prior art reviewed but not taken at face value: the glossary/work-note/correction design described in upstream issue #339 by Luixmc.
