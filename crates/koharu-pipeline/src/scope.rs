use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};

use anyhow::{Result, bail};
use koharu_scene::{
    EntityId, FitsTo, FlowsIn, Geometry, Inside, Presents, RecognizedFrom, RelationSpec, Snapshot,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::Stage;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Bounding box of `points`, padded by 2 px and clamped to the page. The one rule for the
/// area a user-drawn region cleans when it has no painted mask.
pub fn padded_box(width: u32, height: u32, points: &[(f64, f64)]) -> Result<Bounds> {
    const PAD: f64 = 2.0;
    if width == 0 || height == 0 || points.is_empty() {
        bail!("page and region must be non-empty");
    }
    let fold = |pick: fn(f64, f64) -> f64, i: usize, init: f64| {
        points
            .iter()
            .map(|p| if i == 0 { p.0 } else { p.1 })
            .fold(init, pick)
    };
    let left = (fold(f64::min, 0, f64::MAX) - PAD).floor().max(0.0) as u32;
    let top = (fold(f64::min, 1, f64::MAX) - PAD).floor().max(0.0) as u32;
    let right = ((fold(f64::max, 0, f64::MIN) + PAD).ceil().max(0.0) as u32).min(width);
    let bottom = ((fold(f64::max, 1, f64::MIN) + PAD).ceil().max(0.0) as u32).min(height);
    if left >= right || top >= bottom {
        bail!("the region is outside the page");
    }
    Ok(Bounds {
        x: f64::from(left),
        y: f64::from(top),
        width: f64::from(right - left),
        height: f64::from(bottom - top),
    })
}

impl Bounds {
    fn validate(self) -> Result<()> {
        if self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
        {
            Ok(())
        } else {
            bail!("region bounds must be finite and non-empty")
        }
    }

    pub(crate) fn intersects(self, geometry: &Geometry) -> bool {
        let Some((min_x, min_y, max_x, max_y)) = geometry_extents(geometry) else {
            return false;
        };
        min_x < self.x + self.width
            && max_x > self.x
            && min_y < self.y + self.height
            && max_y > self.y
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize, Type)]
#[serde(tag = "scope", content = "value", rename_all = "snake_case")]
pub enum Scope {
    #[default]
    Project,
    Pages(Vec<EntityId>),
    Region {
        page: EntityId,
        bounds: Bounds,
    },
    Entities(Vec<EntityId>),
}

#[derive(Clone, Debug)]
pub(crate) struct NormalizedScope {
    pages: Arc<[EntityId]>,
    entities: Option<Arc<BTreeSet<EntityId>>>,
    region: Option<(EntityId, Bounds)>,
}

impl NormalizedScope {
    pub(crate) fn new(snapshot: &Snapshot, scope: &Scope, stages: &[Stage]) -> Result<Self> {
        if matches!(scope, Scope::Entities(_))
            && stages
                .iter()
                .any(|stage| matches!(stage, Stage::Detection | Stage::Inpainting))
        {
            bail!("detection and inpainting do not support entity-only scope");
        }

        match scope {
            Scope::Project => Ok(Self {
                pages: snapshot.pages().map(|page| page.id()).collect(),
                entities: None,
                region: None,
            }),
            Scope::Pages(requested) => {
                if requested.is_empty() {
                    bail!("page scope is empty");
                }
                let requested = requested.iter().copied().collect::<BTreeSet<_>>();
                for id in &requested {
                    snapshot.page(*id)?;
                }
                let pages = snapshot
                    .pages()
                    .map(|page| page.id())
                    .filter(|id| requested.contains(id))
                    .collect::<Arc<[_]>>();
                Ok(Self {
                    pages,
                    entities: None,
                    region: None,
                })
            }
            Scope::Region { page, bounds } => {
                snapshot.page(*page)?;
                bounds.validate()?;
                Ok(Self {
                    pages: Arc::from([*page]),
                    entities: None,
                    region: Some((*page, *bounds)),
                })
            }
            Scope::Entities(requested) => {
                if requested.is_empty() {
                    bail!("entity scope is empty");
                }
                let requested = requested.iter().copied().collect::<BTreeSet<_>>();
                let mut page_set = BTreeSet::new();
                for entity in &requested {
                    snapshot.entity(*entity)?;
                    page_set.insert(containing_page(snapshot, *entity)?);
                }
                let mut hierarchy = BTreeSet::new();
                for entity in requested {
                    hierarchy.extend(snapshot.subtree(entity)?.map(|entity| entity.id()));
                }
                let entities = semantic_closure(snapshot, hierarchy);
                let pages = snapshot
                    .pages()
                    .map(|page| page.id())
                    .filter(|id| page_set.contains(id))
                    .collect::<Arc<[_]>>();
                Ok(Self {
                    pages,
                    entities: Some(Arc::new(entities)),
                    region: None,
                })
            }
        }
    }

    pub(crate) fn pages(&self) -> &[EntityId] {
        &self.pages
    }

    pub(crate) fn entities(&self) -> Option<Arc<BTreeSet<EntityId>>> {
        self.entities.clone()
    }

    pub(crate) fn region(&self, page: EntityId) -> Option<Bounds> {
        debug_assert!(self.pages.contains(&page));
        self.region
            .and_then(|(candidate, bounds)| (candidate == page).then_some(bounds))
    }
}

fn semantic_closure(snapshot: &Snapshot, mut entities: BTreeSet<EntityId>) -> BTreeSet<EntityId> {
    let mut pending = entities.iter().copied().collect::<VecDeque<_>>();
    while let Some(entity) = pending.pop_front() {
        for relation in snapshot
            .relations_from(entity, None)
            .chain(snapshot.relations_to(entity, None))
        {
            let value = relation.value();
            if ![
                Presents::KIND,
                RecognizedFrom::KIND,
                FitsTo::KIND,
                FlowsIn::KIND,
                Inside::KIND,
            ]
            .contains(&value.kind.as_str())
            {
                continue;
            }
            for related in [value.source, value.target] {
                if entities.insert(related) {
                    pending.push_back(related);
                }
            }
        }
    }
    entities
}

pub(crate) fn contains_entity(
    snapshot: &Snapshot,
    page: EntityId,
    entities: Option<&BTreeSet<EntityId>>,
    region: Option<Bounds>,
    entity: EntityId,
) -> Result<bool> {
    if containing_page(snapshot, entity)? != page {
        return Ok(false);
    }
    if let Some(entities) = entities {
        return Ok(entities.contains(&entity));
    }
    let Some(bounds) = region else {
        return Ok(true);
    };
    Ok(snapshot
        .component::<Geometry>(entity)?
        .is_some_and(|geometry| bounds.intersects(&geometry)))
}

fn containing_page(snapshot: &Snapshot, mut entity: EntityId) -> Result<EntityId> {
    loop {
        if snapshot.page(entity).is_ok() {
            return Ok(entity);
        }
        entity = snapshot
            .parent(entity)?
            .ok_or_else(|| anyhow::anyhow!("entity is not contained by a page"))?;
    }
}

pub(crate) fn geometry_extents(geometry: &Geometry) -> Option<(f64, f64, f64, f64)> {
    let first = geometry.points.first()?;
    let mut min_x = first.x;
    let mut min_y = first.y;
    let mut max_x = first.x;
    let mut max_y = first.y;
    for point in &geometry.points[1..] {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    Some((min_x, min_y, max_x, max_y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use koharu_scene::{
        At, Geometry, Origin, PageDraft, Session, TextLayout, TextLayoutKind, TextRegion,
    };

    /// Drawing a region scopes OCR and translation to `Scope::Entities([region])`: only that
    /// region's text content and layer may be processed, never a neighbour's.
    #[tokio::test]
    async fn entity_scope_on_a_region_covers_only_its_own_text() {
        let mut session = Session::memory().await.unwrap();
        let mut edit = session.snapshot().edit();
        let page = edit
            .add_page(PageDraft::new("p", 100.0, 100.0), At::End)
            .unwrap();
        let mut texts = Vec::new();
        for x in [0.0, 50.0] {
            let region = edit
                .add_analysis_region::<TextRegion>(
                    page,
                    At::End,
                    &Geometry::rectangle(x, 0.0, 10.0, 10.0),
                    None,
                )
                .unwrap();
            let content = edit.add_text_content(page, At::End).unwrap();
            let layer = edit
                .add_text_layer(
                    page,
                    At::End,
                    content,
                    &TextLayout {
                        origin: Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )
                .unwrap();
            edit.relate::<RecognizedFrom>(content, region).unwrap();
            edit.relate::<FitsTo>(layer, region).unwrap();
            texts.push((region, content, layer));
        }
        let snapshot = session
            .commit(edit.finish().unwrap())
            .await
            .unwrap()
            .snapshot;

        let scope = NormalizedScope::new(
            &snapshot,
            &Scope::Entities(vec![texts[0].0]),
            &[Stage::Ocr, Stage::Translation],
        )
        .unwrap();
        let entities = scope.entities().unwrap();
        for id in [texts[0].0, texts[0].1, texts[0].2] {
            assert!(entities.contains(&id));
        }
        for id in [texts[1].0, texts[1].1, texts[1].2] {
            assert!(!entities.contains(&id));
        }
    }
}
