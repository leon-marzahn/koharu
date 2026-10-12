# 19: Favourite models

**What to build:** The user can star models in the model picker (the dropdown opened from the Run control in `InferenceControl.tsx`). Starred models are listed at the top of that dropdown, above the full list, so switching between for example an SFW and an NSFW-capable model is quick. Nothing is added to the top bar or next to the Run button. Selecting a favourite is the same action as picking it in the full list (same preference write, same validation). The favourites list is stored in the app preferences, not the project, so it follows the user across projects.

**Blocked by:** none

**Status:** done

- [x] Star and unstar a model in the picker
- [x] Favourites appear in a group at the top of the dropdown, with the active model marked; selecting one switches the translation model
- [x] With no favourites the dropdown looks exactly as it does today
- [x] A favourite that is no longer available (provider removed, model gone) is shown disabled, not silently dropped
- [x] Existing configs without the field load unchanged (`#[serde(default)]` plus a test that loads a config lacking it)
- [x] Component tests cover star, switch, unavailable model and the unchanged no-favourites state
