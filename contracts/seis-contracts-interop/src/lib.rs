pub use ophiolite_seismic::{
    DatasetSummary, IPC_SCHEMA_VERSION, ImportDatasetRequest, ImportDatasetResponse,
    OpenDatasetRequest, OpenDatasetResponse, PreviewCommand, PreviewResponse,
    SuggestedImportAction, SurveyPreflightRequest, SurveyPreflightResponse,
};

pub fn encode_preview_command(
    command: &PreviewCommand,
) -> serde_json::Result<String> {
    serde_json::to_string(command)
}

pub fn decode_preview_command(json: &str) -> serde_json::Result<PreviewCommand> {
    serde_json::from_str(json)
}
