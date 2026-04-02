#![recursion_limit = "256"]

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use schemars::schema_for;
use seis_contracts_core::{
    DatasetId, InterpretationPoint, ProcessingJobProgress, ProcessingJobState,
    ProcessingJobStatus, ProcessingOperation, ProcessingPipeline, ProcessingPreset, SectionAxis,
    SectionRequest, SectionTileRequest, VolumeDescriptor,
};
use seis_contracts_interop::{
    CancelProcessingJobRequest, CancelProcessingJobResponse, DatasetSummary,
    DeletePipelinePresetRequest, DeletePipelinePresetResponse, GetProcessingJobRequest,
    GetProcessingJobResponse, IPC_SCHEMA_VERSION, ImportDatasetRequest, ImportDatasetResponse,
    ListPipelinePresetsResponse, OpenDatasetRequest, OpenDatasetResponse, PreviewCommand,
    PreviewProcessingRequest, PreviewProcessingResponse, PreviewResponse, RunProcessingRequest,
    RunProcessingResponse, SavePipelinePresetRequest, SavePipelinePresetResponse,
    SuggestedImportAction, SurveyPreflightRequest, SurveyPreflightResponse,
};
use seis_contracts_views::{
    PreviewView, SectionColorMap, SectionCoordinate, SectionDisplayDefaults,
    SectionInteractionChanged, SectionMetadata, SectionPolarity, SectionPrimaryMode, SectionProbe,
    SectionProbeChanged, SectionRenderMode, SectionUnits, SectionView, SectionViewport,
    SectionViewportChanged,
};
use ts_rs::TS;

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("scripts/contracts-export should live two levels under repo root")
        .to_path_buf();

    let package_root = repo_root
        .join("contracts")
        .join("ts")
        .join("seis-contracts");
    let generated_dir = package_root.join("src").join("generated");
    let schema_dir = package_root.join("schemas");

    fs::create_dir_all(&generated_dir)?;
    fs::create_dir_all(&schema_dir)?;

    export_ts_types(&generated_dir)?;
    write_generated_index(&generated_dir)?;
    write_schema_bundle(&schema_dir)?;

    Ok(())
}

