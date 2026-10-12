# 16: Copy and paste boxes and style

**What to build:** Cmd+C / Cmd+V copy whole text boxes (source, translation, style, size, position) within a page (pasted 10 px offset) or across pages (same position), without their detection region. Cmd+Shift+C / Cmd+Shift+V copy a style only (font, size, color, stroke, alignment, writing mode) and paste it onto every selected box. The Inspector also has Copy style and Paste style buttons.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] Box paste creates a user-origin text layer with no region relation
- [ ] Same-page paste is offset 10 px; cross-page paste keeps position
- [ ] Style copy excludes text, position and box size
- [ ] Style paste applies to every selected box and is one undo step
- [ ] Inspector buttons work and are labelled
- [ ] Component tests cover the above
