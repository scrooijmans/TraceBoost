pub use ophiolite_seismic::{
    CancelProcessingJobRequest, CancelProcessingJobResponse, DatasetSummary,
    DeletePipelinePresetRequest, DeletePipelinePresetResponse, GetProcessingJobRequest,
    GetProcessingJobResponse, IPC_SCHEMA_VERSION, ImportDatasetRequest, ImportDatasetResponse,
    ListPipelinePresetsResponse, OpenDatasetRequest, OpenDatasetResponse, PreviewCommand,
    PreviewProcessingRequest, PreviewProcessingResponse, PreviewResponse, RunProcessingRequest,
    RunProcessingResponse, SavePipelinePresetRequest, SavePipelinePresetResponse, SectionAxis,
    SuggestedImportAction, SurveyPreflightRequest, SurveyPreflightResponse,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DatasetRegistryStatus {
    Linked,
    Imported,
    MissingSource,
    MissingStore,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct DatasetRegistryEntry {
    pub entry_id: String,
    pub display_name: String,
    pub source_path: Option<String>,
    pub preferred_store_path: Option<String>,
    pub imported_store_path: Option<String>,
    pub last_dataset: Option<DatasetSummary>,
    pub status: DatasetRegistryStatus,
    pub last_opened_at_unix_s: Option<u64>,
    pub last_imported_at_unix_s: Option<u64>,
    pub updated_at_unix_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct WorkspaceSession {
    pub active_entry_id: Option<String>,
    pub active_store_path: Option<String>,
    pub active_axis: SectionAxis,
    pub active_index: usize,
    pub selected_preset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct LoadWorkspaceStateResponse {
    pub schema_version: u32,
    pub entries: Vec<DatasetRegistryEntry>,
    pub session: WorkspaceSession,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct UpsertDatasetEntryRequest {
    pub schema_version: u32,
    pub entry_id: Option<String>,
    pub display_name: Option<String>,
    pub source_path: Option<String>,
    pub preferred_store_path: Option<String>,
    pub imported_store_path: Option<String>,
    pub dataset: Option<DatasetSummary>,
    pub make_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct UpsertDatasetEntryResponse {
    pub schema_version: u32,
    pub entry: DatasetRegistryEntry,
    pub session: WorkspaceSession,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct RemoveDatasetEntryRequest {
    pub schema_version: u32,
    pub entry_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct RemoveDatasetEntryResponse {
    pub schema_version: u32,
    pub deleted: bool,
    pub session: WorkspaceSession,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct SetActiveDatasetEntryRequest {
    pub schema_version: u32,
    pub entry_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct SetActiveDatasetEntryResponse {
    pub schema_version: u32,
    pub entry: DatasetRegistryEntry,
    pub session: WorkspaceSession,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct SaveWorkspaceSessionRequest {
    pub schema_version: u32,
    pub active_entry_id: Option<String>,
    pub active_store_path: Option<String>,
    pub active_axis: SectionAxis,
    pub active_index: usize,
    pub selected_preset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct SaveWorkspaceSessionResponse {
    pub schema_version: u32,
    pub session: WorkspaceSession,
}

pub fn encode_preview_command(
    command: &PreviewCommand,
) -> serde_json::Result<String> {
    serde_json::to_string(command)
}

pub fn decode_preview_command(json: &str) -> serde_json::Result<PreviewCommand> {
    serde_json::from_str(json)
}
