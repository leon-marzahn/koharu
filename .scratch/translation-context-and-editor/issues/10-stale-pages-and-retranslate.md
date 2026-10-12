# 10: Stale pages and retranslate

**What to build:** Editing a name or term marks pages whose source contains it as stale. Editing a character's style, appearance or notes marks pages containing that character's source name. Stale pages are marked in the page rail, with "Retranslate N stale pages" and "Retranslate all pages" actions. Nothing reruns automatically, and translations you edited by hand are kept.

**Blocked by:** 05 Character sheet (manual), 06 Term list (manual)

**Status:** done

- [x] Stale rules above hold for the old and new source strings
- [x] Stale marker shows in the page rail and clears after retranslation
- [x] Both retranslate actions run the translation stage and keep user-edited translations
- [x] No automatic reruns
- [x] Stage and component tests cover the above
