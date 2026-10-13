# 36: One coherent way to run the pipeline (design)

Design only. No source file was changed. Line numbers are from this worktree at commit 7431cee.

## 1. Current state (checked against the code)

### Corrections to the ticket's table

- `Through` does NOT run "every stage up to and including" the named one. `request.rs:49-60` maps it as:
  - through Detection = [Detection]
  - through Ocr = [Detection, Ocr]
  - through Translation = [Detection, Ocr, Translation] (no Inpainting)
  - through Inpainting = [Detection, Inpainting] (no OCR)
  So "Run Translation" never cleans the page, and "Run Inpainting" never reads text.
- Translation IS listed in the Run menu (`TitleBar.tsx:223`, list is detection, ocr, translation, inpainting).
- "Locks: every run does, with no setting" is true for Translation only. Only `translation.rs` reads `Locked` (`translation.rs:36-48`, `:445`, `:498`). Detection, OCR and Inpainting do not check it (grep of `stages/` shows no `Locked` in ocr.rs, detection.rs, inpainting.rs). Whether OCR overwrites the text of a locked box was not verified.
- "Bulk runs skip Done pages" is front-end only (`lib/bulk.ts:11-22`). The backend has no Done check. It also applies unevenly (see table).
- "A lone `only inpainting` run is forced" is backend, and only for exactly `Operation::Only{Inpainting}` (`execution.rs:46-52`, `:181`). `Through Inpainting` and `Stages [inpainting]` are not forced.
- Description is not a `Stage`. `Stage::ALL` is Detection, Ocr, Translation, Inpainting (`stage.rs:30`). `phase.description` exists only as a locale label (`translation.json` ~351).

### Where the pipeline starts today

| Where | Item | Sends | Scope / Done pages | Ref |
| --- | --- | --- | --- | --- |
| Title bar > Process | Process Project | `full` | project, via runBulk: Done pages skipped, toast with count | TitleBar.tsx:200,65 |
| | Process Selected Pages | `full` | selected pages, Done pages NOT skipped | TitleBar.tsx:203-208 |
| | Process Selected Layers | `stages [ocr, translation]` | entities. No description (entity scope turns it off) | TitleBar.tsx:209-221; translation.rs:36 |
| | Run Detection / OCR / Translation / Inpainting | `through <stage>` | project via runBulk (Done skipped). Item text is `Run {{phase}}` | TitleBar.tsx:222-232 |
| Canvas command bar | hosts the toolbar only; no actions of its own; disabled without an open page | | | CanvasCommandBar.tsx:29-34 |
| Toolbar (InferenceControl) | Run | `full` | scope from store (`page` / `selected-pages` / `project`, default `selected-pages`). Only `project` goes through runBulk | InferenceControl.tsx:109; CanvasCommandBar.tsx:20-27; store.ts:39,150 |
| | Translate, Inpaint | `only translation`, `only inpainting` | same scope. Inpaint is forced | InferenceControl.tsx:45-46,83,94 |
| | Settings popover | scope, models, OCR, inpaint, output | | InferenceControl.tsx:288-330 |
| | Run is disabled when scope is `selected-pages` and nothing is selected; becomes Stop while a job runs | | | InferenceControl.tsx:61,107-112 |
| Page rail | Retranslate N stale pages | `only translation` | stale ids resolved on the client, then runBulk with `only` | PageRail.tsx:223-236,331 |
| | Retranslate all pages | `only translation` | runBulk, all pages | PageRail.tsx:353 |
| | Mark all as up to date / per-page "Mark as up to date" | `dismissStale`, no run | | PageRail.tsx:238,342,643 |
| Inspector | Translate (one box), Translate with note | `translate_box` | `Scope::Entities([box])` | Inspector.tsx:1031-1036 |
| | Describe this page | `describe_page` | `Scope::Pages([page])` | Inspector.tsx:1175-1180 |
| Region tools | draw region -> process; re-OCR region | `process_region(id, clean)` | see below | RegionsOverlay.tsx:222,306 |

Backend entry points:
- `process(scope, operation)` is the only entry (`processing.rs:81`). It refuses to start while any job runs (`processing.rs:100-102`), so there is no queue.
- `process_region` (`canvas.rs:546-614`) runs `Stages [ocr, translation]` on the region entity, and if `clean`, a second job `Only Inpainting` with `Scope::Region` and a rectangle mask. The `Only Inpainting` there is forced too.
- Scope has four variants: Project, Pages, Region, Entities (`scope.rs:77-88`). There is no stale scope. "Stale" is `PageSummary.stale` (`commands/project.rs:67`) resolved by the client into a `Pages` list.
- `Scope::Entities` with Detection or Inpainting is rejected (`scope.rs:99-105`).
- Shortcuts: none for processing. The only menu shortcuts are Undo, Redo, Select all, Delete (`TitleBar.tsx:159-192`).

