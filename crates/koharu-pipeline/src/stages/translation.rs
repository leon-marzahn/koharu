use anyhow::Result;
use async_trait::async_trait;
use koharu_scene::{
    Authored, CharacterSheet, EntityId, LanguageTag, Origin, Snapshot, SourceText, TermList,
    Translation,
};
use koharu_translator::{
    TranslationCharacter, TranslationContext, TranslationRequest, TranslationTerm, Translator,
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

/// Every character sheet entry, on every page: the sheet is small and always relevant.
fn character_sheet(scene: &Snapshot) -> Result<Vec<TranslationCharacter>> {
    Ok(scene
        .project_component::<CharacterSheet>()?
        .unwrap_or_default()
        .characters
        .into_iter()
        .map(|character| TranslationCharacter {
            source_name: character.source_name,
            target_name: character.target_name,
            speech_style: character.speech_style,
            appearance: character.appearance,
            notes: character.notes,
        })
        .collect())
}

/// Term list entries whose source is a substring of at least one segment of the page.
fn page_terms(scene: &Snapshot, segments: &[&str]) -> Result<Vec<TranslationTerm>> {
    Ok(scene
        .project_component::<TermList>()?
        .unwrap_or_default()
        .terms
        .into_iter()
        .filter(|term| {
            !term.source.is_empty() && segments.iter().any(|text| text.contains(&term.source))
        })
        .map(|term| TranslationTerm {
            source: term.source,
            target: term.target,
            note: term.note,
        })
        .collect())
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
        if let Some(group) = input.scene.page(input.page)?.text_group()? {
            for layer in group.text_layers()? {
                if !input.contains_entity(layer.id())? {
                    continue;
                }
                let content = layer.content()?;
                let Some(source) = content.source()? else {
                    continue;
                };
                if !source.text.value.trim().is_empty() {
                    targets.push((content.id(), source.text.value));
                }
            }
        }
        // Every context source (hints today; character sheet, term list later) appends here.
        let mut context = Vec::new();
        context.extend(previous_page_hints(
            &input.scene,
            input.page,
            self.config.previous_pages,
        )?);
        let mut request = TranslationRequest::new(
            targets.iter().map(|(_, source)| source.clone()),
            self.config.target_language,
        )
        .with_context(context)
        .with_characters(character_sheet(&input.scene)?)
        .with_terms(page_terms(
            &input.scene,
            &targets.iter().map(|(_, s)| s.as_str()).collect::<Vec<_>>(),
        )?);
        if let Some(instructions) = self.config.instructions.as_deref() {
            request = request.with_instructions(instructions);
        }
        if Translator::supports_vision(&self.config.model, &self.config.generation)
            && let Some(image) = input.images.get(&input.scene, input.page, "source").await?
        {
            request = request.with_image(image);
        }
        let (provider, translated) = self
            .translator
            .translate(&self.config.model, self.config.generation, request)
            .await?;
        let language = LanguageTag::new(self.config.target_language.tag())?;
        let generated = generation(PRODUCER, provider)?;
        let mut edit = input.scene.edit_as(generated.clone());
        for (entity, _) in &targets {
            edit.observe::<SourceText>(*entity)?;
            edit.observe::<Translation>(*entity)?;
        }
        for ((entity, source), text) in targets.into_iter().zip(translated) {
            if input
                .scene
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
        finish(edit)
    }
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
}
