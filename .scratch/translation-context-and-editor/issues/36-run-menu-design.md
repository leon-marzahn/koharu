# 36: Design: one coherent way to run the pipeline

**This is a design ticket. Write no code and change no source file.** The deliverable is one document, `.scratch/translation-context-and-editor/design/36-run-menu.md`, that the user will read and pick an option from. Do not create other cards.

**Problem:** The pipeline can be started from five places that grew one at a time and do not describe the same model. Scope (which pages) and stage (which work) are mixed up, "Run X" does not say what it covers, and Describe has no bulk action even though it is now part of Translate.

## Current state (verified in the code; re-check every line before you rely on it)

| Where | Items | What it sends |
| --- | --- | --- |
| Title bar menu "Process" (`app/TitleBar.tsx` ~198-230) | Process Project | `full`, every page |
| | Process Selected Pages | `full`, the selected pages |
| | Process Selected Layers | `stages [ocr, translation]` on the selected entities |
| | Run Detection / OCR / Inpainting (`menu.runPhase`, one per phase; check whether Translation is listed) | `through <stage>` over the whole project: it runs every stage up to and including that one |
| Editor toolbar "Run" (`editor/InferenceControl.tsx`) | Run | `full` with the scope from its settings popover (current page / selected pages / entire project) |
| | Translate, Inpaint buttons | `only translation`, `only inpainting` with the same scope |
| | settings popover | scope, translation/description/OCR/inpaint models, output |
| Page rail (`editor/PageRail.tsx` ~222-355) | Retranslate N stale pages, Dismiss stale, Retranslate all pages | `only translation` on the stale pages / all pages |
| Inspector (`editor/Inspector.tsx`) | Translate (one box), Translate with note, Describe this page | one-box `process` with entity scope; `DescribePage` |
| Region tools | Re-OCR region and the drawn-region process | `process_region` |

Facts the design must respect (read the code to confirm):
- Stages are detection, ocr, inpainting, translation. The description runs inside the translation stage; only `Operation::DescribePage` runs it alone, for one page.
- Translation waits for OCR and Inpainting when both are in the run. A stage needs the earlier stages' results; `through` exists for that reason. Say what a single-stage action does when a prerequisite is missing.
- Locks: Translate skips locked text boxes and a locked description. Every run does, with no setting.
- Bulk runs skip pages already Done and show a toast with the count; a lone `only inpainting` run is forced.
- Stale pages are marked by character/term edits and never rerun on their own.

## What the user asked for

- The Process menu should offer the same processing actions as the toolbar, and **Description and Translation should be separate items**.
- "Run X" is unclear: does it do the whole project, or what? Make every action say what it covers.
- Fit the new way of working: bulk first pass, curate the character sheet and term list, then a second pass over everything unlocked, then hand fixes and a final pass (see the workflow in the notes below).

## What the document must contain

1. The current-state table above, corrected and completed (include the canvas command bar and any shortcut).
2. A short model of the actions as **scope x stage**: scopes (project, selected pages, current page, stale pages, selected layers/boxes) and stages (all, detect, OCR, inpaint, describe, translate). Say which cells make sense and which do not.
3. **Two or three options** for the UI. Each has an ASCII mockup of the Process menu and the toolbar, the exact label of every item, where Retranslate stale/all go, what is removed, and what a user who only wants "describe all pages" clicks. Include at least one option that keeps the scope selector as the single source of scope and one that puts scope into the menu.
4. For every option: what `Run X`/`through` becomes (rename, drop, or replace with explicit stage items), pros and cons, and the list of components, commands and locale strings that change.
5. The backend work each option needs, in particular a bulk Describe (`Operation` / scope change) and a "stale pages" scope, and what it does to the lone-stage `forced` behaviour.
6. Your recommendation, and the open questions the user has to answer. Do not decide placement for the user.

Notes for the workflow this serves: (1) rough bulk run with Previous pages (description) at 0; (2) curate the character sheet and term list; (3) rerun everything unlocked; (4) hand fixes, which lock what they touch; (5) a last bulk run. Keep it to the document; the tickets that implement the chosen option come later.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] `design/36-run-menu.md` exists with the sections above
- [ ] Every claim about current behaviour is checked against the code, with file references
- [ ] Two or three options with mockups, a recommendation, and open questions
- [ ] No source file changed
