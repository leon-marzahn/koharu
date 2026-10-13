use std::{io::Cursor, sync::Arc};

use anyhow::Context;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use fast_image_resize::{FilterType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, ImageEncoder, RgbImage, codecs::jpeg::JpegEncoder};

use crate::Language;

const MAX_IMAGE_DIMENSION: u32 = 2048;
const JPEG_QUALITY: u8 = 88;

#[derive(Debug, Clone, PartialEq)]
pub struct TranslationRequest {
    pub segments: Vec<String>,
    pub source_language: Option<Language>,
    pub target_language: Language,
    pub instructions: Option<String>,
    pub context: Vec<TranslationContext>,
    pub characters: Vec<TranslationCharacter>,
    pub terms: Vec<TranslationTerm>,
    /// Sources the model must not propose: rejected ones and pending ones not sent above.
    pub known_sources: Vec<String>,
    pub image: Option<Arc<DynamicImage>>,
    /// One-box request: the model also explains why it translated the line that way.
    pub explain: bool,
    /// One-shot user steer for this request only.
    pub note: Option<String>,
    /// The box's current translation, sent with a note so the model improves on it.
    pub previous_translation: Option<String>,
    /// Page scene description sent to translation (who is present, what happens, the mood).
    pub scene_description: Option<String>,
    /// A description request: no segments, the page image goes in and a description comes out.
    /// It rides the translation request so every provider's existing image path serves it.
    pub describe: bool,
    /// Descriptions of earlier pages, oldest first, sent with a description request only.
    pub previous_descriptions: Vec<PreviousDescription>,
}

/// An earlier page's Scene Description, sent so the next page is described with continuity.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PreviousDescription {
    /// 1-based position in the project's page order.
    pub page: usize,
    pub text: String,
}

impl TranslationRequest {
    /// A scene description request for one page; `with_image` and `with_characters` fill it in.
    #[must_use]
    pub fn describing(target_language: Language) -> Self {
        let mut request = Self::new(Vec::<String>::new(), target_language);
        request.describe = true;
        request
    }

