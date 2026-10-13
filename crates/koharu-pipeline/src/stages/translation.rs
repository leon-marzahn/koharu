use anyhow::Result;
use async_trait::async_trait;
use koharu_scene::{
    Authored, CharacterSheet, CharacterUpdate, EntityId, Generation, LanguageTag, Origin,
    Proposals, Snapshot, SourceText, StalePage, TermList, TermUpdate, Translation,
};
use koharu_translator::{
    TranslationCharacter, TranslationCharacterUpdate, TranslationContext, TranslationProposals,
    TranslationRequest, TranslationTerm, TranslationTermUpdate, Translator,
};

use crate::TranslationConfig;

use super::{StageInput, StageProcessor, finish, generation};

const PRODUCER: &str = "dev.koharu.pipeline.translation";

pub(super) struct Processor {
    config: TranslationConfig,
    translator: Translator,
}

impl Processor {
    pub(super) fn new(config: TranslationConfig, translator: Translator) -> Self {
        Self { config, translator }
    }
}

/// Source/translation pairs from the last `limit` pages before `page` that have any translation,
/// in reading order. Uses the current translation, so user edits win and are flagged.
fn previous_page_hints(
    scene: &Snapshot,
    page: EntityId,
    limit: u32,
) -> Result<Vec<TranslationContext>> {
    let mut pages = Vec::new();
    for candidate in scene.pages().take_while(|candidate| candidate.id() != page) {
        let mut lines = Vec::new();
        if let Some(group) = candidate.text_group()? {
            for layer in group.text_layers()? {
                let content = layer.content()?;
                let (Some(source), Some(translation)) = (content.source()?, content.translation()?)
                else {
                    continue;
                };
                if source.text.value.trim().is_empty() || translation.text.value.trim().is_empty() {
                    continue;
                }
                lines.push(TranslationContext {
                    source: source.text.value,
                    translation: translation.text.value,
                    user_corrected: matches!(translation.text.origin, Origin::User),
                });
            }
        }
        if !lines.is_empty() {
            pages.push(lines);
        }
    }
    let skip = pages.len().saturating_sub(limit as usize);
    Ok(pages.into_iter().skip(skip).flatten().collect())
}

fn proposals(scene: &Snapshot) -> Result<Proposals> {
    Ok(scene.project_component::<Proposals>()?.unwrap_or_default())
}

/// Every approved character sheet entry, on every page: the sheet is small and always relevant.
/// Pending entries follow, marked tentative, only when the project opts in.
fn character_sheet(scene: &Snapshot) -> Result<Vec<TranslationCharacter>> {
    let proposals = proposals(scene)?;
    let pending = proposals.use_in_prompts.then_some(proposals.characters);
    let approved = scene
        .project_component::<CharacterSheet>()?
        .unwrap_or_default()
        .characters;
    Ok(approved
        .into_iter()
        .map(|character| (character, false))
        .chain(pending.into_iter().flatten().map(|c| (c, true)))
        .map(|(character, tentative)| TranslationCharacter {
            source_name: character.source_name,
            target_name: character.target_name,
            speech_style: character.speech_style,
            appearance: character.appearance,
            notes: character.notes,
            tentative,
        })
        .collect())
}

/// Term list entries whose source is a substring of at least one segment of the page, with
/// pending ones (tentative) only when the project opts in.
fn page_terms(scene: &Snapshot, segments: &[&str]) -> Result<Vec<TranslationTerm>> {
    let proposals = proposals(scene)?;
    let pending = proposals.use_in_prompts.then_some(proposals.terms);
    let approved = scene
        .project_component::<TermList>()?
        .unwrap_or_default()
        .terms;
    Ok(approved
        .into_iter()
        .map(|term| (term, false))
        .chain(pending.into_iter().flatten().map(|t| (t, true)))
        .filter(|(term, _)| {
            !term.source.is_empty() && segments.iter().any(|text| text.contains(&term.source))
        })
        .map(|(term, tentative)| TranslationTerm {
            source: term.source,
            target: term.target,
            note: term.note,
            tentative,
        })
        .collect())
}

/// Sources the model must not propose: rejected and pending ones (pending entries are not always
/// in the prompt). Approved entries are already visible to the model in the sheet and term list.
fn known_sources(scene: &Snapshot) -> Result<Vec<String>> {
    let proposals = proposals(scene)?;
    Ok(proposals
        .rejected_characters
        .into_iter()
        .chain(proposals.rejected_terms)
        .chain(proposals.characters.into_iter().map(|c| c.source_name))
        .chain(proposals.terms.into_iter().map(|t| t.source))
        .collect())
}