### Behaviour of single stages when a prerequisite is missing

- The scheduler does not wait for a prerequisite that is not in the run (`scheduler.rs:37-47`); prerequisites are Ocr and Inpainting after Detection, and Translation after Ocr and Inpainting (`scheduler.rs:147-153`).
- Only Translation: runs on whatever OCR text exists. With no source text, the page's translation stage is skipped (`translation.rs:490-500`: skipped when nothing is translatable and no description will run). It does not fail and does not tell the user why.
- Detection skips a page that already has a text region with recognized text (`detection.rs:102-120`). So "Run Detection" on a processed page does nothing. A region with no recognized text does not count.
- Inpainting skips a page that already has a cleanup layer with a source asset, unless an inpainting mask is supplied or the run is forced (`inpainting.rs:105-118`).
- OCR has no skip rule (default `false`, `stages/mod.rs:92`). Re-running it replaces text. Not verified: what it does to locked or hand-edited boxes.
- Only Translation always also describes the page when a description model is set, the scope is not entities, and the description is not locked (`translation.rs:36-38`, `:490-498`). There is no way today to translate without redoing the description, other than locking it.
- `DescribePage` runs the translation stage with `describe_only` (`request.rs:62`, `execution.rs:70`). The Inspector only sends one page. Whether it behaves over many pages was not exercised; the code path is per page and the doc comment says "scoped pages" (`request.rs:37-38`), so it should.
- Translation runs strictly page after page (scheduler test `a_page_translates_only_after_the_previous_page_finished_translating`, `scheduler.rs:230`). That is what lets "Previous pages" read earlier descriptions in a bulk run.

## 2. Model: scope x stage

Scopes: project, selected pages, current page, stale pages, selected layers/boxes.
Stages: all, detect, OCR, inpaint, describe, translate.

```
                 all   detect  OCR   inpaint  describe  translate
project           Y      Y      Y      Y         Y          Y
selected pages    Y      Y      Y      Y         Y          Y
current page      Y      Y      Y      Y         Y          Y
stale pages       -      -      -      -         Y          Y      (stale = glossary/character edits; only text stages react)
selected layers   -      -      Y      -         -          Y      (entity scope: no detect/inpaint, no description)
```

Notes:
- "Stale pages x all/detect/OCR/inpaint" make no sense: stale is set by character and term edits (`translation.rs:686`, `project.rs:382`), which only affect text.
- "Selected layers x describe" makes no sense: description is per page, and entity scope disables it (`translation.rs:36`).
- "Selected layers x all" is today's "Process Selected Layers" = OCR + translate. It is a different thing from "all", so it should keep its own name.
- Describe and Translate are one backend stage today. Everything below that treats them as two menu items needs the "translate without describing" gap closed (question 3).

## 3. Options

Common to all options: the statement of coverage is part of the label. Done-page skipping and locks are stated in a one-line hint, not as a setting.

### Option A: scope stays in the toolbar selector; the menu lists stages only

The Process menu and the toolbar buttons are the same list of stages. Both act on the one scope selector (`Page | Selection | Project`), and the menu shows the current scope in its header so the item never hides what it covers.

```
Menu bar:  File  Edit  Process  View  Help

Process
 +-------------------------------------------+
 | Scope: Selection (4 pages)            >   |   opens the toolbar scope popover
 |-------------------------------------------|
 | Run all stages                            |
 |-------------------------------------------|
 | Detect text                               |
 | Recognise text (OCR)                      |
 | Clean text (inpaint)                      |
 | Describe pages                            |
 | Translate pages                           |
 |-------------------------------------------|
 | Retranslate stale pages (3)               |
 | Process selected layers (OCR + translate) |
 +-------------------------------------------+

Toolbar:
 [ Scope: Selection v ] [ Describe ] [ Translate ] [ Inpaint ] [ Run all v ]
```

