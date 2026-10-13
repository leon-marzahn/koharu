//! Persisted component values grouped by the responsibility that owns them.
//!
//! This module contains data and local validation only. Cross-component
//! document rules belong to `schema`; resolved views and edit intents belong
//! to `document`.

mod analysis;
mod assets;
mod groups;
mod layers;
mod provenance;
mod spatial;
mod structure;
mod text;

pub use analysis::{
    DetectionAnalysis, DetectionLabel, OcrAnalysis, Region, RegionKind, TextDirection,
};
pub(crate) use assets::Assets;
pub use assets::{Asset, AssetInput, AssetMetadata, AssetRole};
pub use groups::{Group, TextGroup};
pub use layers::{
    FIT_SCALE, FontStyle, RasterLayer, RasterLayerKind, TextAlignment, TextLayout, TextLayoutKind,
    Typography, WritingMode,
};
pub use provenance::{Authored, Generation, Origin};
pub use spatial::{Geometry, Point, Visibility};
pub use structure::{
    Character, CharacterSheet, CharacterUpdate, EntityOrigin, Locked, LocksBackfilled, Page,
    PageDraft, PageStatus, PaintedMask, Project, Proposals, Relation, RelationKind,
    SceneDescription, StalePage, Term, TermList, TermUpdate,
};
pub use text::{LanguageTag, SourceText, TextContent, TextRole, Translation};
