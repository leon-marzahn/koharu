# 48: The description call keeps the Character sheet up to date

**What to build:** The describe call proposes characters the same way the translation call does: new characters with every field, and updates to known characters whenever the page shows something new or different. Analysis-only projects never run the translation call, so today they only get `source_name` and `appearance` (the describe schema has nothing else) and never a speech style.

- **Input:** the describe request also carries the page's OCR text (`page_text`, one line per text box, in layer order) and `known_sources`. The model no longer depends on the bubbles being visible: a page that was already cleaned still has its text.
- **Reply:** `{ description, characters, character_updates }`. `characters` and `character_updates` have the translation call's shapes (all five fields; updates use `null` for "unchanged"). Both schemas are built from one shared definition.
- **What gets tracked:** `speech_style` (register, verbal tics, sentence endings, how they refer to themselves and others), `appearance` (lasting traits first; a changed outfit is recorded as the current outfit and does not replace lasting traits), and `notes` (everything noteworthy: role, personality, relationships and how they address others, habits, recurring items, facts revealed so far).
- **Updates rewrite the field:** approving an update replaces the field, so a proposed value is the full revised text: keep what still holds, add what is new, correct what was wrong (three piercings, not two). Only for something the page shows; never restate the current value. The translation call's proposal rules say the same.
- **Description text is unchanged:** it still ignores the dialogue, and a matched character is still called by `target_name` and not described by looks.
- **Pipeline:** in a Translate run the describe call's proposals merge with the translation call's: the translation call's new characters first (it names them), then the describe call's; the describe call's updates first, then the translation call's (later wins per field). Describe this page and analysis-only runs merge the describe call's proposals on their own. A describe failure still only logs during Translate.
- **Repeats:** updates merge per character in the pending list (newest value wins per field). A rejected update can be proposed again on a later page; remembering rejected update text is a follow-up if it nags.

Out of scope: per-arc outfit history; term proposals from the describe call.

**Blocked by:** none (supersedes the describe half of ticket 12; ticket 37 was dropped)

**Status:** done

- [x] Describe request carries `page_text` and `known_sources`
- [x] Describe schema and prompt ask for new characters and updates with every field; shared schema with translation
- [x] Prompts ask for full revised text in updates and define what goes in speech style, appearance and notes
- [x] Describe-only runs store speech style and notes from the describe call
- [x] Translate runs merge both calls' proposals in the stated order
- [x] Rust prompt and stage tests cover input, schema, parsing and merging