fn export_ts_types(output_dir: &Path) -> Result<(), Box<dyn Error>> {
    for file in [
        "DatasetId.ts",
        "VolumeDescriptor.ts",
        "SectionAxis.ts",
        "SectionRequest.ts",
        "SectionTileRequest.ts",
        "ProcessingOperation.ts",
        "ProcessingPipeline.ts",
        "ProcessingJobState.ts",
        "ProcessingJobProgress.ts",
        "ProcessingJobStatus.ts",
        "ProcessingPreset.ts",
        "InterpretationPoint.ts",
        "SectionColorMap.ts",
        "SectionRenderMode.ts",
        "SectionPolarity.ts",
        "SectionPrimaryMode.ts",
        "SectionCoordinate.ts",
        "SectionUnits.ts",
        "SectionMetadata.ts",
        "SectionDisplayDefaults.ts",
        "SectionView.ts",
        "PreviewView.ts",
        "SectionViewport.ts",
        "SectionProbe.ts",
        "SectionProbeChanged.ts",
        "SectionViewportChanged.ts",
        "SectionInteractionChanged.ts",
        "SuggestedImportAction.ts",
        "DatasetSummary.ts",
        "SurveyPreflightRequest.ts",
        "SurveyPreflightResponse.ts",
        "ImportDatasetRequest.ts",
        "ImportDatasetResponse.ts",
        "OpenDatasetRequest.ts",
        "OpenDatasetResponse.ts",
        "PreviewCommand.ts",
        "PreviewResponse.ts",
        "PreviewProcessingRequest.ts",
        "PreviewProcessingResponse.ts",
        "RunProcessingRequest.ts",
        "RunProcessingResponse.ts",
        "GetProcessingJobRequest.ts",
        "GetProcessingJobResponse.ts",
        "CancelProcessingJobRequest.ts",
        "CancelProcessingJobResponse.ts",
        "ListPipelinePresetsResponse.ts",
        "SavePipelinePresetRequest.ts",
        "SavePipelinePresetResponse.ts",
        "DeletePipelinePresetRequest.ts",
        "DeletePipelinePresetResponse.ts",
        "ipc-schema-version.ts",
        "index.ts",
    ] {
        let path = output_dir.join(file);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }

    VolumeDescriptor::export_all_to(output_dir)?;
    SectionTileRequest::export_all_to(output_dir)?;
    ProcessingOperation::export_all_to(output_dir)?;
    ProcessingPipeline::export_all_to(output_dir)?;
    ProcessingJobState::export_all_to(output_dir)?;
    ProcessingJobProgress::export_all_to(output_dir)?;
    ProcessingJobStatus::export_all_to(output_dir)?;
    ProcessingPreset::export_all_to(output_dir)?;
    InterpretationPoint::export_all_to(output_dir)?;
    SectionColorMap::export_all_to(output_dir)?;
    SectionRenderMode::export_all_to(output_dir)?;
    SectionPolarity::export_all_to(output_dir)?;
    SectionPrimaryMode::export_all_to(output_dir)?;
    SectionCoordinate::export_all_to(output_dir)?;
    SectionUnits::export_all_to(output_dir)?;
    SectionMetadata::export_all_to(output_dir)?;
    SectionDisplayDefaults::export_all_to(output_dir)?;
    SectionView::export_all_to(output_dir)?;
    PreviewView::export_all_to(output_dir)?;
    SectionViewport::export_all_to(output_dir)?;
    SectionProbe::export_all_to(output_dir)?;
    SectionProbeChanged::export_all_to(output_dir)?;
    SectionViewportChanged::export_all_to(output_dir)?;
    SectionInteractionChanged::export_all_to(output_dir)?;
    SuggestedImportAction::export_all_to(output_dir)?;
    DatasetSummary::export_all_to(output_dir)?;
    SurveyPreflightRequest::export_all_to(output_dir)?;
    SurveyPreflightResponse::export_all_to(output_dir)?;
    ImportDatasetRequest::export_all_to(output_dir)?;
    ImportDatasetResponse::export_all_to(output_dir)?;
    OpenDatasetRequest::export_all_to(output_dir)?;
    OpenDatasetResponse::export_all_to(output_dir)?;
    PreviewCommand::export_all_to(output_dir)?;
    PreviewResponse::export_all_to(output_dir)?;
    PreviewProcessingRequest::export_all_to(output_dir)?;
    PreviewProcessingResponse::export_all_to(output_dir)?;
    RunProcessingRequest::export_all_to(output_dir)?;
    RunProcessingResponse::export_all_to(output_dir)?;
    GetProcessingJobRequest::export_all_to(output_dir)?;
    GetProcessingJobResponse::export_all_to(output_dir)?;
    CancelProcessingJobRequest::export_all_to(output_dir)?;
    CancelProcessingJobResponse::export_all_to(output_dir)?;
    ListPipelinePresetsResponse::export_all_to(output_dir)?;
    SavePipelinePresetRequest::export_all_to(output_dir)?;
    SavePipelinePresetResponse::export_all_to(output_dir)?;
    DeletePipelinePresetRequest::export_all_to(output_dir)?;
    DeletePipelinePresetResponse::export_all_to(output_dir)?;

    rewrite_generated_numeric_timestamps(&output_dir.join("ProcessingPreset.ts"))?;
    rewrite_generated_numeric_timestamps(&output_dir.join("ProcessingJobStatus.ts"))?;

    fs::write(
        output_dir.join("ipc-schema-version.ts"),
        format!(
            "// Generated by `cargo run -p contracts-export`\nexport const IPC_SCHEMA_VERSION = {IPC_SCHEMA_VERSION} as const;\n"
        ),
    )?;

    Ok(())
}

