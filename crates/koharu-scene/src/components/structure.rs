use std::str::FromStr;

use revision::revisioned;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    EntityId, Error, Result,
    component::{Component, ValidationContext},
    id::validate_namespaced,
};

use super::{LanguageTag, Origin};

#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct Project {
    pub source_locale: Option<LanguageTag>,
    pub target_locales: Vec<LanguageTag>,
}

impl Component for Project {
    const KIND: &'static str = "dev.koharu.project";

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        if let Some(locale) = &self.source_locale {
            locale.validate()?;
        }
        for locale in &self.target_locales {
            locale.validate()?;
        }
        let mut targets = self.target_locales.clone();
        targets.sort();
        targets.dedup();
        if targets.len() == self.target_locales.len() {
            Ok(())
        } else {
            Err(Error::invalid("target locales contain duplicates"))
        }
    }
}

/// One entry of the project's character sheet; all fields are free text.
#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct Character {
    pub source_name: String,
    pub target_name: String,
    pub speech_style: String,
    pub appearance: String,
    pub notes: String,
}

/// Per-project character sheet, sent with every translation request.
#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct CharacterSheet {
    pub characters: Vec<Character>,
}

impl Component for CharacterSheet {
    const KIND: &'static str = "dev.koharu.character-sheet";

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        Ok(())
    }
}

/// One term list entry: `source` must be rendered as `target`; `note` explains context.
#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct Term {
    pub source: String,
    pub target: String,
    pub note: String,
}

/// Per-project term list; only entries matching the page are sent with a translation request.
#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct TermList {
    pub terms: Vec<Term>,
}

impl Component for TermList {
    const KIND: &'static str = "dev.koharu.term-list";

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        Ok(())
    }
}

/// A proposed change to an approved character, identified by its `source_name`.
/// Only the `Some` fields change; `None` leaves the current value alone.
#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct CharacterUpdate {
    pub source_name: String,
    pub target_name: Option<String>,
    pub speech_style: Option<String>,
    pub appearance: Option<String>,
    pub notes: Option<String>,
}

/// A proposed change to an approved term, identified by its `source`.
#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct TermUpdate {
    pub source: String,
    pub target: Option<String>,
    pub note: Option<String>,
}

/// Model-proposed character and term entries awaiting review, plus the user's review state.
///
/// Kept apart from `CharacterSheet`/`TermList` so those stay the approved, always-sent data:
/// approving moves an entry across, and rejected sources are remembered here so they are
/// never proposed again.
#[revisioned(revision = 3)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct Proposals {
    pub characters: Vec<Character>,
    pub terms: Vec<Term>,
    /// Pending changes to approved entries; rejecting one just deletes it.
    #[revision(start = 2)]
    pub character_updates: Vec<CharacterUpdate>,
    #[revision(start = 2)]
    pub term_updates: Vec<TermUpdate>,
    pub rejected_characters: Vec<String>,
    pub rejected_terms: Vec<String>,
    /// Full rejected entries, so restoring puts them back in review. Rejects made before
    /// revision 3 only have the source name and restore as unblocked only.
    #[revision(start = 3)]
    #[serde(default)]
    pub rejected_character_entries: Vec<Character>,
    #[revision(start = 3)]
    #[serde(default)]
    pub rejected_term_entries: Vec<Term>,
    /// Send pending entries in prompts, marked tentative.
    pub use_in_prompts: bool,
}

impl Component for Proposals {
    const KIND: &'static str = "dev.koharu.proposals";
    // The translation stage appends proposals; the user approves and rejects them.
    const SHARED_WITH_PIPELINE: bool = true;

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        Ok(())
    }
}

/// Marks a page whose translation predates an edit to the character sheet or term list.
/// Recorded at edit time, removed by the translation stage; nothing reruns automatically.
#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct StalePage {}

impl Component for StalePage {
    const KIND: &'static str = "dev.koharu.stale-page";
    const SHARED_WITH_PIPELINE: bool = true;

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        Ok(())
    }
}

