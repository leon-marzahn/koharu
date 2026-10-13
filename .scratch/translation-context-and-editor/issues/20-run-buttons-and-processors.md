# 20: Run buttons and processor choice

**What to build:** Replace the stage multi-select (the "Stages" row and its checklist panel in `InferenceControl.tsx`) with individual run actions, because in practice the user runs one of three things: everything, translate only, or inpaint only. The OCR and inpainting model choices move into the dropdown where the Stages row used to be, next to the translation model.

- Run all stays the primary Run button and behaves as it does today for the chosen scope.
- "Translate" and "Inpaint" are one-click actions that run just that stage on the chosen scope (the backend already supports this: `process` with `operation: 'only'`, which stale-page retranslation uses). Rerunning a stage on a page that already has output replaces it (translations the user edited by hand are kept, as in ticket 10).
- The actions live inside the dropdown, so the top bar and the Run button stay as they are.
- The OCR engine and the inpainting model are chosen with the same kind of rows and pickers as the translation model, and persist in the pipeline preferences.

**Blocked by:** none

**Status:** done

- [x] The Stages row and checklist are gone; nothing else relied on `processingStages` (remove the store field and its persisted value, or keep it only if something still reads it)
- [x] "Translate" runs only the translation stage on the current scope; "Inpaint" runs only inpainting; both respect the scope selector and the stop button
- [x] Run all runs the full pipeline exactly as before
- [x] OCR and inpainting model rows sit where Stages was; choosing one saves the preference and the next run uses it
- [x] Rerunning "Translate" on a translated page replaces the machine translation and keeps lines the user edited
- [x] New strings are in every locale; component tests cover each action's `process` arguments and the preference writes

**Open question:** whether Detect and OCR also deserve their own buttons. Not in v1: the user said they run all, translate, or inpaint.