/// Adds returned proposals as pending entries. A "new" entry whose source is already approved
/// becomes an update; one already pending or rejected is dropped. Returns `None` when nothing is new.
fn merge_proposals(scene: &Snapshot, mut new: TranslationProposals) -> Result<Option<Proposals>> {
    let mut merged = proposals(scene)?;
    let sheet = scene
        .project_component::<CharacterSheet>()?
        .unwrap_or_default();
    let terms = scene.project_component::<TermList>()?.unwrap_or_default();
    let before = merged.clone();
    // The model only sees terms that appear in the prompt's text, so it re-proposes an existing
    // entry as new when it learns more about it. Treat those as updates instead of dropping them;
    // the update path below keeps only the fields that actually changed.
    let given = |value: String| Some(value).filter(|value| !value.trim().is_empty());
    let (existing, characters): (Vec<_>, Vec<_>) = new.characters.into_iter().partition(|c| {
        sheet
            .characters
            .iter()
            .any(|known| known.source_name.trim() == c.source_name.trim())
    });
    new.characters = characters;
    new.character_updates
        .extend(existing.into_iter().map(|c| TranslationCharacterUpdate {
            source_name: c.source_name,
            target_name: given(c.target_name),
            speech_style: given(c.speech_style),
            appearance: given(c.appearance),
            notes: given(c.notes),
        }));
    let (existing, new_terms): (Vec<_>, Vec<_>) = new.terms.into_iter().partition(|t| {
        terms
            .terms
            .iter()
            .any(|known| known.source.trim() == t.source.trim())
    });
    new.terms = new_terms;
    new.term_updates
        .extend(existing.into_iter().map(|t| TranslationTermUpdate {
            source: t.source,
            target: given(t.target),
            note: given(t.note),
        }));
    for character in new.characters {
        let source = character.source_name.trim().to_owned();
        let known = |name: &str| name.trim() == source;
        if source.is_empty()
            || sheet.characters.iter().any(|c| known(&c.source_name))
            || merged.characters.iter().any(|c| known(&c.source_name))
            || merged.rejected_characters.iter().any(|name| known(name))
        {
            continue;
        }
        merged.characters.push(koharu_scene::Character {
            source_name: source,
            target_name: character.target_name,
            speech_style: character.speech_style,
            appearance: character.appearance,
            notes: character.notes,
        });
    }
    for term in new.terms {
        let source = term.source.trim().to_owned();
        let known = |name: &str| name.trim() == source;
        if source.is_empty()
            || terms.terms.iter().any(|t| known(&t.source))
            || merged.terms.iter().any(|t| known(&t.source))
            || merged.rejected_terms.iter().any(|name| known(name))
        {
            continue;
        }
        merged.terms.push(koharu_scene::Term {
            source,
            target: term.target,
            note: term.note,
        });
    }
    // Updates target approved entries only. Fields equal to the current value are dropped, an
    // update with nothing left is dropped, and one for an already pending entry merges into it.
    for update in new.character_updates {
        let key = update.source_name.trim();
        let Some(current) = sheet
            .characters
            .iter()
            .find(|c| c.source_name.trim() == key)
        else {
            continue;
        };
        let changed = |new: Option<String>, old: &str| new.filter(|value| value != old);
        let update = CharacterUpdate {
            source_name: current.source_name.clone(),
            target_name: changed(update.target_name, &current.target_name),
            speech_style: changed(update.speech_style, &current.speech_style),
            appearance: changed(update.appearance, &current.appearance),
            notes: changed(update.notes, &current.notes),
        };
        if update
            == (CharacterUpdate {
                source_name: update.source_name.clone(),
                ..Default::default()
            })
        {
            continue;
        }
        match merged
            .character_updates
            .iter_mut()
            .find(|u| u.source_name == update.source_name)
        {
            Some(pending) => {
                pending.target_name = update.target_name.or(pending.target_name.take());
                pending.speech_style = update.speech_style.or(pending.speech_style.take());
                pending.appearance = update.appearance.or(pending.appearance.take());
                pending.notes = update.notes.or(pending.notes.take());
            }
            None => merged.character_updates.push(update),
        }
    }
    for update in new.term_updates {
        let key = update.source.trim();
        let Some(current) = terms.terms.iter().find(|t| t.source.trim() == key) else {
            continue;
        };
        let changed = |new: Option<String>, old: &str| new.filter(|value| value != old);
        let update = TermUpdate {
            source: current.source.clone(),
            target: changed(update.target, &current.target),
            note: changed(update.note, &current.note),
        };
        if update.target.is_none() && update.note.is_none() {
            continue;
        }
        match merged
            .term_updates
            .iter_mut()
            .find(|u| u.source == update.source)
        {
            Some(pending) => {
                pending.target = update.target.or(pending.target.take());
                pending.note = update.note.or(pending.note.take());
            }
            None => merged.term_updates.push(update),
        }
    }
    Ok((merged != before).then_some(merged))
}

