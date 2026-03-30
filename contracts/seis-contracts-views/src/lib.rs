use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use seis_contracts_core::{DatasetId, SectionAxis};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionView {
    pub dataset_id: DatasetId,
    pub axis: SectionAxis,
    pub index: usize,
    pub traces: usize,
    pub samples: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct PreviewView {
    pub section: SectionView,
    pub processing_label: String,
    pub preview_ready: bool,
}

impl PreviewView {
    pub fn pending(section: SectionView, processing_label: impl Into<String>) -> Self {
        Self {
            section,
            processing_label: processing_label.into(),
            preview_ready: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_preview_marks_state_as_not_ready() {
        let section = SectionView {
            dataset_id: DatasetId("demo".to_string()),
            axis: SectionAxis::Inline,
            index: 10,
            traces: 128,
            samples: 512,
        };
        let preview = PreviewView::pending(section, "denoise");
        assert!(!preview.preview_ready);
    }
}
