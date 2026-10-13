# 35: Previous pages as context for the description

**What to build:** Translation can send the previous N pages as continuity hints (`previous_pages`, `previous_page_hints` in `stages/translation.rs`). The description cannot, so each page is described cold: the model does not know who was where or what just happened.

- **Setting:** `description_previous_pages: u32` on `TranslationConfig`, `#[serde(default)]`, default **0** (off). A number field "Previous pages" (0 to 10, like Translation's) in the Output section of the Description page from ticket 34, with a description line saying what is sent.
- **What is sent:** the Scene Descriptions of the previous N pages in project page order (the same page lookup `previous_page_hints` uses), oldest first, each with its page number. Pages with no description are skipped. Text only, not images. The describe system prompt tells the model they are for continuity (who is where, what changed), and that it must describe only the current page and not repeat them.
- **Where it applies:** the describe block inside Translate and the Describe button. In a bulk run, the previous page's description must be written before this page is described. Check how the scheduler orders pages in the translation lane and prove it with a test: page 2 of a two-page run receives page 1's freshly written description.
- Nine locales for the new strings (real translations).

Out of scope: sending previous pages' images; grouping pages (ticket 18).

**Blocked by:** 34 Description settings

**Status:** ready-for-agent

- [ ] `description_previous_pages` loads from a config without it and defaults to 0; 0 sends nothing extra
- [ ] N > 0 adds the previous N described pages (number and text) to the describe request, oldest first, skipping pages with no description
- [ ] In a bulk run the second page's request includes the first page's description written in the same run
- [ ] The Description page has the number field; nine locales
