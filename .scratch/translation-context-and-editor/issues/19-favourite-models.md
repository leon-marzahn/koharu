# 19: Favourite models

**What to build:** The user can star models in the model picker. Starred models appear as a compact switcher next to the Run button, so changing model (for example between an SFW and an NSFW-capable one) is one click and does not open the picker. Selecting a favourite is the same action as picking it in the model picker (same preference write, same validation). The favourites list is stored in the app preferences, not the project, so it follows the user across projects.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] Star and unstar a model in the picker
- [ ] Favourites show next to Run, with the active model marked, and selecting one switches the translation model
- [ ] With no favourites the control is hidden and the layout is unchanged
- [ ] A favourite that is no longer available (provider removed, model gone) is shown disabled, not silently dropped
- [ ] Existing configs without the field load unchanged (`#[serde(default)]` plus a test that loads a config lacking it)
- [ ] Component tests cover star, switch, unavailable model and the hidden state