#[async_trait]
impl StageProcessor for Processor {
    fn model(&self) -> &'static str {
        Translator::model(&self.config.model)
    }

    fn unload(&self) -> bool {
        self.translator.unload()
    }

    async fn load(&self) -> Result<()> {
        self.translator.load_model(&self.config.model).await
    }

    async fn process(&self, input: StageInput) -> Result<koharu_scene::Patch> {
        let mut targets = Vec::new();
        // Entity scope translates only the chosen segments; the page's other segments go to the
        // model as context so a single box reads like it does in a full run.
        let mut siblings = Vec::new();
        if let Some(group) = input.scene.page(input.page)?.text_group()? {
            for layer in group.text_layers()? {
                let in_scope = input.contains_entity(layer.id())?;
                let content = layer.content()?;
                let Some(source) = content.source()? else {
                    continue;
                };
                if source.text.value.trim().is_empty() {
                    continue;
                }
                if in_scope {
                    targets.push((content.id(), source.text.value));
                } else if input.entities.is_some() {
                    let translation = content.translation()?;
                    siblings.push(TranslationContext {
                        source: source.text.value,
                        user_corrected: translation
                            .as_ref()
                            .is_some_and(|t| matches!(t.text.origin, Origin::User)),
                        translation: translation.map(|t| t.text.value).unwrap_or_default(),
                    });
                }
            }
        }
        // Every context source (hints today; character sheet, term list later) appends here.
        let mut context = siblings;
        context.extend(previous_page_hints(
            &input.scene,
            input.page,
            self.config.previous_pages,
        )?);
        // Terms match the current page and the previous-page lines, so a term stays in play
        // while the scene keeps talking about it without naming it.
        let term_text = targets
            .iter()
            .map(|(_, s)| s.as_str())
            .chain(context.iter().map(|hint| hint.source.as_str()))
            .collect::<Vec<_>>();
        let terms = page_terms(&input.scene, &term_text)?;
        let mut request = TranslationRequest::new(
            targets.iter().map(|(_, source)| source.clone()),
            self.config.target_language,
        )
        .with_context(context)
        .with_characters(character_sheet(&input.scene)?)
        .with_terms(terms)
        .with_known_sources(known_sources(&input.scene)?);
        if let Some(explain) = &input.explain {
            // Only a steered request carries the current translation as the previous attempt.
            let previous = match targets.as_slice() {
                [(entity, _)] => input
                    .scene
                    .component::<Translation>(*entity)?
                    .map(|translation| translation.text.value),
                _ => None,
            };
            request = request.with_explanation(explain.note.clone(), previous);
        }
        if let Some(instructions) = self.config.instructions.as_deref() {
            request = request.with_instructions(instructions);
        }
        if Translator::supports_vision(&self.config.model, &self.config.generation)
            && let Some(image) = input.images.get(&input.scene, input.page, "source").await?
        {
            request = request.with_image(image);
        }
        let (provider, mut translated) = self
            .translator
            .translate(&self.config.model, self.config.generation, request)
            .await?;
        // ponytail: the explanation only travels as a progress event, so reloading loses it; the
        // upgrade is a stored component (revision, serde default, load test) written in this edit.
        if let (Some(explain), Some(text)) = (&input.explain, translated.explanation.take()) {
            crate::progress::emit(
                explain.progress.as_ref(),
                crate::Progress::Explained {
                    page: input.page,
                    text,
                },
            );
        }
        let language = LanguageTag::new(self.config.target_language.tag())?;
        let generated = generation(PRODUCER, provider)?;
        let mut edit = input.scene.edit_as(generated.clone());
        if input.entities.is_some() {
            edit.replace_user_edits();
        }
        for (entity, _) in &targets {
            edit.observe::<SourceText>(*entity)?;
            edit.observe::<Translation>(*entity)?;
        }
        // ponytail: replaces the whole component from this page's snapshot, so a review made while
        // pages translate concurrently can be overwritten; add project-level observation if it bites.
        if let Some(merged) = merge_proposals(&input.scene, translated.proposals)? {
            edit.set_project(&merged)?;
        }
        write_translations(
            &input.scene,
            &mut edit,
            input.page,
            targets,
            translated.segments,
            &language,
            &generated,
            // An entity-scoped run is the user asking for these boxes, so it replaces their edits.
            input.entities.is_some(),
        )?;
        finish(edit)
    }
}