- Page rail: "Retranslate N stale pages" stays (it is a scope of its own that the selector cannot express); "Retranslate all pages" is removed. It equals Translate with scope Project.
- Removed: `Process Project`, `Process Selected Pages`, `Run <phase>` (all four), `Retranslate all pages`.
- "Describe all pages" = scope Project in the selector, then Describe (2 clicks, or 3 if the scope was not Project).
- `Run X` / `through`: replaced by explicit stage items. `Operation::Through` is deleted (TitleBar is its only frontend user; I did not search the Rust side beyond `request.rs`, and `bin/run.rs:196` uses `Full`).
- Pros: smallest change, one source of scope, no state is duplicated in the menu. Matches the user's "same actions as the toolbar" literally.
- Cons: the same item means different things depending on a setting hidden in a popover. A mis-set scope (default `selected-pages`) runs on the wrong pages. The user's complaint ("what does it cover?") is answered only by the menu header, not by the item.
- Changes:
  - Components: `TitleBar.tsx` (menu), `InferenceControl.tsx` (add Describe button; shared stage list), `PageRail.tsx` (remove Retranslate all), `CanvasCommandBar.tsx` (unchanged except it passes the new operations).
  - Commands: `process` unchanged; frontend sends `only <stage>` and a new describe operation (section 4).
  - Locale: remove `menu.processProject`, `menu.processPages`, `menu.runPhase`, `navigator.retranslateAll`; add `menu.scopeHeader`, `menu.describe`, `menu.translate`, `menu.runAll`, plus the stage names. `inference.translateOnly` / `inpaintOnly` get a `describeOnly` sibling. Strings in every locale folder (`en-US`, `es-ES`, `ja-JP`, ...).

### Option B: scope lives in the menu; the toolbar selector goes away

Scope is the first level of the Process menu, and stage is the second. Nothing is hidden in a popover. The toolbar keeps one primary button that repeats the last action.

```
Process
 +--------------------+      +----------------------------------+
 | Project         >  | ---> | Everything                       |
 | Selected pages  >  |      | Describe                         |
 | Current page    >  |      | Translate                        |
 | Stale pages (3) >  |      | Detect / OCR / Inpaint   (>)     |
 | Selected layers >  |      +----------------------------------+
 +--------------------+
 Stale pages > : Describe, Translate only
 Selected layers > : OCR + translate only

Toolbar:
 [ Translate stale (3) v ]   <- last used action, with its scope in the label
```

- Page rail: Retranslate stale and Retranslate all are removed (they are `Stale pages > Translate` and `Project > Translate`). The "Mark as up to date" actions stay.
- Removed: the toolbar scope selector, the scope row in the popover, `Process Project`, `Process Selected Pages`, `Run <phase>`, both rail buttons, `processingScope` in the store.
- "Describe all pages" = Process > Project > Describe (3 clicks, nothing to set first).
- `Run X` / `through`: dropped. Impossible cells (section 2) are simply not in the submenus, which tells the user what is allowed.
- Pros: every item states its scope by its position; impossible combinations are not offered; no hidden state.
- Cons: more clicks; a nested menu with 5 x 6 entries; the toolbar "repeat last action" is new state that has to be defined. Biggest rewrite.
- Changes:
  - Components: `TitleBar.tsx`, `InferenceControl.tsx` (rebuilt: loses the scope view and `SelectorView 'scope'`), `PageRail.tsx`, `CanvasCommandBar.tsx` (its `run` dispatcher by `PipelineScope` goes away), `store.ts` (`processingScope`, `setProcessingScope`, `PipelineScope`), `lib/bulk.ts` (callers change).
  - Commands: none required for scopes the client can resolve to `Pages`; Stale pages is a client-side resolve (as the rail does today).
  - Locale: remove `inference.scope*`, `inference.currentPage*`, `selectedPages`, `entireProject*`, `menu.processProject/Pages/Layers`, `menu.runPhase`, `navigator.retranslate*`; add scope and stage labels.

### Option C: one "Run..." dialog; the menu has two shortcuts

Process opens a single dialog that states what will happen before it happens. The toolbar button opens the same dialog, preselected from the selection.

```
Process
 +---------------------------+
 | Run...                    |
 | Retranslate stale (3)     |
 | Process selected layers   |
 +---------------------------+

 Run
 +-----------------------------------------------+
 | Pages   (o) Project  ( ) Selected (4)         |
 |         ( ) Current  ( ) Stale (3)            |
 | Steps   [x] Detect   [x] OCR    [x] Clean     |
 |         [x] Describe [x] Translate            |
 |-----------------------------------------------|
 | 12 pages, 3 Done skipped. Locked boxes and    |
 | locked descriptions are kept.                 |
 |                              [Cancel] [Run]   |
 +-----------------------------------------------+

Toolbar:  [ Run... ]  [ Stop ] while a job runs
```