/// Marks a region whose "painted-mask" asset is the user's own page-sized inpainting mask.
/// User-authored: the pipeline never writes it. Moving or resizing the region removes it
/// (with the asset), because the mask was painted over the old box.
#[revisioned(revision = 1)]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Type)]
pub struct PaintedMask {}

impl Component for PaintedMask {
    const KIND: &'static str = "dev.koharu.painted-mask";

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        Ok(())
    }
}

/// The user's own workflow label for a page. Absent means Todo, so only InProgress and
/// Done are stored. User-authored: the pipeline never writes it and nothing resets it.
#[revisioned(revision = 1)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageStatus {
    InProgress,
    Done,
}

impl Component for PageStatus {
    const KIND: &'static str = "dev.koharu.page-status";

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        Ok(())
    }
}

/// Who is on a page, what happens and the mood. Page-level: written by the description
/// stage (generated) or the user (edited); a user-owned description is never replaced by a rerun.
/// Absent on pages not described yet and on projects saved before this existed.
#[revisioned(revision = 1)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct SceneDescription {
    pub text: String,
    pub origin: Origin,
}

impl Component for SceneDescription {
    const KIND: &'static str = "dev.koharu.scene-description";

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        if self.text.len() > 1024 * 1024 || self.text.contains('\0') {
            return Err(Error::invalid(
                "scene description is too large or contains NUL",
            ));
        }
        self.origin.validate()
    }

    fn origin(&self) -> Option<&Origin> {
        Some(&self.origin)
    }

    fn set_origin(&mut self, origin: Origin) -> bool {
        self.origin = origin;
        true
    }
}

#[revisioned(revision = 1)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct Page {
    pub label: String,
    pub width: f64,
    pub height: f64,
}

impl Component for Page {
    const KIND: &'static str = "dev.koharu.page";

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        if self.label.len() <= 4096
            && !self.label.contains('\0')
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
        {
            Ok(())
        } else {
            Err(Error::invalid("page label or dimensions are invalid"))
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageDraft {
    pub label: String,
    pub width: f64,
    pub height: f64,
}

impl PageDraft {
    #[must_use]
    pub fn new(label: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
            label: label.into(),
            width,
            height,
        }
    }
}

impl From<PageDraft> for Page {
    fn from(value: PageDraft) -> Self {
        Self {
            label: value.label,
            width: value.width,
            height: value.height,
        }
    }
}

#[revisioned(revision = 1)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct EntityOrigin {
    pub origin: Origin,
}

impl Component for EntityOrigin {
    const KIND: &'static str = "dev.koharu.entity.origin";

    fn origin(&self) -> Option<&Origin> {
        Some(&self.origin)
    }

    fn set_origin(&mut self, origin: Origin) -> bool {
        self.origin = origin;
        true
    }

    fn validate(&self, _context: &ValidationContext<'_>) -> Result<()> {
        self.origin.validate()
    }
}

#[revisioned(revision = 1)]
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Type)]
#[serde(transparent)]
pub struct RelationKind(String);

impl RelationKind {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate_namespaced(&value, "relation kind")?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<()> {
        validate_namespaced(&self.0, "relation kind")
    }
}

impl FromStr for RelationKind {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

#[revisioned(revision = 1)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
pub struct Relation {
    pub origin: Origin,
    pub kind: RelationKind,
    pub source: EntityId,
    pub target: EntityId,
}

impl Component for Relation {
    const KIND: &'static str = "dev.koharu.relation";

    fn record_refs(&self) -> Vec<EntityId> {
        vec![self.source, self.target]
    }

    fn validate(&self, context: &ValidationContext<'_>) -> Result<()> {
        self.origin.validate()?;
        self.kind.validate()?;
        if context.contains_entity(self.source) && context.contains_entity(self.target) {
            Ok(())
        } else {
            Err(Error::invalid("relation endpoint is missing"))
        }
    }

    fn origin(&self) -> Option<&Origin> {
        Some(&self.origin)
    }

    fn set_origin(&mut self, origin: Origin) -> bool {
        self.origin = origin;
        true
    }
}
