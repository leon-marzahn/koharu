use std::sync::Arc;

use anyhow::{Context as _, anyhow, bail};
use image::{GrayImage, ImageEncoder as _, codecs::png::PngEncoder};
use koharu_desktop::{CanvasState, Desktop, Frame, TransformFrame};
use koharu_rasterizer::ResourceId;
use koharu_scene::{EntityId, Geometry, Revision};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{
    AppHandle, Manager as _, State,
    ipc::{Channel, IpcResponse},
};
use tauri_runtime_cef::CefRuntime;

use super::{
    ChannelExt as _, Error, processing,
    processing::{JobChannel, JobId, Processing},
    project::{CurrentProject, Page, Project, RasterStrokeMode},
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// One press-to-release polyline of Paint mask. It carries its own diameter, so changing the
/// brush size later never resizes strokes already drawn; `erase` removes mask instead of adding.
#[derive(Clone, Debug, Deserialize, Type)]
pub struct MaskStroke {
    pub points: Vec<Point>,
    pub diameter: f32,
    pub erase: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Type)]
pub struct PaintBrush {
    pub diameter: f32,
    pub color: [u8; 4],
}

#[derive(Clone, Copy, Debug, Serialize, Type)]
pub struct LayerCommit {
    pub revision: Revision,
    pub layer: EntityId,
}

