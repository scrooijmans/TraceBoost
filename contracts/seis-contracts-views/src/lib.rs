use schemars::JsonSchema;
use seis_contracts_core::{DatasetId, SectionAxis};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SectionColorMap {
    Grayscale,
    RedWhiteBlue,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SectionRenderMode {
    Heatmap,
    Wiggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SectionPolarity {
    Normal,
    Reversed,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SectionPrimaryMode {
    Cursor,
    PanZoom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionCoordinate {
    pub index: usize,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionUnits {
    pub horizontal: Option<String>,
    pub sample: Option<String>,
    pub amplitude: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionMetadata {
    pub store_id: Option<String>,
    pub derived_from: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionDisplayDefaults {
    pub gain: f32,
    pub clip_min: Option<f32>,
    pub clip_max: Option<f32>,
    pub render_mode: SectionRenderMode,
    pub colormap: SectionColorMap,
    pub polarity: SectionPolarity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionView {
    pub dataset_id: DatasetId,
    pub axis: SectionAxis,
    pub coordinate: SectionCoordinate,
    pub traces: usize,
    pub samples: usize,
    pub horizontal_axis_f64le: Vec<u8>,
    pub sample_axis_f32le: Vec<u8>,
    pub amplitudes_f32le: Vec<u8>,
    pub units: Option<SectionUnits>,
    pub metadata: Option<SectionMetadata>,
    pub display_defaults: Option<SectionDisplayDefaults>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionViewport {
    pub trace_start: usize,
    pub trace_end: usize,
    pub sample_start: usize,
    pub sample_end: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionProbe {
    pub trace_index: usize,
    pub trace_coordinate: f64,
    pub sample_index: usize,
    pub sample_value: f32,
    pub amplitude: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionProbeChanged {
    pub chart_id: String,
    pub view_id: String,
    pub probe: Option<SectionProbe>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionViewportChanged {
    pub chart_id: String,
    pub view_id: String,
    pub viewport: SectionViewport,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SectionInteractionChanged {
    pub chart_id: String,
    pub view_id: String,
    pub primary_mode: SectionPrimaryMode,
    pub crosshair_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_preview_marks_state_as_not_ready() {
        let section = SectionView {
            dataset_id: DatasetId("demo".to_string()),
            axis: SectionAxis::Inline,
            coordinate: SectionCoordinate {
                index: 10,
                value: 1042.0,
            },
            traces: 128,
            samples: 512,
            horizontal_axis_f64le: vec![0; 128 * 8],
            sample_axis_f32le: vec![0; 512 * 4],
            amplitudes_f32le: vec![0; 128 * 512 * 4],
            units: Some(SectionUnits {
                horizontal: Some("xline".to_string()),
                sample: Some("ms".to_string()),
                amplitude: Some("amp".to_string()),
            }),
            metadata: None,
            display_defaults: Some(SectionDisplayDefaults {
                gain: 1.0,
                clip_min: None,
                clip_max: None,
                render_mode: SectionRenderMode::Heatmap,
                colormap: SectionColorMap::Grayscale,
                polarity: SectionPolarity::Normal,
            }),
        };
        let preview = PreviewView::pending(section, "denoise");
        assert!(!preview.preview_ready);
    }
}