    #[must_use]
    pub fn with_previous_descriptions(
        mut self,
        descriptions: impl IntoIterator<Item = PreviousDescription>,
    ) -> Self {
        self.previous_descriptions = descriptions.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_scene_description(mut self, description: Option<String>) -> Self {
        self.scene_description = description.filter(|text| !text.trim().is_empty());
        self
    }

    #[must_use]
    pub fn new(
        segments: impl IntoIterator<Item = impl Into<String>>,
        target_language: Language,
    ) -> Self {
        Self {
            segments: segments.into_iter().map(Into::into).collect(),
            source_language: None,
            target_language,
            instructions: None,
            context: Vec::new(),
            characters: Vec::new(),
            terms: Vec::new(),
            known_sources: Vec::new(),
            image: None,
            explain: false,
            note: None,
            previous_translation: None,
            scene_description: None,
            describe: false,
            previous_descriptions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_explanation(
        mut self,
        note: Option<String>,
        previous_translation: Option<String>,
    ) -> Self {
        self.explain = true;
        self.note = note.filter(|note| !note.trim().is_empty());
        // The previous attempt only matters when the user steers; without a note it is noise.
        self.previous_translation =
            previous_translation.filter(|text| self.note.is_some() && !text.trim().is_empty());
        self
    }

    #[cfg(test)]
    #[must_use]
    pub fn with_source_language(mut self, language: Language) -> Self {
        self.source_language = Some(language);
        self
    }

    #[must_use]
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    #[must_use]
    pub fn with_image(mut self, image: Arc<DynamicImage>) -> Self {
        self.image = Some(image);
        self
    }

    #[must_use]
    pub fn with_context(mut self, context: impl IntoIterator<Item = TranslationContext>) -> Self {
        self.context = context.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_characters(
        mut self,
        characters: impl IntoIterator<Item = TranslationCharacter>,
    ) -> Self {
        self.characters = characters.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_terms(mut self, terms: impl IntoIterator<Item = TranslationTerm>) -> Self {
        self.terms = terms.into_iter().collect();
        self
    }

    #[must_use]
    pub fn with_known_sources(mut self, sources: impl IntoIterator<Item = String>) -> Self {
        self.known_sources = sources.into_iter().collect();
        self
    }

    pub(crate) fn prepare_image(&mut self) -> anyhow::Result<()> {
        let Some(image) = self.image.as_ref() else {
            return Ok(());
        };
        let (width, height) = (image.width(), image.height());
        let longest = width.max(height);
        if longest <= MAX_IMAGE_DIMENSION {
            return Ok(());
        }
        let resized_width =
            (u64::from(width) * u64::from(MAX_IMAGE_DIMENSION) / u64::from(longest)).max(1) as u32;
        let resized_height =
            (u64::from(height) * u64::from(MAX_IMAGE_DIMENSION) / u64::from(longest)).max(1) as u32;
        let source = image.to_rgb8();
        let mut resized = RgbImage::new(resized_width, resized_height);
        Resizer::new()
            .resize(
                &source,
                &mut resized,
                &ResizeOptions::new()
                    .resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3))
                    .use_alpha(false),
            )
            .context("failed to resize translation image")?;
        self.image = Some(Arc::new(DynamicImage::ImageRgb8(resized)));
        Ok(())
    }

    pub(crate) fn remove_image(&mut self) {
        self.image = None;
    }
}

pub(crate) struct EncodedImage {
    pub(crate) data: String,
}

impl EncodedImage {
    pub(crate) fn data_url(&self) -> String {
        format!("data:image/jpeg;base64,{}", self.data)
    }
}

pub(crate) fn encode_image(image: &DynamicImage) -> anyhow::Result<EncodedImage> {
    let rgb = image.to_rgb8();
    let mut bytes = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .context("failed to encode translation image")?;
    Ok(EncodedImage {
        data: STANDARD.encode(bytes.into_inner()),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TranslationContext {
    pub source: String,
    pub translation: String,
    /// The user wrote or edited this translation, so the model should treat it as authoritative.
    pub user_corrected: bool,
}

/// A project character sheet entry. Sent on every request, unlike `context`, which is per-page.
/// Also the shape of a character the model proposes, so every field tolerates being absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TranslationCharacter {
    pub source_name: String,
    pub target_name: String,
    pub speech_style: String,
    pub appearance: String,
    pub notes: String,
    /// A pending entry the user has not approved yet.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub tentative: bool,
}

/// A term list entry already filtered to the page being translated; also a proposed term.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TranslationTerm {
    pub source: String,
    pub target: String,
    pub note: String,
    /// A pending entry the user has not approved yet.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub tentative: bool,
}

/// A suggested change to an existing character, keyed by `source_name`; `None` = unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TranslationCharacterUpdate {
    pub source_name: String,
    pub target_name: Option<String>,
    pub speech_style: Option<String>,
    pub appearance: Option<String>,
    pub notes: Option<String>,
}

/// A suggested change to an existing term, keyed by `source`; `None` = unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TranslationTermUpdate {
    pub source: String,
    pub target: Option<String>,
    pub note: Option<String>,
}

/// New characters and terms, and updates to existing ones, the model suggests; never applied
/// without review.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct TranslationProposals {
    pub characters: Vec<TranslationCharacter>,
    pub terms: Vec<TranslationTerm>,
    pub character_updates: Vec<TranslationCharacterUpdate>,
    pub term_updates: Vec<TranslationTermUpdate>,
}

/// A finished translation: one text per input segment, plus optional proposals.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Translated {
    pub segments: Vec<String>,
    pub proposals: TranslationProposals,
    /// Why the model chose this wording; only requested for one-box runs.
    pub explanation: Option<String>,
    /// Only for description requests: the page description, absent when the model gave none.
    pub description: Option<String>,
}

impl From<Vec<String>> for Translated {
    fn from(segments: Vec<String>) -> Self {
        Self {
            segments,
            proposals: TranslationProposals::default(),
            explanation: None,
            description: None,
        }
    }
}

impl TranslationContext {
    #[cfg(test)]
    #[must_use]
    pub fn new(source: impl Into<String>, translation: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            translation: translation.into(),
            user_corrected: false,
        }
    }
}