#[derive(Type)]
#[specta(transparent)]
pub(crate) struct CanvasBytes(#[specta(type = Vec<u8>)] Vec<u8>);

#[derive(Clone, Copy, Deserialize, Type)]
#[specta(transparent)]
pub(crate) struct CanvasGeneration(#[specta(type = f64)] u64);

#[derive(Clone, Debug, Serialize, Type)]
pub struct CanvasPagePreparation {
    pub revision: Revision,
    pub page: Page,
}

impl IpcResponse for CanvasBytes {
    fn body(self) -> tauri::Result<tauri::ipc::InvokeResponseBody> {
        Ok(self.0.into())
    }
}

#[derive(Default)]
pub(crate) struct CanvasChannel {
    pub(crate) channel: Mutex<Option<Channel<CanvasState>>>,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_canvas_manifest(
    generation: CanvasGeneration,
    desktop: State<'_, Desktop>,
) -> Result<CanvasBytes, Error> {
    Ok(CanvasBytes(desktop.frame_manifest_bytes(generation.0)?))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_canvas_resource(
    generation: CanvasGeneration,
    resource: String,
    desktop: State<'_, Desktop>,
) -> Result<CanvasBytes, Error> {
    let resource = resource
        .parse::<ResourceId>()
        .context("canvas resource id is invalid")?;
    Ok(CanvasBytes(
        desktop.frame_resource_bytes(generation.0, resource)?,
    ))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn prepare_canvas_page(
    page: EntityId,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
) -> Result<Option<CanvasPagePreparation>, Error> {
    let (snapshot, prepared_page) = {
        let project = project.project.lock().await;
        let project = project.as_ref().context("no project is open")?;
        let snapshot = project.snapshot();
        let prepared_page = Project::page(&snapshot, page)?;
        (snapshot, prepared_page)
    };
    let revision = snapshot.revision();
    Ok(desktop
        .prepare_page(&snapshot, page)
        .await?
        .then_some(CanvasPagePreparation {
            revision,
            page: prepared_page,
        }))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_canvas_page_manifest(
    page: EntityId,
    revision: Revision,
    desktop: State<'_, Desktop>,
) -> Result<CanvasBytes, Error> {
    Ok(CanvasBytes(desktop.page_manifest_bytes(page, revision)?))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_canvas_page_resource(
    page: EntityId,
    revision: Revision,
    resource: String,
    desktop: State<'_, Desktop>,
) -> Result<CanvasBytes, Error> {
    let resource = resource
        .parse::<ResourceId>()
        .context("canvas resource id is invalid")?;
    Ok(CanvasBytes(
        desktop.page_resource_bytes(page, revision, resource)?,
    ))
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "point_text_added",
    skip_all,
    fields(origin = "user", point_count = 1_u64)
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_point_text(
    point: Point,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<LayerCommit, Error> {
    let (commit, page, layer) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let page = project
            .active_page()
            .context("the project has no active page")?;
        let (commit, layer) = project.add_point_text(page, point).await?;
        project.record_commit(&commit);
        (commit, project.active_page(), layer)
    };
    desktop.synchronize(&commit.snapshot, page, &commit).await?;
    canvas_channel.channel.publish(desktop.canvas_state());
    Ok(LayerCommit {
        revision: commit.revision,
        layer,
    })
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "text_box_added",
    skip_all,
    fields(origin = "user", entity_count = 1_u64)
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_text_box(
    frame: Frame,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<LayerCommit, Error> {
    let (commit, page, layer) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let page = project
            .active_page()
            .context("the project has no active page")?;
        let (commit, layer) = project.add_text_box(page, frame).await?;
        project.record_commit(&commit);
        (commit, project.active_page(), layer)
    };
    desktop.synchronize(&commit.snapshot, page, &commit).await?;
    canvas_channel.channel.publish(desktop.canvas_state());
    Ok(LayerCommit {
        revision: commit.revision,
        layer,
    })
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "paint_committed",
    skip_all,
    fields(origin = "user", point_count = points.len(), size = f64::from(brush.diameter)),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn commit_paint(
    expected_revision: Revision,
    layer: Option<EntityId>,
    points: Vec<Point>,
    brush: PaintBrush,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<LayerCommit, Error> {
    commit_raster_stroke(
        expected_revision,
        layer,
        points,
        brush.diameter,
        brush.color,
        RasterStrokeMode::Paint,
        &desktop,
        &project,
        &canvas_channel,
    )
    .await
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "erase_committed",
    skip_all,
    fields(origin = "user", point_count = points.len(), size = f64::from(diameter)),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn commit_erase(
    expected_revision: Revision,
    layer: EntityId,
    points: Vec<Point>,
    diameter: f32,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<LayerCommit, Error> {
    commit_raster_stroke(
        expected_revision,
        Some(layer),
        points,
        diameter,
        [0; 4],
        RasterStrokeMode::Erase,
        &desktop,
        &project,
        &canvas_channel,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn commit_raster_stroke(
    expected_revision: Revision,
    layer: Option<EntityId>,
    points: Vec<Point>,
    diameter: f32,
    color: [u8; 4],
    mode: RasterStrokeMode,
    desktop: &Desktop,
    project: &CurrentProject,
    canvas_channel: &CanvasChannel,
) -> Result<LayerCommit, Error> {
    let (commit, page, element) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        ensure_revision(project.snapshot().revision(), expected_revision)?;
        let page = project
            .active_page()
            .context("the project has no active page")?;
        let (commit, element) = project
            .apply_raster_stroke(
                page,
                layer,
                mode,
                color,
                diameter,
                points
                    .into_iter()
                    .map(|point| koharu_scene::Point {
                        x: point.x,
                        y: point.y,
                    })
                    .collect(),
            )
            .await?;
        project.record_commit(&commit);
        (commit, project.active_page(), element)
    };
    desktop.synchronize(&commit.snapshot, page, &commit).await?;
    canvas_channel.channel.publish(desktop.canvas_state());
    Ok(LayerCommit {
        revision: commit.revision,
        layer: element,
    })
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "transform_committed",
    skip_all,
    fields(origin = "user", entity_count = elements.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn commit_transform(
    expected_revision: Revision,
    elements: Vec<TransformFrame>,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> Result<Option<Revision>, Error> {
    let geometries = desktop.transform_geometries(expected_revision, &elements)?;
    if geometries.is_empty() {
        return Ok(None);
    }
    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        ensure_revision(project.snapshot().revision(), expected_revision)?;
        let commit = project.set_geometries(geometries).await?;
        project.record_commit(&commit);
        (commit, project.active_page())
    };
    desktop.synchronize(&commit.snapshot, page, &commit).await?;
    canvas_channel.channel.publish(desktop.canvas_state());
    Ok(Some(commit.revision))
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "inpaint_requested",
    skip_all,
    fields(origin = "user", point_count = points.len(), size = f64::from(diameter)),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn commit_inpaint(
    expected_revision: Revision,
    points: Vec<Point>,
    diameter: f32,
    handle: AppHandle<CefRuntime>,
    project: State<'_, CurrentProject>,
) -> Result<Option<JobId>, Error> {
    if !diameter.is_finite() || diameter <= 0.0 || points.is_empty() {
        return Err(anyhow!(
            "an inpaint stroke requires a positive diameter and at least one point"
        )
        .into());
    }
    if points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(anyhow!("inpaint stroke points must be finite").into());
    }
    let (page, width, height) = {
        let project = project.project.lock().await;
        let project = project.as_ref().context("no project is open")?;
        let snapshot = project.snapshot();
        ensure_revision(snapshot.revision(), expected_revision)?;
        let page = project
            .active_page()
            .context("the project has no active page")?;
        let page_value = snapshot.page(page)?.page()?;
        (
            page,
            page_value.width.round() as u32,
            page_value.height.round() as u32,
        )
    };
    let stroke = MaskStroke {
        points,
        diameter,
        erase: false,
    };
    let (png, bounds, _) =
        tokio_rayon::spawn(move || encode_mask(width, height, None, &[stroke], None)).await?;
    Ok(Some(
        run_manual_inpaint(&handle, page, png, bounds, None).await?,
    ))
}

/// Inpaints exactly the pixels of a user-supplied page-sized mask, inside `bounds`. With
/// `restore`, the cleanup layer is first reset to the original pixels inside that box.
async fn run_manual_inpaint(
    handle: &AppHandle<CefRuntime>,
    page: EntityId,
    png: Vec<u8>,
    bounds: koharu_pipeline::Bounds,
    restore: Option<koharu_pipeline::Bounds>,
) -> Result<JobId, Error> {
    *handle.state::<Processing>().inpainting_mask.lock() = Some(koharu_pipeline::InpaintingMask {
        page,
        png: Arc::from(png),
        restore,
    });
    processing::process(
        handle.clone(),
        koharu_pipeline::Scope::Region { page, bounds },
        koharu_pipeline::Operation::Only {
            stage: koharu_pipeline::Stage::Inpainting,
        },
        handle.state::<CurrentProject>(),
        handle.state::<Processing>(),
        handle.state::<JobChannel>(),
    )
    .await
}

/// The region's stored paint mask as a page-sized PNG; empty bytes when it was never painted.
#[tauri::command]
#[specta::specta]
pub(crate) async fn get_painted_mask(
    region: EntityId,
    project: State<'_, CurrentProject>,
) -> Result<CanvasBytes, Error> {
    let snapshot = {
        let project = project.project.lock().await;
        project.as_ref().context("no project is open")?.snapshot()
    };
    let role = koharu_scene::AssetRole::new("painted-mask")?;
    let Some(asset) = snapshot.asset(region, &role)? else {
        return Ok(CanvasBytes(Vec::new()));
    };
    Ok(CanvasBytes(snapshot.read_blob(asset.blob).await?.to_vec()))
}

/// Paint mask: stores the displayed mask as the region's own mask (one undo step) and repaints
/// it. The displayed mask is the stored one with `strokes` applied in order, or a blank one
/// with `replace` (the user cleared first). A mask left with no masked pixel is removed rather
/// than stored as an all-black mask that would inpaint nothing. The region's padded box is
/// reset to the original pixels before inpainting so a smaller mask un-cleans what the old one
/// covered. Strokes are clipped here, to that box, whatever the client sent.
#[tracing::instrument(
    target = "koharu_metrics",
    name = "region_mask_painted",
    skip_all,
    fields(origin = "user", stroke_count = strokes.len()),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn paint_region_mask(
    region: EntityId,
    strokes: Vec<MaskStroke>,
    replace: bool,
    handle: AppHandle<CefRuntime>,
    project: State<'_, CurrentProject>,
) -> Result<Option<JobId>, Error> {
    validate_strokes(&strokes)?;
    let (page, width, height, clip, stored) = {
        let project = project.project.lock().await;
        let project = project.as_ref().context("no project is open")?;
        let snapshot = project.snapshot();
        let page = project
            .active_page()
            .context("the project has no active page")?;
        let size = snapshot.page(page)?.page()?;
        let (width, height) = (size.width.round() as u32, size.height.round() as u32);
        let corners = snapshot
            .component::<Geometry>(region)?
            .context("the region has no geometry")?
            .points
            .iter()
            .map(|p| (p.x, p.y))
            .collect::<Vec<_>>();
        let stored = match snapshot.asset(region, &koharu_scene::AssetRole::new("painted-mask")?)? {
            Some(asset) if !replace => Some(snapshot.read_blob(asset.blob).await?),
            _ => None,
        };
        (
            page,
            width,
            height,
            koharu_pipeline::padded_box(width, height, &corners)?,
            stored,
        )
    };
    let Ok((png, _, empty)) = tokio_rayon::spawn(move || {
        encode_mask(width, height, stored.as_deref(), &strokes, Some(clip))
    })
    .await
    else {
        return Ok(None); // the strokes changed nothing: nothing to store
    };
    let bounds = clip;
    {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let commit = if empty {
            project.clear_painted_mask(region).await?
        } else {
            project
                .set_painted_mask(region, png.clone(), width, height)
                .await?
        };
        project.record_commit(&commit);
    }
    Ok(Some(
        run_manual_inpaint(&handle, page, png, bounds, Some(clip)).await?,
    ))
}

/// OCR + translate one region, then (optionally) clean it. Entity scope keeps neighbours
/// untouched but cannot include inpainting, so cleaning is a second job that follows with a
/// rectangle mask over the region.
#[tauri::command]
#[specta::specta]
pub(crate) async fn process_region(
    region: EntityId,
    clean: bool,
    handle: AppHandle<CefRuntime>,
) -> Result<(), Error> {
    let (page, width, height, rect) = {
        let current = handle.state::<CurrentProject>();
        let project = current.project.lock().await;
        let project = project.as_ref().context("no project is open")?;
        let snapshot = project.snapshot();
        let page = project
            .active_page()
            .context("the project has no active page")?;
        let size = snapshot.page(page)?.page()?;
        let geometry = snapshot
            .component::<Geometry>(region)?
            .context("the region has no geometry")?;
        (
            page,
            size.width.round() as u32,
            size.height.round() as u32,
            geometry
                .points
                .iter()
                .map(|p| (p.x, p.y))
                .collect::<Vec<_>>(),
        )
    };
    processing::process(
        handle.clone(),
        koharu_pipeline::Scope::Entities(vec![region]),
        koharu_pipeline::Operation::Stages {
            stages: vec![
                koharu_pipeline::Stage::Ocr,
                koharu_pipeline::Stage::Translation,
            ],
        },
        handle.state::<CurrentProject>(),
        handle.state::<Processing>(),
        handle.state::<JobChannel>(),
    )
    .await?;
    if !clean {
        return Ok(());
    }
    // ponytail: polling until the OCR job ends; replace with a completion signal if jobs ever run concurrently.
    while !handle.state::<Processing>().stops.lock().is_empty() {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let (png, bounds) = tokio_rayon::spawn(move || rect_mask(width, height, &rect)).await?;
    *handle.state::<Processing>().inpainting_mask.lock() = Some(koharu_pipeline::InpaintingMask {
        page,
        png: Arc::from(png),
        restore: None,
    });
    processing::process(
        handle.clone(),
        koharu_pipeline::Scope::Region { page, bounds },
        koharu_pipeline::Operation::Only {
            stage: koharu_pipeline::Stage::Inpainting,
        },
        handle.state::<CurrentProject>(),
        handle.state::<Processing>(),
        handle.state::<JobChannel>(),
    )
    .await?;
    Ok(())
}

/// Filled bounding box of `points`, padded by 2 px and clamped to the page.
fn rect_mask(
    width: u32,
    height: u32,
    points: &[(f64, f64)],
) -> anyhow::Result<(Vec<u8>, koharu_pipeline::Bounds)> {
    let bounds = koharu_pipeline::padded_box(width, height, points)?;
    let mut image = GrayImage::new(width, height);
    for y in bounds.y as u32..(bounds.y + bounds.height) as u32 {
        for x in bounds.x as u32..(bounds.x + bounds.width) as u32 {
            image.put_pixel(x, y, image::Luma([255]));
        }
    }
    let mut png = Vec::new();
    PngEncoder::new(&mut png).write_image(
        image.as_raw(),
        width,
        height,
        image::ExtendedColorType::L8,
    )?;
    Ok((png, bounds))
}

#[cfg(test)]
mod region_mask_tests {
    use super::{MaskStroke, Point, encode_mask, rect_mask, validate_strokes};

    #[test]
    fn each_stroke_is_validated_on_its_own() {
        let good = || MaskStroke {
            points: vec![Point { x: 1.0, y: 1.0 }],
            diameter: 2.0,
            erase: false,
        };
        assert!(validate_strokes(&[good()]).is_ok());
        for diameter in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(validate_strokes(&[good(), MaskStroke { diameter, ..good() }]).is_err());
        }
        let mut bad_point = good();
        bad_point.points.push(Point {
            x: f64::NAN,
            y: 0.0,
        });
        assert!(validate_strokes(&[good(), bad_point]).is_err());
        assert!(
            validate_strokes(&[MaskStroke {
                points: vec![],
                ..good()
            }])
            .is_err()
        );
    }

    fn stroke(points: &[(f64, f64)], diameter: f32, erase: bool) -> MaskStroke {
        MaskStroke {
            points: points.iter().map(|&(x, y)| Point { x, y }).collect(),
            diameter,
            erase,
        }
    }

    fn add(points: &[(f64, f64)], diameter: f32) -> MaskStroke {
        stroke(points, diameter, false)
    }

    fn erase(points: &[(f64, f64)], diameter: f32) -> MaskStroke {
        stroke(points, diameter, true)
    }

    fn pixels(png: &[u8]) -> image::GrayImage {
        image::load_from_memory(png).unwrap().to_luma8()
    }

    #[test]
    fn a_stored_mask_keeps_its_pixels_under_new_strokes() {
        let a = encode_mask(20, 20, None, &[add(&[(3.5, 3.5)], 2.0)], None)
            .unwrap()
            .0;
        let b = encode_mask(20, 20, Some(&a), &[add(&[(15.5, 15.5)], 2.0)], None)
            .unwrap()
            .0;
        let mask = pixels(&b);
        assert_eq!(mask.get_pixel(3, 3).0[0], 255);
        assert_eq!(mask.get_pixel(15, 15).0[0], 255);
        assert_eq!(mask.get_pixel(9, 9).0[0], 0);
    }

    #[test]
    fn an_erase_stroke_removes_only_the_pixels_it_covers() {
        let line = add(&[(2.5, 10.5), (37.5, 10.5)], 4.0);
        let (png, ..) = encode_mask(40, 20, None, &[line], None).unwrap();
        let (png, _, empty) =
            encode_mask(40, 20, Some(&png), &[erase(&[(20.5, 10.5)], 6.0)], None).unwrap();
        let mask = pixels(&png);
        assert!(!empty);
        assert_eq!(mask.get_pixel(20, 10).0[0], 0);
        assert_eq!(mask.get_pixel(10, 10).0[0], 255);
        assert_eq!(mask.get_pixel(30, 10).0[0], 255);
        assert_eq!(mask.get_pixel(20, 5).0[0], 0);
    }

    #[test]
    fn strokes_apply_in_order() {
        let strokes = [
            add(&[(5.5, 10.5), (35.5, 10.5)], 4.0),
            erase(&[(20.5, 10.5)], 8.0),
            add(&[(20.5, 10.5)], 2.0),
        ];
        let mask = pixels(&encode_mask(40, 20, None, &strokes, None).unwrap().0);
        assert_eq!(mask.get_pixel(20, 10).0[0], 255); // painted again after the erase
        assert_eq!(mask.get_pixel(17, 10).0[0], 0); // still erased
        assert_eq!(mask.get_pixel(8, 10).0[0], 255);
    }

    #[test]
    fn erasing_an_empty_or_missing_mask_changes_nothing() {
        let erasing = [erase(&[(10.5, 10.5)], 8.0)];
        assert!(encode_mask(20, 20, None, &erasing, None).is_err());
        let blank = encode_mask(20, 20, None, &[add(&[(1.5, 1.5)], 2.0)], None)
            .unwrap()
            .0;
        // The erase misses the only painted pixels, so nothing would be stored.
        assert!(encode_mask(20, 20, Some(&blank), &erasing, None).is_err());
    }

    #[test]
    fn each_stroke_uses_its_own_diameter() {
        let strokes = [add(&[(10.5, 10.5)], 2.0), add(&[(30.5, 10.5)], 10.0)];
        let mask = pixels(&encode_mask(40, 20, None, &strokes, None).unwrap().0);
        assert_eq!(mask.get_pixel(10, 10).0[0], 255);
        assert_eq!(mask.get_pixel(13, 10).0[0], 0);
        assert_eq!(mask.get_pixel(33, 10).0[0], 255);
    }

    #[test]
    fn a_replace_then_erase_works_on_the_new_strokes_only() {
        // `replace` passes no base, so the erase only sees what the same call painted.
        let strokes = [
            add(&[(5.5, 5.5), (25.5, 5.5)], 4.0),
            erase(&[(15.5, 5.5)], 6.0),
        ];
        let mask = pixels(&encode_mask(30, 10, None, &strokes, None).unwrap().0);
        assert_eq!(mask.get_pixel(5, 5).0[0], 255);
        assert_eq!(mask.get_pixel(15, 5).0[0], 0);
    }

    #[test]
    fn erasing_every_masked_pixel_reports_an_empty_mask() {
        let (png, ..) = encode_mask(20, 20, None, &[add(&[(10.5, 10.5)], 4.0)], None).unwrap();
        let (_, _, empty) =
            encode_mask(20, 20, Some(&png), &[erase(&[(10.5, 10.5)], 30.0)], None).unwrap();
        assert!(empty);
    }

    #[test]
    fn strokes_are_clipped_to_the_region_box() {
        let clip = koharu_pipeline::Bounds {
            x: 10.0,
            y: 10.0,
            width: 10.0,
            height: 10.0,
        };
        let line = add(&[(0.0, 15.0), (40.0, 15.0)], 4.0);
        let (png, bounds, _) = encode_mask(50, 50, None, &[line], Some(clip)).unwrap();
        let mask = pixels(&png);
        assert_eq!(mask.get_pixel(15, 15).0[0], 255);
        assert_eq!(mask.get_pixel(5, 15).0[0], 0);
        assert_eq!(mask.get_pixel(30, 15).0[0], 0);
        assert!(bounds.x >= 10.0 && bounds.x + bounds.width <= 20.0);
        let outside = add(&[(40.0, 40.0)], 4.0);
        assert!(encode_mask(50, 50, None, &[outside], Some(clip)).is_err());
    }

    #[test]
    fn separate_strokes_are_not_joined() {
        let strokes = [
            add(&[(5.0, 5.0), (5.0, 6.0)], 2.0),
            add(&[(45.0, 5.0), (45.0, 6.0)], 2.0),
        ];
        let (png, ..) = encode_mask(50, 50, None, &strokes, None).unwrap();
        let mask = pixels(&png);
        assert_eq!(mask.get_pixel(5, 5).0[0], 255);
        assert_eq!(mask.get_pixel(45, 5).0[0], 255);
        assert_eq!(mask.get_pixel(25, 5).0[0], 0);
    }

    #[test]
    fn rect_mask_covers_the_padded_box_and_clamps_to_the_page() {
        let (png, bounds) =
            rect_mask(100, 50, &[(10.0, 10.0), (30.0, 20.0), (30.0, 10.0)]).unwrap();
        assert_eq!(
            (bounds.x, bounds.y, bounds.width, bounds.height),
            (8.0, 8.0, 24.0, 14.0)
        );
        let mask = image::load_from_memory(&png).unwrap().to_luma8();
        assert_eq!(mask.get_pixel(9, 9).0[0], 255);
        assert_eq!(mask.get_pixel(40, 40).0[0], 0);
        let (_, edge) = rect_mask(100, 50, &[(-5.0, -5.0), (500.0, 500.0)]).unwrap();
        assert_eq!(
            (edge.x, edge.y, edge.width, edge.height),
            (0.0, 0.0, 100.0, 50.0)
        );
        assert!(rect_mask(100, 50, &[(200.0, 10.0), (210.0, 20.0)]).is_err());
    }
}

fn ensure_revision(actual: Revision, expected: Revision) -> anyhow::Result<()> {
    if actual != expected {
        bail!("canvas edit expected revision {expected}, but the project is at {actual}");
    }
    Ok(())
}

/// Every stroke needs its own positive finite diameter and finite points.
fn validate_strokes(strokes: &[MaskStroke]) -> anyhow::Result<()> {
    if strokes.iter().all(|stroke| stroke.points.is_empty())
        || strokes.iter().any(|stroke| {
            !stroke.diameter.is_finite()
                || stroke.diameter <= 0.0
                || stroke
                    .points
                    .iter()
                    .any(|point| !point.x.is_finite() || !point.y.is_finite())
        })
    {
        bail!("a mask stroke needs a positive diameter and finite points");
    }
    Ok(())
}

/// Rasterises `strokes` in order onto `base` (a same-sized PNG mask) or a blank mask. Add
/// strokes raise coverage, erase strokes lower it (like canvas `source-over` and
/// `destination-out`). Returns the PNG, the box of pixels the strokes changed and whether no
/// masked pixel remains. Fails when the strokes changed nothing.
fn encode_mask(
    width: u32,
    height: u32,
    base: Option<&[u8]>,
    strokes: &[MaskStroke],
    clip: Option<koharu_pipeline::Bounds>,
) -> anyhow::Result<(Vec<u8>, koharu_pipeline::Bounds, bool)> {
    if width == 0 || height == 0 {
        bail!("page dimensions must be positive");
    }
    // Pixels outside `clip` stay untouched; whole page when absent.
    let clip = clip.map_or([0, 0, width, height], |c| {
        [
            c.x as u32,
            c.y as u32,
            ((c.x + c.width) as u32).min(width),
            ((c.y + c.height) as u32).min(height),
        ]
    });
    let mut image = match base {
        Some(base) => {
            let image = image::load_from_memory(base)?.to_luma8();
            if image.dimensions() != (width, height) {
                bail!("stored mask does not match the page size");
            }
            image
        }
        None => GrayImage::new(width, height),
    };
    let mut dirty = None::<[u32; 4]>;
    // Each stroke is its own polyline: lifting the pen never draws a connecting segment.
    for (stroke, (start, end)) in strokes.iter().flat_map(|stroke| {
        let points = &stroke.points;
        points
            .iter()
            .zip(points.iter().skip(1))
            .chain(points.last().map(|point| (point, point)))
            .map(move |segment| (stroke, segment))
    }) {
        let radius = f64::from(stroke.diameter) * 0.5;
        let left = ((start.x.min(end.x) - radius - 0.5).floor().max(0.0) as u32).max(clip[0]);
        let top = ((start.y.min(end.y) - radius - 0.5).floor().max(0.0) as u32).max(clip[1]);
        let right = ((start.x.max(end.x) + radius + 0.5)
            .ceil()
            .min(f64::from(width)) as u32)
            .min(clip[2]);
        let bottom = ((start.y.max(end.y) + radius + 0.5)
            .ceil()
            .min(f64::from(height)) as u32)
            .min(clip[3]);
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length_squared = dx.mul_add(dx, dy * dy);
        for y in top..bottom {
            for x in left..right {
                let px = f64::from(x) + 0.5;
                let py = f64::from(y) + 0.5;
                let progress = if length_squared <= f64::EPSILON {
                    0.0
                } else {
                    (((px - start.x) * dx + (py - start.y) * dy) / length_squared).clamp(0.0, 1.0)
                };
                let nearest_x = start.x + progress * dx;
                let nearest_y = start.y + progress * dy;
                let distance = (px - nearest_x).hypot(py - nearest_y);
                let coverage = ((radius + 0.5 - distance).clamp(0.0, 1.0) * 255.0).round() as u8;
                if coverage == 0 {
                    continue;
                }
                let pixel = image.get_pixel_mut(x, y);
                let before = pixel.0[0];
                pixel.0[0] = if stroke.erase {
                    before.min(255 - coverage)
                } else {
                    before.max(coverage)
                };
                if pixel.0[0] == before {
                    continue;
                }
                dirty = Some(match dirty {
                    Some([min_x, min_y, max_x, max_y]) => {
                        [min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y)]
                    }
                    None => [x, y, x, y],
                });
            }
        }
    }
    let [left, top, right, bottom] = dirty.context("the strokes change no pixel of the mask")?;
    let empty = image.as_raw().iter().all(|&value| value == 0);
    let mut png = Vec::new();
    PngEncoder::new(&mut png).write_image(
        image.as_raw(),
        width,
        height,
        image::ExtendedColorType::L8,
    )?;
    Ok((
        png,
        koharu_pipeline::Bounds {
            x: f64::from(left),
            y: f64::from(top),
            width: f64::from(right - left + 1),
            height: f64::from(bottom - top + 1),
        },
        empty,
    ))
}
