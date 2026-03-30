use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use seis_contracts_core::SectionRequest;
use seis_contracts_views::PreviewView;
use ts_rs::TS;

pub const IPC_SCHEMA_VERSION: u32 = 1;

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
    use seis_contracts_core::{DatasetId, SectionAxis};
    use seis_contracts_views::SectionView;

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
                    index: 9,
                    traces: 64,
                    samples: 256,
                },
                "gain",
            ),
        };

        assert_eq!(response.schema_version, 1);
        assert!(!response.preview.preview_ready);
    }
}