fn rewrite_generated_numeric_timestamps(path: &Path) -> Result<(), Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    let rewritten = source.replace(": bigint", ": number");
    fs::write(path, rewritten)?;
    Ok(())
}

fn write_generated_index(output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let index = r#"// Generated by `cargo run -p contracts-export`
export type { DatasetId } from "./DatasetId";
export type { VolumeDescriptor } from "./VolumeDescriptor";
export type { SectionAxis } from "./SectionAxis";
export type { SectionRequest } from "./SectionRequest";
export type { SectionTileRequest } from "./SectionTileRequest";
export type { ProcessingOperation } from "./ProcessingOperation";
export type { ProcessingPipeline } from "./ProcessingPipeline";
export type { ProcessingJobState } from "./ProcessingJobState";
export type { ProcessingJobProgress } from "./ProcessingJobProgress";
export type { ProcessingJobStatus } from "./ProcessingJobStatus";
export type { ProcessingPreset } from "./ProcessingPreset";
export type { InterpretationPoint } from "./InterpretationPoint";
export type { SectionColorMap } from "./SectionColorMap";
export type { SectionRenderMode } from "./SectionRenderMode";
export type { SectionPolarity } from "./SectionPolarity";
export type { SectionPrimaryMode } from "./SectionPrimaryMode";
export type { SectionCoordinate } from "./SectionCoordinate";
export type { SectionUnits } from "./SectionUnits";
export type { SectionMetadata } from "./SectionMetadata";
export type { SectionDisplayDefaults } from "./SectionDisplayDefaults";
export type { SectionView } from "./SectionView";
export type { PreviewView } from "./PreviewView";
export type { SectionViewport } from "./SectionViewport";
export type { SectionProbe } from "./SectionProbe";
export type { SectionProbeChanged } from "./SectionProbeChanged";
export type { SectionViewportChanged } from "./SectionViewportChanged";
export type { SectionInteractionChanged } from "./SectionInteractionChanged";
export type { SuggestedImportAction } from "./SuggestedImportAction";
export type { DatasetSummary } from "./DatasetSummary";
export type { SurveyPreflightRequest } from "./SurveyPreflightRequest";
export type { SurveyPreflightResponse } from "./SurveyPreflightResponse";
export type { ImportDatasetRequest } from "./ImportDatasetRequest";
export type { ImportDatasetResponse } from "./ImportDatasetResponse";
export type { OpenDatasetRequest } from "./OpenDatasetRequest";
export type { OpenDatasetResponse } from "./OpenDatasetResponse";
export type { PreviewCommand } from "./PreviewCommand";
export type { PreviewResponse } from "./PreviewResponse";
export type { PreviewProcessingRequest } from "./PreviewProcessingRequest";
export type { PreviewProcessingResponse } from "./PreviewProcessingResponse";
export type { RunProcessingRequest } from "./RunProcessingRequest";
export type { RunProcessingResponse } from "./RunProcessingResponse";
export type { GetProcessingJobRequest } from "./GetProcessingJobRequest";
export type { GetProcessingJobResponse } from "./GetProcessingJobResponse";
export type { CancelProcessingJobRequest } from "./CancelProcessingJobRequest";
export type { CancelProcessingJobResponse } from "./CancelProcessingJobResponse";
export type { ListPipelinePresetsResponse } from "./ListPipelinePresetsResponse";
export type { SavePipelinePresetRequest } from "./SavePipelinePresetRequest";
export type { SavePipelinePresetResponse } from "./SavePipelinePresetResponse";
export type { DeletePipelinePresetRequest } from "./DeletePipelinePresetRequest";
export type { DeletePipelinePresetResponse } from "./DeletePipelinePresetResponse";
export { IPC_SCHEMA_VERSION } from "./ipc-schema-version";
"#;

    fs::write(output_dir.join("index.ts"), index)?;
    Ok(())
}

