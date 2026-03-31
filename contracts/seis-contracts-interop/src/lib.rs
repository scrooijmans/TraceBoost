use schemars::JsonSchema;
use seis_contracts_core::{SectionRequest, VolumeDescriptor};
use seis_contracts_views::PreviewView;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const IPC_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedImportAction {
    DirectDenseIngest,
    RegularizeSparseSurvey,
    ReviewGeometryMapping,
    UnsupportedInV1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct DatasetSummary {
    pub store_path: String,
    pub descriptor: VolumeDescriptor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SurveyPreflightRequest {
    pub schema_version: u32,
    pub input_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SurveyPreflightResponse {
    pub schema_version: u32,
    pub input_path: String,
    pub trace_count: u64,
    pub samples_per_trace: usize,
    pub classification: String,
    pub suggested_action: SuggestedImportAction,
    pub observed_trace_count: usize,
    pub expected_trace_count: usize,
    pub completeness_ratio: f64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ImportDatasetRequest {
    pub schema_version: u32,
    pub input_path: String,
    pub output_store_path: String,
    #[serde(default)]
    pub overwrite_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ImportDatasetResponse {
    pub schema_version: u32,
    pub dataset: DatasetSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct OpenDatasetRequest {
    pub schema_version: u32,
    pub store_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct OpenDatasetResponse {
    pub schema_version: u32,
    pub dataset: DatasetSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct PreviewCommand {
    pub schema_version: u32,
    pub request: SectionRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct PreviewResponse {
    pub schema_version: u32,
    pub preview: PreviewView,
}

pub fn encode_preview_command(command: &PreviewCommand) -> serde_json::Result<String> {
    serde_json::to_string(command)
}

pub fn decode_preview_command(json: &str) -> serde_json::Result<PreviewCommand> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use seis_contracts_core::{DatasetId, SectionAxis, VolumeDescriptor};
    use seis_contracts_views::{
        SectionColorMap, SectionCoordinate, SectionDisplayDefaults, SectionPolarity,
        SectionRenderMode, SectionView,
    };

    #[test]
    fn preview_command_round_trips() {
        let command = PreviewCommand {
            schema_version: IPC_SCHEMA_VERSION,
            request: SectionRequest {
                dataset_id: DatasetId("demo".to_string()),
                axis: SectionAxis::Inline,
                index: 4,
            },
        };

        let encoded = encode_preview_command(&command).expect("command should serialize");
        let decoded = decode_preview_command(&encoded).expect("command should deserialize");
        assert_eq!(decoded, command);
    }

    #[test]
    fn preview_response_carries_view_model() {
        let response = PreviewResponse {
            schema_version: IPC_SCHEMA_VERSION,
            preview: PreviewView::pending(
                SectionView {
                    dataset_id: DatasetId("demo".to_string()),
                    axis: SectionAxis::Xline,
                    coordinate: SectionCoordinate {
                        index: 9,
                        value: 7202.0,
                    },
                    traces: 64,
                    samples: 256,
                    horizontal_axis_f64le: vec![0; 64 * 8],
                    sample_axis_f32le: vec![0; 256 * 4],
                    amplitudes_f32le: vec![0; 64 * 256 * 4],
                    units: None,
                    metadata: None,
                    display_defaults: Some(SectionDisplayDefaults {
                        gain: 1.0,
                        clip_min: None,
                        clip_max: None,
                        render_mode: SectionRenderMode::Heatmap,
                        colormap: SectionColorMap::Grayscale,
                        polarity: SectionPolarity::Normal,
                    }),
                },
                "gain",
            ),
        };

        assert_eq!(response.schema_version, 1);
        assert!(!response.preview.preview_ready);
    }

    #[test]
    fn dataset_summary_carries_store_and_descriptor() {
        let summary = DatasetSummary {
            store_path: "C:/data/demo.zarr".to_string(),
            descriptor: VolumeDescriptor {
                id: DatasetId("demo.zarr".to_string()),
                label: "demo".to_string(),
                shape: [10, 20, 30],
                chunk_shape: [5, 5, 30],
                sample_interval_ms: 2.0,
            },
        };

        assert_eq!(summary.descriptor.label, "demo");
        assert!(summary.store_path.ends_with(".zarr"));
    }
}
