use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{ProgressSink, Scope, Stage};
use koharu_scene::EntityId;

#[derive(Clone, Debug)]
pub struct InpaintingMask {
    pub page: EntityId,
    pub png: Arc<[u8]>,
    /// When set, the cleanup layer is cleared inside this box before inpainting, so the mask
    /// replaces earlier cleaning there instead of merging into it.
    pub restore: Option<crate::Bounds>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, Type)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum Operation {
    #[default]
    Full,
    Only {
        stage: Stage,
    },
    /// Translate the scoped boxes and explain the result; `note` steers this request only.
    TranslateBox {
        note: Option<String>,
    },
    /// Ask the description model for the scoped pages' Scene Description and appearance
    /// proposals, and nothing else: no translation, no stale marker.
    DescribePage,
    Stages {
        stages: Vec<Stage>,
    },
}

impl Operation {
    pub(crate) fn stages(&self) -> Result<Vec<Stage>> {
        let stages = match self {
            Self::Full => Stage::ALL.to_vec(),
            Self::Only { stage } => vec![*stage],
            Self::TranslateBox { .. } | Self::DescribePage => vec![Stage::Translation],
            Self::Stages { stages } => Stage::ALL
                .into_iter()
                .filter(|stage| stages.contains(stage))
                .collect(),
        };
        if stages.is_empty() {
            bail!("at least one pipeline stage must be selected");
        }
        Ok(stages)
    }
}

#[derive(Clone)]
pub struct Request {
    pub operation: Operation,
    pub scope: Scope,
    pub stop: StopToken,
    pub progress: Option<ProgressSink>,
    pub inpainting_mask: Option<InpaintingMask>,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            operation: Operation::Full,
            scope: Scope::Project,
            stop: StopToken::default(),
            progress: None,
            inpainting_mask: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct StopToken(Arc<AtomicBool>);

impl StopToken {
    pub fn stop(&self) {
        self.0.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn stopped(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_box_runs_only_the_translation_stage() {
        let operation = Operation::TranslateBox { note: None };
        assert_eq!(operation.stages().unwrap(), [Stage::Translation]);
    }

    #[test]
    fn describe_page_runs_only_the_translation_stage() {
        assert_eq!(
            Operation::DescribePage.stages().unwrap(),
            [Stage::Translation]
        );
    }
}
