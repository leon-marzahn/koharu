# 37: The description call stops proposing characters

**What to build:** The describe call returns a `characters` array of people who match nobody on the sheet, but it can never fill it. The prompt says to ignore all text and to skip people the page does not name (`description_system_prompt` in `crates/koharu-translator/src/prompt.rs`), the image is the cleaned page with the text removed (ticket 32), and the request carries only the character sheet and the previous pages. The model never sees a name, so it correctly returns `[]`. Characters and their appearance are proposed by the translation call, which sees the text and the original image. Remove the dead path.

- **Prompt and schema:** the describe reply is `{ "description": string }`. Drop `characters` from the schema (`request_schema`, `properties` and `required`) and the bullet that asks for it in the system prompt. Keep the rule that a person who matches the character sheet is called by `target_name` and not described by looks. Keep sending the sheet.
- **Parsing:** the description is read as before. A reply that still contains a `characters` key (an old prompt, a model that adds it anyway) must parse and the key is ignored. A fenced reply keeps working (`unfence`).
- **Pipeline:** remove the plumbing that carries the describe call's characters into the proposals: the `appearances` value taken from the describe result and extended into the translation proposals in `stages/translation.rs` (around lines 560 and 642), and the describe-only `merge_proposals` call (around line 467). `merge_proposals` itself stays; the translation call uses it.
- **Behaviour that must not change:** a pending character created by Translate's proposals keeps its appearance; the description failing only logs; Describe this page still writes only the description and the lock rules are unchanged.
- Update the tests that assert the describe `characters` array, its schema and its merge (prompt tests, `translation.rs` tests). Add one test: a describe reply with a stray `characters` array parses and creates no proposals.
- Update ticket 12's `Status:` line with a note that this ticket replaced its describe-side proposals. Regenerate `protocol.ts` only if a type changed.

Out of scope: sending the page's OCR text to the describe call; changes to Translate's own proposals.

**Blocked by:** none

**Status:** ready-for-agent

- [ ] The describe schema and prompt no longer mention `characters`; matched characters are still named by `target_name`
- [ ] A stray `characters` key in a describe reply is ignored, not an error
- [ ] No appearance or character proposal comes from the describe call; Translate's proposals are unchanged
- [ ] Rust tests updated and passing, including the stray-key test
