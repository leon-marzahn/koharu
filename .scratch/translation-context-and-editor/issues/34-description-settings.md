# 34: Description settings

**What to build:** The description model has no settings of its own. It is picked from a row in the editor's model dropdown, it never receives the Translation page's instructions, and its generation settings are hard-coded. Add a **Description** page to Settings, next to Translation, that mirrors the Translation page's Model, Generation and Output sections for the description call.

- **Settings navigation:** a new `description` tab in `SettingsPage.tsx` (icon of your choice from lucide, label "Description"), directly after Translation. New page component `DescriptionPreferences.tsx` modelled on `TranslationPreferences.tsx`.
- **Model section:** the same model picker as Translation (`ModelPicker`, same popover, same provider badge), bound to `description_model`. The picker has no "none" choice, so add a small "Don't describe" action that clears it; with no model the description stage is off, as today. The row in the editor's model dropdown stays as it is.
- **Generation section:** reuse `GenerationPreferences` bound to a new `description_generation`. Add a prop to hide the reasoning switch and hide it here: reasoning is never sent for the description (`reasoning: None`), so the provider's own setting (for example LM Studio's thinking toggle) decides. A reasoning level the model does not support makes LM Studio fall back to thinking on, which overrides its own toggle.
- **Output section:** no language control. The description is written in the Translation page's target language (`describing(self.config.target_language)` in `stages/translation.rs` already does this); say so in the section's description text. One new field, **Instructions** (`description_instructions`, textarea like Translation's), is appended to the describe system prompt (the `TranslationRequest` already has `with_instructions`; check the describe prompt shows it). The translation `instructions` field is never sent to the description.
- **Config** (in `TranslationConfig`, `crates/koharu-pipeline/src/config.rs`): keep `description_model` where it is, add flat fields `description_generation: GenerationConfig` and `description_instructions: Option<String>`, both `#[serde(default)]`. The default `description_generation` has `reasoning: None`. Add a test that loads a config without these fields. Remove the temporary `description_generation()` helper in `stages/translation.rs` and read the config field instead. Regenerate `protocol.ts`.
- **Where it applies:** the describe block inside Translate and `Operation::DescribePage` (the Describe button). One-box runs never describe.
- Nine locales for the new strings (real translations).

Out of scope: previous pages for the description (ticket 35); a reasoning control; removing the editor dropdown row.

**Blocked by:** 33 Description activity (merged)

**Status:** done

- [x] Settings has a Description page with Model, Generation (without reasoning) and Instructions; no language control
- [x] The description call uses `description_generation` and `description_instructions`, never the translation ones; no `reasoning_effort` is sent
- [x] "Don't describe" clears the model; the editor dropdown row still works and edits the same field
- [x] Config without the new fields loads; Rust tests for the prompt carrying the instructions and for the config default
- [x] Component test for the page (edits reach `onChange`, model clear, no reasoning switch); nine locales
