use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use image::{DynamicImage, GenericImageView as _};
use koharu_scene::{
    Authored, CharacterSheet, CharacterUpdate, EntityId, Generation, LanguageTag, Locked, Origin,
    PageRef, Proposals, RasterLayer, RasterLayerKind, SceneDescription, Snapshot, SourceText,
    StalePage, TermList, TermUpdate, Translation,
};
use koharu_translator::{
    PreviousDescription, Translated, TranslationCharacter, TranslationCharacterUpdate,
    TranslationContext, TranslationProposals, TranslationRequest, TranslationTerm,
    TranslationTermUpdate, Translator,
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

/// Whether this run asks the description model for the page. A one-box run never does, and a
/// locked description is kept; every other page run describes the page again.
fn should_describe(configured: bool, scoped_to_entities: bool, description_locked: bool) -> bool {
    configured && !scoped_to_entities && !description_locked
}

/// The boxes this run translates (in scope, with source text, not locked) and every locked text
/// layer on the page. Locked boxes go to the model as context instead.
fn translatable_boxes(input: &StageInput) -> Result<(Vec<(EntityId, String)>, Vec<EntityId>)> {
    let mut targets = Vec::new();
    let mut locked = Vec::new();
    if let Some(group) = input.scene.page(input.page)?.text_group()? {
        for layer in group.text_layers()? {
            if input.scene.component::<Locked>(layer.id())?.is_some() {
                locked.push(layer.id());
                continue;
            }
            if !input.contains_entity(layer.id())? {
                continue;
            }
            let content = layer.content()?;
            if let Some(source) = content.source()?
                && !source.text.value.trim().is_empty()
            {
                targets.push((content.id(), source.text.value));
            }
        }
    }
    Ok((targets, locked))
}

/// What `pick` finds on the last `limit` pages before `page` that have anything, in reading
/// order. `pick` gets the page's 1-based number; pages it returns `None` for use no window.
fn previous_pages<T>(
    scene: &Snapshot,
    page: EntityId,
    limit: u32,
    mut pick: impl FnMut(usize, PageRef<'_>) -> Result<Option<T>>,
) -> Result<Vec<T>> {
    let mut found = Vec::new();
    for (index, candidate) in scene
        .pages()
        .enumerate()
        .take_while(|(_, candidate)| candidate.id() != page)
    {
        found.extend(pick(index + 1, candidate)?);
    }
    let skip = found.len().saturating_sub(limit as usize);
    Ok(found.split_off(skip))
}

/// Source/translation pairs from the last `limit` pages before `page` that have any translation,
/// in reading order. Uses the current translation, so user edits win and are flagged.
fn previous_page_hints(
    scene: &Snapshot,
    page: EntityId,
    limit: u32,
) -> Result<Vec<TranslationContext>> {
    let pages = previous_pages(scene, page, limit, |_, candidate| {
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
        Ok((!lines.is_empty()).then_some(lines))
    })?;
    Ok(pages.into_iter().flatten().collect())
}

/// The Scene Descriptions of the last `limit` described pages before `page`, oldest first.
fn previous_descriptions(
    scene: &Snapshot,
    page: EntityId,
    limit: u32,
) -> Result<Vec<PreviousDescription>> {
    previous_pages(scene, page, limit, |number, candidate| {
        Ok(scene
            .component::<SceneDescription>(candidate.id())?
            .filter(|description| !description.text.trim().is_empty())
            .map(|description| PreviousDescription {
                page: number,
                text: description.text,
            }))
    })
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
            || merged.rejected_characters.iter().any(|name| known(name))
        {
            continue;
        }
        // The description and translation calls both propose a new character: fill what the
        // pending entry lacks instead of dropping the second sighting.
        if let Some(pending) = merged.characters.iter_mut().find(|c| known(&c.source_name)) {
            for (field, new) in [
                (&mut pending.target_name, character.target_name),
                (&mut pending.speech_style, character.speech_style),
                (&mut pending.appearance, character.appearance),
                (&mut pending.notes, character.notes),
            ] {
                if field.trim().is_empty() {
                    *field = new;
                }
            }
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

/// The page for the description: the source with its Cleanup layers overlaid (the same overlay
/// inpainting uses), so dialogue does not get summarised. No renderer: it would also draw text and
/// paint layers. Falls back to the plain source when there is no cleanup layer.
async fn clean_page(input: &StageInput) -> Result<Option<Arc<DynamicImage>>> {
    let Some(source) = input.images.get(&input.scene, input.page, "source").await? else {
        return Ok(None);
    };
    let mut page = None;
    for entity in input.scene.children(input.page)? {
        if input
            .scene
            .component::<RasterLayer>(entity)?
            .is_none_or(|layer| layer.kind != RasterLayerKind::Cleanup)
        {
            continue;
        }
        let Some(cleanup) = input.images.get(&input.scene, entity, "source").await? else {
            continue;
        };
        anyhow::ensure!(
            cleanup.dimensions() == source.dimensions(),
            "cleanup layer dimensions do not match page {}",
            input.page
        );
        let page = page.get_or_insert_with(|| source.to_rgba8());
        image::imageops::overlay(page, &cleanup.to_rgba8(), 0, 0);
    }
    Ok(Some(match page {
        Some(page) => Arc::new(DynamicImage::ImageRgba8(page)),
        None => source,
    }))
}

impl Processor {
    /// The description request: page image plus Character sheet, sent to the description model
    /// (its own provider config, usually not translation's). `None` when no model is set or the
    /// page has no image. Shared by Translate and the Inspector's Describe.
    async fn describe(
        &self,
        input: &StageInput,
        characters: &[TranslationCharacter],
    ) -> Result<Option<Translated>> {
        let Some(model) = self.config.description_model.as_ref() else {
            return Ok(None);
        };
        let Some(image) = clean_page(input).await? else {
            return Ok(None);
        };
        let mut request = TranslationRequest::describing(self.config.target_language)
            .with_characters(characters.to_vec())
            .with_previous_descriptions(previous_descriptions(
                &input.scene,
                input.page,
                self.config.description_previous_pages,
            )?)
            .with_image(image);
        if let Some(instructions) = self.config.description_instructions.as_deref() {
            request = request.with_instructions(instructions);
        }
        let report = |active: bool, model: &str| {
            crate::progress::emit(
                input.progress.as_ref(),
                crate::Progress::Describing {
                    page: input.page,
                    active,
                    model: model.to_owned(),
                },
            );
        };
        report(true, model.provider.name());
        let described = self
            .translator
            .describe(model, self.config.description_generation(), request)
            .await;
        report(false, Translator::model(&self.config.model));
        described.map(Some)
    }

    /// Describe only: writes the page's description and the appearance proposals, nothing else.
    /// Unlike a Translate run, a failure is returned so the user sees it.
    async fn describe_only(&self, input: StageInput) -> Result<koharu_scene::Patch> {
        let model = self
            .config
            .description_model
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no description model is set"))?;
        anyhow::ensure!(
            input.scene.component::<Locked>(input.page)?.is_none(),
            "the description is locked"
        );
        let found = self
            .describe(&input, &character_sheet(&input.scene)?)
            .await?
            .ok_or_else(|| anyhow::anyhow!("the page has no image to describe"))?;
        let text = found
            .description
            .ok_or_else(|| anyhow::anyhow!("the model returned no description"))?;
        // The description model's provider stamps the edit, since it is not the translation's.
        let generated = generation(PRODUCER, model.provider.into())?;
        let mut edit = input.scene.edit_as(generated.clone());
        // An explicit request replaces an unlocked description, hand-written or not.
        edit.replace_user_edits();
        edit.set(
            input.page,
            &SceneDescription {
                text,
                origin: Origin::Generated(generated),
            },
        )?;
        if let Some(merged) = merge_proposals(&input.scene, found.proposals)? {
            edit.set_project(&merged)?;
        }
        finish(edit)
    }
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

    /// A page whose boxes are all locked has nothing to translate; it still runs when the
    /// description will be redone.
    fn skip(&self, input: &StageInput) -> Result<bool> {
        if input.describe_only {
            return Ok(false);
        }
        let describes = self.config.description_model.is_some()
            && should_describe(
                true,
                input.entities.is_some(),
                input.scene.component::<Locked>(input.page)?.is_some(),
            );
        Ok(!describes && translatable_boxes(input)?.0.is_empty())
    }

    async fn process(&self, input: StageInput) -> Result<koharu_scene::Patch> {
        if input.describe_only {
            return self.describe_only(input).await;
        }
        let (targets, locked) = translatable_boxes(&input)?;
        // Entity scope translates only the chosen segments; the page's other segments go to the
        // model as context so a single box reads like it does in a full run. Locked boxes are
        // context in every run, with their current translation.
        let mut siblings = Vec::new();
        if let Some(group) = input.scene.page(input.page)?.text_group()? {
            for layer in group.text_layers()? {
                let content = layer.content()?;
                let Some(source) = content.source()? else {
                    continue;
                };
                if source.text.value.trim().is_empty() {
                    continue;
                }
                let is_target = input.contains_entity(layer.id())? && !locked.contains(&layer.id());
                if !is_target && (input.entities.is_some() || locked.contains(&layer.id())) {
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
        let characters = character_sheet(&input.scene)?;
        let existing = input.scene.component::<SceneDescription>(input.page)?;
        let description_locked = input.scene.component::<Locked>(input.page)?.is_some();
        let mut description = existing.as_ref().map(|d| d.text.clone());
        let mut described = None;
        let mut appearances = Vec::new();
        if should_describe(true, input.entities.is_some(), description_locked) {
            // ponytail: a failed description only costs context, so translation goes on without
            // it; the Inspector's Describe button is where the failure is shown.
            match self.describe(&input, &characters).await {
                Ok(Some(found)) => {
                    appearances = found.proposals.characters;
                    if let Some(text) = found.description {
                        description = Some(text.clone());
                        described = Some(text);
                    }
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "scene description failed"),
            }
        }
        let mut request = TranslationRequest::new(
            targets.iter().map(|(_, source)| source.clone()),
            self.config.target_language,
        )
        .with_scene_description(description)
        .with_context(context)
        .with_characters(characters)
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
        // Every box is locked: only the description is redone, so there is nothing to translate.
        // The description model's provider stamps the edit, since it is not the translation's.
        let (provider, mut translated) = match (targets.is_empty(), &self.config.description_model)
        {
            (true, Some(model)) => (model.provider.into(), Translated::default()),
            _ => {
                self.translator
                    .translate(&self.config.model, self.config.generation, request)
                    .await?
            }
        };
        // ponytail: the explanation only travels as a progress event, so reloading loses it; the
        // upgrade is a stored component (revision, serde default, load test) written in this edit.
        if let (Some(_), Some(text)) = (&input.explain, translated.explanation.take()) {
            crate::progress::emit(
                input.progress.as_ref(),
                crate::Progress::Explained {
                    page: input.page,
                    text,
                },
            );
        }
        let language = LanguageTag::new(self.config.target_language.tag())?;
        let generated = generation(PRODUCER, provider)?;
        let mut edit = input.scene.edit_as(generated.clone());
        // Locks are the only protection: whatever is unlocked is the user's request to redo,
        // hand-edited or not, so the authorship check must not block this edit.
        edit.replace_user_edits();
        for (entity, _) in &targets {
            edit.observe::<SourceText>(*entity)?;
            edit.observe::<Translation>(*entity)?;
        }
        if let Some(text) = described {
            edit.set(
                input.page,
                &SceneDescription {
                    text,
                    origin: Origin::Generated(generated.clone()),
                },
            )?;
        }
        // ponytail: replaces the whole component from this page's snapshot, so a review made while
        // pages translate concurrently can be overwritten; add project-level observation if it bites.
        // Appearance proposals ride with the translation call's so one new character is one entry.
        translated.proposals.characters.extend(appearances);
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
            input.entities.is_some(),
        )?;
        finish(edit)
    }
}

/// Writes generated translations for the unlocked targets and clears the page's stale marker:
/// the page is now translated against the current sheet and term list.
fn write_translations(
    scene: &Snapshot,
    edit: &mut koharu_scene::Edit,
    page: EntityId,
    targets: Vec<(EntityId, String)>,
    segments: Vec<String>,
    language: &LanguageTag,
    generated: &Generation,
    scoped: bool,
) -> Result<()> {
    for ((entity, source), text) in targets.into_iter().zip(segments) {
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
    // A scoped run leaves the rest of the page stale.
    if !scoped && scene.component::<StalePage>(page)?.is_some() {
        edit.remove::<StalePage>(page)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use image::{Rgba, RgbaImage};
    use koharu_scene::{At, Authored, PageDraft, Session, TextLayout, TextLayoutKind, Translation};

    use super::*;

    fn png(image: &DynamicImage) -> Arc<[u8]> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        Arc::<[u8]>::from(bytes.into_inner())
    }

    async fn page_input(cleanup: Option<RgbaImage>) -> StageInput {
        use koharu_scene::{AssetInput, AssetMetadata, AssetRole};
        let asset = |image: &DynamicImage| {
            AssetInput::new(
                png(image),
                "image/png",
                AssetMetadata {
                    width: Some(4),
                    height: Some(4),
                    attributes: Default::default(),
                },
            )
        };
        let mut session = Session::memory().await.unwrap();
        let mut page = None;
        let patch = session
            .snapshot()
            .patch(|edit| {
                let id = edit.add_page(PageDraft::new("page", 4.0, 4.0), At::End)?;
                let source = DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([9; 4])));
                edit.set_asset(id, &AssetRole::new("source")?, asset(&source))?;
                if let Some(cleanup) = &cleanup {
                    let layer = edit.add_entity(id, At::Start)?;
                    edit.set(
                        layer,
                        &RasterLayer {
                            origin: Origin::User,
                            name: "Cleanup".to_owned(),
                            kind: RasterLayerKind::Cleanup,
                        },
                    )?;
                    let cleanup = DynamicImage::ImageRgba8(cleanup.clone());
                    edit.set_asset(layer, &AssetRole::new("source")?, asset(&cleanup))?;
                }
                page = Some(id);
                Ok(())
            })
            .unwrap();
        let scene = session.commit(patch).await.unwrap().snapshot;
        StageInput::new(
            scene,
            page.unwrap(),
            None,
            None,
            Arc::new(crate::ImageCache::default()),
            None,
        )
    }

    #[tokio::test]
    async fn description_image_has_cleanup_composited_else_is_the_source() {
        let plain = clean_page(&page_input(None).await).await.unwrap().unwrap();
        assert_eq!(plain.to_rgba8().get_pixel(1, 1), &Rgba([9; 4]));

        let mut cleanup = RgbaImage::new(4, 4);
        cleanup.put_pixel(1, 1, Rgba([200, 0, 0, 255]));
        let cleaned = clean_page(&page_input(Some(cleanup)).await)
            .await
            .unwrap()
            .unwrap()
            .to_rgba8();
        assert_eq!(cleaned.get_pixel(1, 1), &Rgba([200, 0, 0, 255]));
        assert_eq!(cleaned.get_pixel(0, 0), &Rgba([9; 4]));
    }

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

    /// Page numbers are project order; undescribed pages are skipped and use no window; a
    /// description committed since the last snapshot is seen by the next page's lookup.
    #[tokio::test]
    async fn previous_descriptions_are_the_last_n_described_pages_oldest_first() {
        let mut session = Session::memory().await.unwrap();
        let mut pages = Vec::new();
        let patch = session
            .snapshot()
            .patch(|edit| {
                for index in 0..4 {
                    pages
                        .push(edit.add_page(PageDraft::new(index.to_string(), 1.0, 1.0), At::End)?);
                }
                edit.set(pages[0], &described("one"))?;
                edit.set(pages[2], &described("three"))?;
                Ok(())
            })
            .unwrap();
        let scene = session.commit(patch).await.unwrap().snapshot;
        let texts = |scene: &Snapshot, page, limit| {
            previous_descriptions(scene, page, limit)
                .unwrap()
                .into_iter()
                .map(|found| (found.page, found.text))
                .collect::<Vec<_>>()
        };
        assert_eq!(texts(&scene, pages[3], 0), []);
        assert_eq!(texts(&scene, pages[3], 1), [(3, "three".into())]);
        assert_eq!(
            texts(&scene, pages[3], 5),
            [(1, "one".into()), (3, "three".into())]
        );
        assert_eq!(texts(&scene, pages[0], 5), []);

        // The previous page of a bulk run: its description lands, the next lookup sees it.
        assert_eq!(texts(&scene, pages[2], 1), [(1, "one".into())]);
        let mut edit = scene.edit();
        edit.set(pages[1], &described("two")).unwrap();
        let scene = session
            .commit(edit.finish().unwrap())
            .await
            .unwrap()
            .snapshot;
        assert_eq!(texts(&scene, pages[2], 1), [(2, "two".into())]);
    }

    fn described(text: &str) -> SceneDescription {
        SceneDescription {
            text: text.into(),
            origin: Origin::User,
        }
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

    #[test]
    fn description_runs_only_when_configured_unscoped_and_unlocked() {
        assert!(should_describe(true, false, false));
        assert!(!should_describe(true, false, true));
        assert!(!should_describe(false, false, false));
        assert!(!should_describe(true, true, false));
    }

    /// Locked boxes are context, not targets; a page of locked boxes is skipped unless the
    /// description will be redone.
    #[tokio::test]
    async fn locked_boxes_are_not_translated_and_an_all_locked_page_is_skipped() {
        let mut session = Session::memory().await.unwrap();
        let (mut page, mut layers) = (None, Vec::new());
        let patch = session
            .snapshot()
            .patch(|edit| {
                let id = edit.add_page(PageDraft::new("p", 1.0, 1.0), At::End)?;
                for source in ["a", "b"] {
                    let content = edit.add_text_content(id, At::End)?;
                    let layer = edit.add_text_layer(
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
                    layers.push(layer);
                }
                page = Some(id);
                Ok(())
            })
            .unwrap();
        let scene = session.commit(patch).await.unwrap().snapshot;
        let page = page.unwrap();
        let input = |scene: &Snapshot| {
            StageInput::new(
                scene.clone(),
                page,
                None,
                None,
                std::sync::Arc::new(crate::ImageCache::default()),
                None,
            )
        };
        let processor = |description_model| {
            Processor::new(
                TranslationConfig {
                    description_model,
                    ..TranslationConfig::default()
                },
                Translator::from_config(
                    koharu_ml::Device::cpu(),
                    koharu_config::Config::memory(koharu_translator::ProvidersConfig::default()),
                )
                .unwrap(),
            )
        };
        let (targets, locked) = translatable_boxes(&input(&scene)).unwrap();
        assert_eq!((targets.len(), locked.len()), (2, 0));

        let mut edit = scene.edit();
        edit.set(layers[0], &Locked {}).unwrap();
        let scene = session
            .commit(edit.finish().unwrap())
            .await
            .unwrap()
            .snapshot;
        let (targets, locked) = translatable_boxes(&input(&scene)).unwrap();
        assert_eq!((targets.len(), locked), (1, vec![layers[0]]));
        assert!(!processor(None).skip(&input(&scene)).unwrap());

        let mut edit = scene.edit();
        edit.set(layers[1], &Locked {}).unwrap();
        let scene = session
            .commit(edit.finish().unwrap())
            .await
            .unwrap()
            .snapshot;
        assert!(processor(None).skip(&input(&scene)).unwrap());
        let described = Some(koharu_translator::ModelSelection {
            provider: koharu_translator::Provider::LmStudio,
            model: None,
            quantization: None,
            vision: true,
            reasoning: false,
        });
        assert!(!processor(described.clone()).skip(&input(&scene)).unwrap());

        // A locked description is kept, so only then is the all-locked page skipped.
        let mut edit = scene.edit();
        edit.set(page, &Locked {}).unwrap();
        let scene = session
            .commit(edit.finish().unwrap())
            .await
            .unwrap()
            .snapshot;
        assert!(processor(described).skip(&input(&scene)).unwrap());
    }

    /// A pipeline run can neither add nor clear a lock.
    #[tokio::test]
    async fn a_pipeline_run_cannot_add_or_remove_a_lock() {
        let mut session = Session::memory().await.unwrap();
        let mut page = None;
        let patch = session
            .snapshot()
            .patch(|edit| {
                let id = edit.add_page(PageDraft::new("p", 1.0, 1.0), At::End)?;
                edit.set(id, &Locked {})?;
                page = Some(id);
                Ok(())
            })
            .unwrap();
        let scene = session.commit(patch).await.unwrap().snapshot;
        let generated = generation(PRODUCER, "test").unwrap();
        let mut edit = scene.edit_as(generated.clone());
        edit.replace_user_edits();
        assert!(edit.remove::<Locked>(page.unwrap()).is_err());
        let mut edit = scene.edit_as(generated);
        assert!(edit.set(page.unwrap(), &Locked {}).is_err());
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
    async fn description_and_translation_proposals_for_one_character_merge() {
        let scene = scene_with(vec![], vec![], Proposals::default()).await;
        let named = TranslationCharacter {
            source_name: "ヒナ".into(),
            target_name: "Hina".into(),
            speech_style: "curt".into(),
            ..Default::default()
        };
        let seen = TranslationCharacter {
            source_name: "ヒナ".into(),
            target_name: "Other".into(),
            appearance: "red scarf".into(),
            ..Default::default()
        };
        // Either order gives one pending entry; what the first call said is not overwritten.
        for (first, second, target) in [(&named, &seen, "Hina"), (&seen, &named, "Other")] {
            let new = TranslationProposals {
                characters: vec![first.clone(), second.clone()],
                ..Default::default()
            };
            let merged = merge_proposals(&scene, new).unwrap().unwrap();
            assert_eq!(merged.characters.len(), 1);
            let merged = &merged.characters[0];
            assert_eq!(merged.target_name, target);
            assert_eq!(merged.speech_style, "curt");
            assert_eq!(merged.appearance, "red scarf");
        }

        // An appearance-only sighting becomes a pending character; a rejected one is dropped.
        let scene = scene_with(
            vec![],
            vec![],
            Proposals {
                rejected_characters: vec!["ミト".into()],
                ..Default::default()
            },
        )
        .await;
        let new = TranslationProposals {
            characters: ["ヒナ", "ミト"]
                .map(|name| TranslationCharacter {
                    source_name: name.into(),
                    appearance: "red scarf".into(),
                    ..Default::default()
                })
                .into(),
            ..Default::default()
        };
        let merged = merge_proposals(&scene, new).unwrap().unwrap();
        assert_eq!(merged.characters.len(), 1);
        assert_eq!(merged.characters[0].appearance, "red scarf");
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
    async fn retranslation_replaces_unlocked_hand_edits_and_clears_the_stale_marker() {
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
        edit.replace_user_edits();
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
        assert_eq!(
            (text(machine), text(manual)),
            ("new".into(), "overwritten".into())
        );
        assert!(scene.component::<StalePage>(page).unwrap().is_none());

        // One-box translate overwrites as a single commit; undo restores the earlier text.
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
        assert_eq!(text_of(&undone, manual), "overwritten");
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