- Page rail: both Retranslate buttons removed (set Pages = Stale, Steps = Translate in the dialog), or kept as presets that open the dialog already filled in.
- "Describe all pages": Run... > untick all but Describe > Run (3 clicks, the dialog opens with the last choice remembered).
- `Run X` / `through`: dropped; the dialog sends `Operation::Stages { stages }` (already exists, `request.rs:40`) plus a describe flag.
- Pros: the summary line answers "what does it cover" with real numbers; any scope x stage combination in one place; the matrix needs no per-cell UI; the dialog is where Done-skipping and locks can be explained once.
- Cons: a dialog for each run is slow for the common "just translate this page" case (mitigated by keeping the single-page Inspector actions); the arbitrary stage subset means the dialog must handle incoherent choices (Clean without Detect on a new page); new component. Invalid cells (stale x detect) are greyed out instead of absent.
- Changes:
  - Components: new `RunDialog` (new file), `TitleBar.tsx`, `InferenceControl.tsx` (shrinks to one button plus model settings), `PageRail.tsx`, `CanvasCommandBar.tsx`, `store.ts` (remembers the last choice).
  - Commands: `process` plus a way to say "describe" inside `Stages` (section 4).
  - Locale: new dialog strings in every locale folder; remove `menu.processProject/Pages`, `menu.runPhase`, `navigator.retranslate*`, `inference.translateOnly/inpaintOnly`.

## 4. Backend work

Needed by all options:

1. Bulk Describe. `Operation::DescribePage` already takes any `Scope::Pages` (`request.rs:37-39`; `execution.rs:70` has no single-page check). Likely only a rename (`Describe`, since it is no longer "a page") and a test over two pages. Pending: confirm the multi-page path with a test, and decide whether Done pages are skipped (front-end `runBulk` today; unchanged).
2. Stale scope. Not needed on the backend: the client already resolves stale pages to `Pages` (`PageRail.tsx:223`). A `Scope::Stale` would only matter if the agent panel or a CLI should run it atomically; I would not add it now.
3. Translate without describing. Gap (see section 1). `Only Translation` re-describes every unlocked page (`translation.rs:36`). If the second pass should keep the description from the curated first pass, there needs to be either a flag on the operation or an explicit `Translate` operation that sets the describe step off (the code already has `should_describe(configured, scoped_to_entities, locked)`; this adds a fourth input). Needed for B and C's "Translate" item and for A's too. Not needed if the user accepts "Translate redoes the description unless it is locked".
4. `forced`. Only `Only{Inpainting}` is forced (`execution.rs:46`) and `process_region` uses it too (`canvas.rs:601`). Describe is unaffected. Options A and B keep `Only{Inpainting}` as the explicit "Clean text" action and so keep the force. In C, a ticked "Clean" in a multi-stage `Stages` run is not forced, which means a re-run of a project with Clean ticked skips pages that already have a cleanup layer. The user needs to know that (question 4). Making the dialog send `Only` when Clean is the single tick keeps today's behaviour.
5. `Through`. Under all three options it has no caller and should be deleted along with its tests and the bridge type (regenerate after; AGENTS.md: no back-compat).

## 5. Recommendation

Option A, with the menu header showing the scope. It is the smallest change that makes the menu and the toolbar the same list, gives Describe and Translate their own items, and removes "Run X" in favour of names that say the stage. The weakness is the hidden scope; showing it in the menu header and in the toolbar button label ("Translate - 4 selected pages") covers most of it. If the scope confusion is the main pain, B is the better fit and costs about twice as much; C is the right choice only if the user wants one place for every combination.

This is a recommendation, not a decision about placement.

## 6. Questions for the user

1. Scope: single selector (A), scope in the menu (B), or chosen in a dialog (C)?
2. Does "Process Selected Layers" stay as its own item (OCR + translate on boxes)? It is the only entity-scope item in the menu.
3. In the second pass, should "Translate" also redo the description (today it does for every unlocked page), or keep it and only translate? If keep, a backend change is needed (section 4, item 3).
4. Should a project-wide Clean skip pages that already have a cleanup layer (today: yes for `full`, no for the lone Inpaint button)?
5. Should "Run all stages" / `full` exist at all once Describe and Translate are separate, or is the first pass done as Detect, OCR, Clean, Describe, Translate by hand? Today `full` is the only way to do the first pass in one job.
6. Done pages: keep skipping them silently with a toast in bulk runs, or add an explicit "include Done" choice? The skip is front-end only, and explicit selections ignore it.
7. Do you want a keyboard shortcut for any of these? None exists now.
8. Is OCR allowed to overwrite locked boxes? Unverified in the code; the menu text must not promise otherwise.
