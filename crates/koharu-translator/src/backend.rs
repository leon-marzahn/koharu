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
}

impl TranslationRequest {
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
        }
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

/// New characters and terms the model suggests; never applied without review.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranslationProposals {
    pub characters: Vec<TranslationCharacter>,
    pub terms: Vec<TranslationTerm>,
}

/// A finished translation: one text per input segment, plus optional proposals.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Translated {
    pub segments: Vec<String>,
    pub proposals: TranslationProposals,
}

impl From<Vec<String>> for Translated {
    fn from(segments: Vec<String>) -> Self {
        Self {
            segments,
            proposals: TranslationProposals::default(),
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