/// Writes generated translations, keeping hand-edited ones unless `overwrite_user`, and clears the page's stale
/// marker: the page is now translated against the current sheet and term list.
fn write_translations(
    scene: &Snapshot,
    edit: &mut koharu_scene::Edit,
    page: EntityId,
    targets: Vec<(EntityId, String)>,
    segments: Vec<String>,
    language: &LanguageTag,
    generated: &Generation,
    overwrite_user: bool,
) -> Result<()> {
    for ((entity, source), text) in targets.into_iter().zip(segments) {
        if !overwrite_user
            && scene
                .component::<Translation>(entity)?
                .is_some_and(|value| matches!(value.text.origin, Origin::User))
        {
            continue;
        }
        let text = if source.trim() == "\u{2026}" {
            "\u{2026}".to_owned()
        } else {
            text
        };
        edit.set(
            entity,
            &Translation {
                text: Authored::generated(text, generated.clone()),
                language: Some(language.clone()),
            },
        )?;
    }
    // A one-box run (overwrite_user) leaves the rest of the page stale.
    if !overwrite_user && scene.component::<StalePage>(page)?.is_some() {
        edit.remove::<StalePage>(page)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use koharu_scene::{At, Authored, PageDraft, Session, TextLayout, TextLayoutKind, Translation};

    use super::*;

    #[tokio::test]
    async fn every_character_sheet_entry_reaches_the_request() {
        let mut session = Session::memory().await.unwrap();
        assert!(character_sheet(&session.snapshot()).unwrap().is_empty());
        let character = |name: &str| koharu_scene::Character {
            source_name: name.into(),
            target_name: format!("{name}-t"),
            speech_style: "curt".into(),
            appearance: "tall".into(),
            notes: "rival".into(),
        };
        let patch = session
            .snapshot()
            .patch(|edit| {
                edit.set_project(&CharacterSheet {
                    characters: vec![character("a"), character("b")],
                })
            })
            .unwrap();
        let scene = session.commit(patch).await.unwrap().snapshot;
        let names = character_sheet(&scene)
            .unwrap()
            .into_iter()
            .map(|entry| (entry.source_name, entry.target_name, entry.notes))
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                ("a".into(), "a-t".into(), "rival".into()),
                ("b".into(), "b-t".into(), "rival".into())
            ]
        );
    }

    #[tokio::test]
    async fn only_terms_found_in_a_segment_reach_the_request() {
        let mut session = Session::memory().await.unwrap();
        let term = |source: &str| koharu_scene::Term {
            source: source.into(),
            target: format!("{source}-t"),
            note: "n".into(),
        };
        let patch = session
            .snapshot()
            .patch(|edit| {
                edit.set_project(&TermList {
                    terms: vec![term("魔王"), term("勇者"), term("")],
                })
            })
            .unwrap();
        let scene = session.commit(patch).await.unwrap().snapshot;
        let sent = page_terms(&scene, &["魔王城へ行く", "hello"]).unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(
            (sent[0].source.as_str(), sent[0].target.as_str()),
            ("魔王", "魔王-t")
        );
        assert!(page_terms(&scene, &["nothing"]).unwrap().is_empty());
    }

    #[tokio::test]
    async fn hints_use_last_n_translated_pages_in_order_with_user_flag() {
        let mut session = Session::memory().await.unwrap();
        let mut pages = Vec::new();
        let generated = generation(PRODUCER, "test").unwrap();
        let mut machine = Vec::new();
        let patch = session
            .snapshot()
            .patch(|edit| {
                // (source, translation, user-authored) per page; page 2 is untranslated.
                let rows: [&[(&str, Option<(&str, bool)>)]; 5] = [
                    &[("a", Some(("A", false)))],
                    &[("b", Some(("B", true)))],
                    &[("c", None)],
                    &[("d", Some(("D", false)))],
                    &[("e", None)],
                ];
                for (index, row) in rows.iter().enumerate() {
                    let page =
                        edit.add_page(PageDraft::new(index.to_string(), 1.0, 1.0), At::End)?;
                    for (source, translation) in *row {
                        let content = edit.add_text_content(page, At::End)?;
                        edit.add_text_layer(
                            page,
                            At::End,
                            content,
                            &TextLayout {
                                origin: koharu_scene::Origin::User,
                                kind: TextLayoutKind::Paragraph,
                                angle_degrees: None,
                            },
                        )?;
                        edit.set(
                            content,
                            &SourceText {
                                text: Authored::user((*source).to_owned()),
                                language: None,
                            },
                        )?;
                        if let Some((text, user)) = translation {
                            if *user {
                                edit.set(
                                    content,
                                    &Translation {
                                        text: Authored::user((*text).to_owned()),
                                        language: None,
                                    },
                                )?;
                            } else {
                                machine.push((content, (*text).to_owned()));
                            }
                        }
                    }
                    pages.push(page);
                }
                Ok(())
            })
            .unwrap();
        let scene = session.commit(patch).await.unwrap().snapshot;
        // Plain edits author as the user; generated text needs an edit made as the producer.
        let mut edit = scene.edit_as(generated);
        for (content, text) in machine {
            edit.set(
                content,
                &Translation {
                    text: Authored::generated(text, generation(PRODUCER, "test").unwrap()),
                    language: None,
                },
            )
            .unwrap();
        }
        let scene = session
            .commit(edit.finish().unwrap())
            .await
            .unwrap()
            .snapshot;
        let lines = |page, limit| {
            previous_page_hints(&scene, page, limit)
                .unwrap()
                .into_iter()
                .map(|hint| (hint.source, hint.translation, hint.user_corrected))
                .collect::<Vec<_>>()
        };

        // Untranslated page 2 is skipped and does not use up the window.
        assert_eq!(
            lines(pages[4], 2),
            [
                ("b".into(), "B".into(), true),
                ("d".into(), "D".into(), false)
            ]
        );
        assert_eq!(lines(pages[4], 1), [("d".into(), "D".into(), false)]);
        assert_eq!(lines(pages[4], 0), []);
        assert_eq!(lines(pages[0], 2), []);
    }

    async fn scene_with(
        sheet: Vec<koharu_scene::Character>,
        terms: Vec<koharu_scene::Term>,
        proposals: Proposals,
    ) -> Snapshot {
        let mut session = Session::memory().await.unwrap();
        let patch = session
            .snapshot()
            .patch(|edit| {
                edit.set_project(&CharacterSheet { characters: sheet })?;
                edit.set_project(&TermList { terms })?;
                edit.set_project(&proposals)
            })
            .unwrap();
        session.commit(patch).await.unwrap().snapshot
    }

    fn character(name: &str) -> koharu_scene::Character {
        koharu_scene::Character {
            source_name: name.into(),
            ..Default::default()
        }
    }

    fn term(source: &str) -> koharu_scene::Term {
        koharu_scene::Term {
            source: source.into(),
            target: format!("{source}-t"),
            note: String::new(),
        }
    }

    #[tokio::test]
    async fn pending_entries_are_excluded_unless_the_project_opts_in() {
        let mut proposals = Proposals {
            characters: vec![character("pending")],
            terms: vec![term("魔王")],
            ..Default::default()
        };
        let scene = scene_with(vec![character("approved")], vec![], proposals.clone()).await;
        let names = |scene: &Snapshot| {
            character_sheet(scene)
                .unwrap()
                .into_iter()
                .map(|c| (c.source_name, c.tentative))
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&scene), [("approved".into(), false)]);
        assert!(page_terms(&scene, &["魔王"]).unwrap().is_empty());

        proposals.use_in_prompts = true;
        let scene = scene_with(vec![character("approved")], vec![], proposals).await;
        assert_eq!(
            names(&scene),
            [("approved".into(), false), ("pending".into(), true)]
        );
        let terms = page_terms(&scene, &["魔王"]).unwrap();
        assert_eq!((terms.len(), terms[0].tentative), (1, true));
    }

    #[tokio::test]
    async fn returned_proposals_are_stored_pending_and_known_sources_are_dropped() {
        let scene = scene_with(
            vec![character("a")],
            vec![term("t")],
            Proposals {
                characters: vec![character("p")],
                terms: vec![term("pt")],
                rejected_characters: vec!["rc".into()],
                rejected_terms: vec!["rt".into()],
                ..Default::default()
            },
        )
        .await;
        let new = TranslationProposals {
            characters: ["new", "a", "p", "rc", "new", " "]
                .map(|name| TranslationCharacter {
                    source_name: name.into(),
                    target_name: "x".into(),
                    ..Default::default()
                })
                .into(),
            terms: ["newt", "t", "pt", "rt"]
                .map(|source| TranslationTerm {
                    source: source.into(),
                    ..Default::default()
                })
                .into(),
            ..Default::default()
        };
        let merged = merge_proposals(&scene, new).unwrap().unwrap();
        let sources = |p: &Proposals| {
            (
                p.characters
                    .iter()
                    .map(|c| c.source_name.clone())
                    .collect::<Vec<_>>(),
                p.terms.iter().map(|t| t.source.clone()).collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            sources(&merged),
            (
                vec!["p".into(), "new".into()],
                vec!["pt".into(), "newt".into()]
            )
        );
        // Rejections survive, and nothing was applied to the approved sheet or term list.
        assert_eq!(merged.rejected_characters, ["rc"]);
        assert_eq!(character_sheet(&scene).unwrap().len(), 1);
        assert!(
            merge_proposals(&scene, TranslationProposals::default())
                .unwrap()
                .is_none()
        );
        let mut known = known_sources(&scene).unwrap();
        known.sort();
        assert_eq!(known, ["p", "pt", "rc", "rt"]);
    }

    #[tokio::test]
    async fn updates_need_an_approved_entry_a_real_change_and_merge_into_pending_ones() {
        let approved = koharu_scene::Character {
            source_name: "a".into(),
            target_name: "Alice".into(),
            speech_style: "polite".into(),
            ..Default::default()
        };
        let pending = CharacterUpdate {
            source_name: "a".into(),
            speech_style: Some("curt".into()),
            notes: Some("old".into()),
            ..Default::default()
        };
        let scene = scene_with(
            vec![approved.clone()],
            vec![term("t")],
            Proposals {
                character_updates: vec![pending],
                ..Default::default()
            },
        )
        .await;
        let update =
            |name: &str, target: Option<&str>, style: Option<&str>, notes: Option<&str>| {
                let own = |v: Option<&str>| v.map(str::to_owned);
                koharu_translator::TranslationCharacterUpdate {
                    source_name: name.into(),
                    target_name: own(target),
                    speech_style: own(style),
                    notes: own(notes),
                    ..Default::default()
                }
            };
        let new = TranslationProposals {
            character_updates: vec![
                update("unknown", Some("x"), None, None),
                update("a", Some("Alice"), Some("polite"), None), // no-ops
                update("a", Some("Alicia"), None, Some("new")),   // merges
            ],
            term_updates: vec![
                koharu_translator::TranslationTermUpdate {
                    source: "missing".into(),
                    target: Some("x".into()),
                    ..Default::default()
                },
                koharu_translator::TranslationTermUpdate {
                    source: "t".into(),
                    target: Some("t-t".into()), // same as current
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let merged = merge_proposals(&scene, new).unwrap().unwrap();
        assert_eq!(
            merged.character_updates,
            [CharacterUpdate {
                source_name: "a".into(),
                target_name: Some("Alicia".into()),
                speech_style: Some("curt".into()),
                appearance: None,
                notes: Some("new".into()),
            }]
        );
        assert!(merged.term_updates.is_empty());
        // The approved sheet is untouched and pending updates are never sent.
        assert_eq!(character_sheet(&scene).unwrap()[0].target_name, "Alice");
        let noop = TranslationProposals {
            character_updates: vec![update("a", Some("Alice"), None, None)],
            ..Default::default()
        };
        assert!(merge_proposals(&scene, noop).unwrap().is_none());
    }

    #[tokio::test]
    async fn a_new_proposal_for_an_approved_entry_becomes_an_update() {
        let scene = scene_with(vec![character("a")], vec![term("t")], Proposals::default()).await;
        let new = TranslationProposals {
            characters: vec![TranslationCharacter {
                source_name: "a".into(),
                notes: "learned more".into(), // empty fields are not updates
                ..Default::default()
            }],
            terms: vec![TranslationTerm {
                source: "t".into(),
                note: "both like the band".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let merged = merge_proposals(&scene, new).unwrap().unwrap();
        assert!(merged.characters.is_empty() && merged.terms.is_empty());
        assert_eq!(
            merged.character_updates,
            [CharacterUpdate {
                source_name: "a".into(),
                notes: Some("learned more".into()),
                ..Default::default()
            }]
        );
        assert_eq!(
            merged.term_updates,
            [TermUpdate {
                source: "t".into(),
                target: None,
                note: Some("both like the band".into()),
            }]
        );
    }

    #[tokio::test]
    async fn retranslation_keeps_user_edits_and_clears_the_stale_marker() {
        let mut session = Session::memory().await.unwrap();
        let (mut page, mut machine, mut manual) = (None, None, None);
        let patch = session
            .snapshot()
            .patch(|edit| {
                let id = edit.add_page(PageDraft::new("p", 1.0, 1.0), At::End)?;
                let mut layer = |source: &str, translation: Option<Authored<String>>| {
                    let content = edit.add_text_content(id, At::End)?;
                    edit.add_text_layer(
                        id,
                        At::End,
                        content,
                        &TextLayout {
                            origin: Origin::User,
                            kind: TextLayoutKind::Paragraph,
                            angle_degrees: None,
                        },
                    )?;
                    edit.set(
                        content,
                        &SourceText {
                            text: Authored::user(source.to_owned()),
                            language: None,
                        },
                    )?;
                    if let Some(text) = translation {
                        edit.set(
                            content,
                            &Translation {
                                text,
                                language: None,
                            },
                        )?;
                    }
                    Ok::<_, koharu_scene::Error>(content)
                };
                machine = Some(layer("a", None)?);
                manual = Some(layer("b", Some(Authored::user("mine".to_owned())))?);
                edit.set(id, &StalePage {})?;
                page = Some(id);
                Ok(())
            })
            .unwrap();
        let scene = session.commit(patch).await.unwrap().snapshot;
        let (page, machine, manual) = (page.unwrap(), machine.unwrap(), manual.unwrap());
        // Make the first translation machine-authored; the second stays the user's.
        let generated = generation(PRODUCER, "test").unwrap();
        let mut edit = scene.edit_as(generated.clone());
        edit.set(
            machine,
            &Translation {
                text: Authored::generated("old".to_owned(), generated.clone()),
                language: None,
            },
        )
        .unwrap();
        let scene = session
            .commit(edit.finish().unwrap())
            .await
            .unwrap()
            .snapshot;

        let language = LanguageTag::new("en").unwrap();
        let mut edit = scene.edit_as(generated.clone());
        write_translations(
            &scene,
            &mut edit,
            page,
            vec![(machine, "a".into()), (manual, "b".into())],
            vec!["new".into(), "overwritten".into()],
            &language,
            &generated,
            false,
        )
        .unwrap();
        let scene = session
            .commit(edit.finish().unwrap())
            .await
            .unwrap()
            .snapshot;

        let text = |entity| {
            scene
                .component::<Translation>(entity)
                .unwrap()
                .unwrap()
                .text
                .value
        };
        assert_eq!((text(machine), text(manual)), ("new".into(), "mine".into()));
        assert!(scene.component::<StalePage>(page).unwrap().is_none());

        // One-box translate overwrites the hand edit as a single commit; undo restores it.
        let mut edit = scene.edit_as(generated.clone());
        edit.replace_user_edits();
        write_translations(
            &scene,
            &mut edit,
            page,
            vec![(manual, "b".into())],
            vec!["fresh".into()],
            &language,
            &generated,
            true,
        )
        .unwrap();
        let commit = session.commit(edit.finish().unwrap()).await.unwrap();
        assert_eq!(text_of(&commit.snapshot, manual), "fresh");
        let undone = session.undo(commit.revision).await.unwrap().snapshot;
        assert_eq!(text_of(&undone, manual), "mine");
    }

    fn text_of(scene: &Snapshot, entity: EntityId) -> String {
        scene
            .component::<Translation>(entity)
            .unwrap()
            .unwrap()
            .text
            .value
    }
}
