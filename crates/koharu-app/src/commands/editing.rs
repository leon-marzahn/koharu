use anyhow::Context as _;
use koharu_desktop::{CanvasState, Desktop};
use koharu_scene::{Character, EntityId, PageStatus, Proposals, Term};
use serde::Deserialize;
use specta::Type;
use tauri::State;

use super::{
    ChannelExt as _, Error,
    canvas::{CanvasChannel, Point},
    project::{CurrentProject, Page, Project, Typography},
};
async fn synchronize_canvas(
    desktop: &Desktop,
    commit: &koharu_scene::Commit,
    page: Option<EntityId>,
) -> anyhow::Result<CanvasState> {
    desktop.synchronize(&commit.snapshot, page, commit).await?;
    Ok(desktop.canvas_state())
}

#[derive(Clone, Debug, Deserialize, Type)]
pub struct GeometryUpdate {
    pub layer: EntityId,
    pub points: Option<Vec<Point>>,
}

#[derive(Clone, Debug, Deserialize, Type)]
pub struct TypographyUpdate {
    pub layer: EntityId,
    pub typography: Typography,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn dismiss_stale(
    pages: Vec<EntityId>,
    project: State<'_, CurrentProject>,
) -> Result<(), Error> {
    let mut project = project.project.lock().await;
    let project = project.as_mut().context("no project is open")?;
    let commit = project.dismiss_stale(&pages).await?;
    project.record_commit(&commit);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_page_status(
    page: EntityId,
    status: Option<PageStatus>,
    project: State<'_, CurrentProject>,
) -> Result<(), Error> {
    let mut project = project.project.lock().await;
    let project = project.as_mut().context("no project is open")?;
    let commit = project.set_page_status(page, status).await?;
    project.record_commit(&commit);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_scene_description(
    page: EntityId,
    text: String,
    project: State<'_, CurrentProject>,
) -> Result<(), Error> {
    let mut project = project.project.lock().await;
    let project = project.as_mut().context("no project is open")?;
    let commit = project.set_scene_description(page, text).await?;
    project.record_commit(&commit);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "page_renamed",
    skip_all,
    fields(origin = "user", character_count = label.chars().count(), empty = label.is_empty()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn rename_page(
    page: EntityId,
    label: String,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.rename_page(page, label).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "pages_deleted",
    skip_all,
    fields(origin = "user", entity_count = pages.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_pages(
    pages: Vec<EntityId>,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.delete_pages(pages).await?;
        project.record_commit(&commit);
        project.reconcile_page();
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "page_moved",
    skip_all,
    fields(origin = "user", page_number = index + 1),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn move_page(
    page: EntityId,
    index: u32,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.move_page(page, index as usize).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "source_text_edited",
    skip_all,
    fields(origin = "user", character_count = text.chars().count(), empty = text.is_empty()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_source_text(
    layer: EntityId,
    text: String,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.set_source_text(layer, text).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "translation_edited",
    skip_all,
    fields(
        origin = "user",
        character_count = text.as_ref().map_or(0, |text| text.chars().count()),
        empty = text.as_ref().is_none_or(String::is_empty),
    ),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_translation(
    layer: EntityId,
    text: Option<String>,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.set_translation(layer, text).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "characters_edited",
    skip_all,
    fields(origin = "user", entry_count = characters.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_characters(
    characters: Vec<Character>,
    project: State<'_, CurrentProject>,
) -> Result<(), Error> {
    let mut project = project.project.lock().await;
    let project = project.as_mut().context("no project is open")?;
    let commit = project.set_characters(characters).await?;
    project.record_commit(&commit);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "proposals_reviewed",
    skip_all,
    fields(origin = "user", pending = proposals.characters.len() + proposals.terms.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_context(
    characters: Vec<Character>,
    terms: Vec<Term>,
    proposals: Proposals,
    project: State<'_, CurrentProject>,
) -> Result<(), Error> {
    let mut project = project.project.lock().await;
    let project = project.as_mut().context("no project is open")?;
    let commit = project.set_context(characters, terms, proposals).await?;
    project.record_commit(&commit);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "terms_edited",
    skip_all,
    fields(origin = "user", entry_count = terms.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_terms(
    terms: Vec<Term>,
    project: State<'_, CurrentProject>,
) -> Result<(), Error> {
    let mut project = project.project.lock().await;
    let project = project.as_mut().context("no project is open")?;
    let commit = project.set_terms(terms).await?;
    project.record_commit(&commit);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "typography_edited",
    skip_all,
    fields(origin = "user", entity_count = updates.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_typography(
    updates: Vec<TypographyUpdate>,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.set_typography(updates).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "geometry_edited",
    skip_all,
    fields(origin = "user", entity_count = updates.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_geometry(
    updates: Vec<GeometryUpdate>,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.set_geometry(updates).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "region_geometry_edited",
    skip_all,
    fields(origin = "user", entity_count = updates.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_region_geometry(
    updates: Vec<GeometryUpdate>,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.set_region_geometry(updates).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "regions_deleted",
    skip_all,
    fields(origin = "user", entity_count = regions.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_regions(
    regions: Vec<EntityId>,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.delete_regions(regions).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

/// Returns the new region so the caller can scope OCR and translation to it.
#[tracing::instrument(
    target = "koharu_metrics",
    name = "region_drawn",
    skip_all,
    fields(origin = "user")
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn draw_region(
    page: EntityId,
    from: Point,
    to: Point,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<EntityId, Error> {
    let (commit, region, active) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let (commit, region) = project.add_text_region(page, from, to).await?;
        project.record_commit(&commit);
        (commit, region, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, active).await?;
    canvas_channel.channel.publish(canvas);
    Ok(region)
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "visibility_edited",
    skip_all,
    fields(origin = "user", entity_count = layers.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn set_visibility(
    layers: Vec<EntityId>,
    visible: Option<bool>,
    opacity: Option<f32>,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.set_visibility(layers, visible, opacity).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "layers_deleted",
    skip_all,
    fields(origin = "user", entity_count = layers.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_layers(
    layers: Vec<EntityId>,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.delete_layers(layers).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "layer_moved",
    skip_all,
    fields(origin = "user", entity_count = 1_u64)
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn move_layer(
    layer: EntityId,
    parent: EntityId,
    index: u32,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<Page, Error> {
    let (commit, page, view) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.move_layer(layer, parent, index as usize).await?;
        project.record_commit(&commit);
        let page = project.active_page().context("no active page")?;
        let view = Project::page(&commit.snapshot, page)?;
        (commit, page, view)
    };
    desktop
        .synchronize(&commit.snapshot, Some(page), &commit)
        .await?;
    canvas_channel.channel.publish(desktop.canvas_state());
    Ok(view)
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "undo",
    skip_all,
    fields(origin = "user")
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn undo(
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.undo().await?;
        project.reconcile_page();
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "redo",
    skip_all,
    fields(origin = "user")
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn redo(
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<(), Error> {
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = project.redo().await?;
        project.reconcile_page();
        (commit, project.active_page())
    };
    let canvas = synchronize_canvas(&desktop, &commit, page).await?;
    canvas_channel.channel.publish(canvas);
    Ok(())
}
