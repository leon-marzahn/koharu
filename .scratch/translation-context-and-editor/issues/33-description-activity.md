# 33: Show the scene description in the activity list as a description

**What to build:** The activity row for a job shows the job's stage and model (`ActivityCenter.tsx` reads `job.stage` and `job.model`; `Job` is in `crates/koharu-app/src/commands/processing.rs`, `model` comes from `StageProcessor::model()` which is `Translator::model(&self.config.model)` in `stages/translation.rs`). The description runs inside the translation processor, so while it runs the row says "Translation" with the translation provider (for example openai-compatible), although it is a scene description sent to the description model's provider (for example LM Studio).

- While the describe block runs, whether inside a Translate run or as `Operation::DescribePage`, the row shows a description label (a new `phase.*` entry, real translations in the nine locales) and the description model's provider name.
- When the translation block runs, the row shows what it shows today.
- Do not add a `Stage` variant (that touches the scheduler, `Stage::ALL` and the order of runs). Add the smallest extra the job needs, for example an optional phase field next to `stage`, set when the describe block starts and cleared when it ends. Regenerate `protocol.ts`.
- If the translation and the description run one after the other for the same page, the row follows them (Translation, then Description, or the reverse of what the code does).

Out of scope: separate rows for each, history of past phases.

**Blocked by:** 32 Describe the page without its text

**Status:** ready-for-agent

- [ ] During the describe block the row says description and names the description provider
- [ ] During the translation block it says Translation and names the translation provider, as before
- [ ] Works for a Translate run and for the Describe button
- [ ] Rust test on the job's reported phase and provider; component test for the row; nine locales (real translations)