fn write_schema_bundle(output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let bundle = serde_json::json!({
        "ipcSchemaVersion": IPC_SCHEMA_VERSION,
        "types": {
            "DatasetId": schema_for!(DatasetId),
            "VolumeDescriptor": schema_for!(VolumeDescriptor),
            "SectionAxis": schema_for!(SectionAxis),
            "SectionRequest": schema_for!(SectionRequest),
            "SectionTileRequest": schema_for!(SectionTileRequest),
            "ProcessingOperation": schema_for!(ProcessingOperation),
            "ProcessingPipeline": schema_for!(ProcessingPipeline),
            "ProcessingJobState": schema_for!(ProcessingJobState),
            "ProcessingJobProgress": schema_for!(ProcessingJobProgress),
            "ProcessingJobStatus": schema_for!(ProcessingJobStatus),
            "ProcessingPreset": schema_for!(ProcessingPreset),
            "InterpretationPoint": schema_for!(InterpretationPoint),
            "SectionColorMap": schema_for!(SectionColorMap),
            "SectionRenderMode": schema_for!(SectionRenderMode),
            "SectionPolarity": schema_for!(SectionPolarity),
            "SectionPrimaryMode": schema_for!(SectionPrimaryMode),
            "SectionCoordinate": schema_for!(SectionCoordinate),
            "SectionUnits": schema_for!(SectionUnits),
            "SectionMetadata": schema_for!(SectionMetadata),
            "SectionDisplayDefaults": schema_for!(SectionDisplayDefaults),
            "SectionView": schema_for!(SectionView),
            "PreviewView": schema_for!(PreviewView),
            "SectionViewport": schema_for!(SectionViewport),
            "SectionProbe": schema_for!(SectionProbe),
            "SectionProbeChanged": schema_for!(SectionProbeChanged),
            "SectionViewportChanged": schema_for!(SectionViewportChanged),
            "SectionInteractionChanged": schema_for!(SectionInteractionChanged),
            "SuggestedImportAction": schema_for!(SuggestedImportAction),
            "DatasetSummary": schema_for!(DatasetSummary),
            "SurveyPreflightRequest": schema_for!(SurveyPreflightRequest),
            "SurveyPreflightResponse": schema_for!(SurveyPreflightResponse),
            "ImportDatasetRequest": schema_for!(ImportDatasetRequest),
            "ImportDatasetResponse": schema_for!(ImportDatasetResponse),
            "OpenDatasetRequest": schema_for!(OpenDatasetRequest),
            "OpenDatasetResponse": schema_for!(OpenDatasetResponse),
            "PreviewCommand": schema_for!(PreviewCommand),
            "PreviewResponse": schema_for!(PreviewResponse),
            "PreviewProcessingRequest": schema_for!(PreviewProcessingRequest),
            "PreviewProcessingResponse": schema_for!(PreviewProcessingResponse),
            "RunProcessingRequest": schema_for!(RunProcessingRequest),
            "RunProcessingResponse": schema_for!(RunProcessingResponse),
            "GetProcessingJobRequest": schema_for!(GetProcessingJobRequest),
            "GetProcessingJobResponse": schema_for!(GetProcessingJobResponse),
            "CancelProcessingJobRequest": schema_for!(CancelProcessingJobRequest),
            "CancelProcessingJobResponse": schema_for!(CancelProcessingJobResponse),
            "ListPipelinePresetsResponse": schema_for!(ListPipelinePresetsResponse),
            "SavePipelinePresetRequest": schema_for!(SavePipelinePresetRequest),
            "SavePipelinePresetResponse": schema_for!(SavePipelinePresetResponse),
            "DeletePipelinePresetRequest": schema_for!(DeletePipelinePresetRequest),
            "DeletePipelinePresetResponse": schema_for!(DeletePipelinePresetResponse),
        }
    });

    fs::write(
        output_dir.join("seis-contracts.schema.json"),
        serde_json::to_string_pretty(&bundle)?,
    )?;

    Ok(())
}
