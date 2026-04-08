#![recursion_limit = "512"]

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use schemars::schema_for;
use seis_contracts_core::{
    AmplitudeSpectrumCurve, AmplitudeSpectrumRequest, AmplitudeSpectrumResponse, AxisSummaryF32,
    AxisSummaryI32, DatasetId, FrequencyPhaseMode, FrequencyWindowShape, GatherInterpolationMode,
    GatherProcessingOperation, GatherProcessingPipeline, GatherRequest, GatherSelector,
    GeometryDescriptor, GeometryProvenanceSummary, GeometrySummary, InterpretationPoint,
    ProcessingJobArtifact, ProcessingJobArtifactKind, ProcessingJobProgress, ProcessingJobState,
    ProcessingJobStatus, ProcessingPipelineFamily, ProcessingPipelineSpec, SectionAxis,
    SectionRequest, SectionSpectrumSelection, SectionTileRequest, SemblancePanel,
    TraceLocalProcessingCheckpoint, TraceLocalProcessingOperation, TraceLocalProcessingPipeline,
    TraceLocalProcessingPreset, TraceLocalVolumeArithmeticOperator, VelocityFunctionSource,
    VelocityScanRequest, VelocityScanResponse, VolumeDescriptor,
};
use seis_contracts_interop::{
    CancelProcessingJobRequest, CancelProcessingJobResponse, DatasetRegistryEntry,
    DatasetRegistryStatus, DatasetSummary, DeletePipelinePresetRequest,
    DeletePipelinePresetResponse, GetProcessingJobRequest, GetProcessingJobResponse,
    IPC_SCHEMA_VERSION, ImportDatasetRequest, ImportDatasetResponse, ListPipelinePresetsResponse,
    LoadWorkspaceStateResponse, OpenDatasetRequest, OpenDatasetResponse, PreviewCommand,
    PreviewGatherProcessingRequest, PreviewGatherProcessingResponse, PreviewResponse,
    PreviewTraceLocalProcessingRequest, PreviewTraceLocalProcessingResponse,
    RemoveDatasetEntryRequest, RemoveDatasetEntryResponse, RunGatherProcessingRequest,
    RunGatherProcessingResponse, RunTraceLocalProcessingRequest, RunTraceLocalProcessingResponse,
    SavePipelinePresetRequest, SavePipelinePresetResponse, SaveWorkspaceSessionRequest,
    SaveWorkspaceSessionResponse, SetActiveDatasetEntryRequest, SetActiveDatasetEntryResponse,
    SuggestedImportAction, SurveyPreflightRequest, SurveyPreflightResponse,
    UpsertDatasetEntryRequest, UpsertDatasetEntryResponse, WorkspacePipelineEntry,
    WorkspaceSession,
};
use seis_contracts_views::{
    GatherPreviewView, GatherProbe, GatherProbeChanged, GatherView, GatherViewport,
    GatherViewportChanged, PreviewView, SectionColorMap, SectionCoordinate, SectionDisplayDefaults,
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
        "AxisSummaryF32.ts",
        "AxisSummaryI32.ts",
        "GeometryDescriptor.ts",
        "GeometryProvenanceSummary.ts",
        "GeometrySummary.ts",
        "VolumeDescriptor.ts",
        "SectionAxis.ts",
        "SectionRequest.ts",
        "GatherRequest.ts",
        "GatherSelector.ts",
        "SectionTileRequest.ts",
        "FrequencyPhaseMode.ts",
        "FrequencyWindowShape.ts",
        "VelocityFunctionSource.ts",
        "GatherInterpolationMode.ts",
        "SectionSpectrumSelection.ts",
        "AmplitudeSpectrumCurve.ts",
        "AmplitudeSpectrumRequest.ts",
        "AmplitudeSpectrumResponse.ts",
        "ProcessingOperation.ts",
        "ProcessingPipeline.ts",
        "ProcessingPreset.ts",
        "TraceLocalProcessingOperation.ts",
        "TraceLocalProcessingPipeline.ts",
        "TraceLocalProcessingCheckpoint.ts",
        "TraceLocalVolumeArithmeticOperator.ts",
        "GatherProcessingOperation.ts",
        "GatherProcessingPipeline.ts",
        "ProcessingPipelineFamily.ts",
        "ProcessingPipelineSpec.ts",
        "ProcessingJobState.ts",
        "ProcessingJobProgress.ts",
        "ProcessingJobArtifactKind.ts",
        "ProcessingJobArtifact.ts",
        "ProcessingJobStatus.ts",
        "TraceLocalProcessingPreset.ts",
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
        "GatherView.ts",
        "PreviewView.ts",
        "GatherPreviewView.ts",
        "SectionViewport.ts",
        "GatherViewport.ts",
        "SectionProbe.ts",
        "GatherProbe.ts",
        "SectionProbeChanged.ts",
        "GatherProbeChanged.ts",
        "SectionViewportChanged.ts",
        "GatherViewportChanged.ts",
        "SectionInteractionChanged.ts",
        "SemblancePanel.ts",
        "VelocityScanRequest.ts",
        "VelocityScanResponse.ts",
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
        "PreviewTraceLocalProcessingRequest.ts",
        "PreviewTraceLocalProcessingResponse.ts",
        "RunProcessingRequest.ts",
        "RunProcessingResponse.ts",
        "RunTraceLocalProcessingRequest.ts",
        "RunTraceLocalProcessingResponse.ts",
        "PreviewGatherProcessingRequest.ts",
        "PreviewGatherProcessingResponse.ts",
        "RunGatherProcessingRequest.ts",
        "RunGatherProcessingResponse.ts",
        "GetProcessingJobRequest.ts",
        "GetProcessingJobResponse.ts",
        "CancelProcessingJobRequest.ts",
        "CancelProcessingJobResponse.ts",
        "ListPipelinePresetsResponse.ts",
        "SavePipelinePresetRequest.ts",
        "SavePipelinePresetResponse.ts",
        "DeletePipelinePresetRequest.ts",
        "DeletePipelinePresetResponse.ts",
        "DatasetRegistryStatus.ts",
        "WorkspacePipelineEntry.ts",
        "DatasetRegistryEntry.ts",
        "WorkspaceSession.ts",
        "LoadWorkspaceStateResponse.ts",
        "UpsertDatasetEntryRequest.ts",
        "UpsertDatasetEntryResponse.ts",
        "RemoveDatasetEntryRequest.ts",
        "RemoveDatasetEntryResponse.ts",
        "SetActiveDatasetEntryRequest.ts",
        "SetActiveDatasetEntryResponse.ts",
        "SaveWorkspaceSessionRequest.ts",
        "SaveWorkspaceSessionResponse.ts",
        "ipc-schema-version.ts",
        "index.ts",
    ] {
        let path = output_dir.join(file);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }

    AxisSummaryF32::export_all_to(output_dir)?;
    AxisSummaryI32::export_all_to(output_dir)?;
    GeometryDescriptor::export_all_to(output_dir)?;
    GeometryProvenanceSummary::export_all_to(output_dir)?;
    GeometrySummary::export_all_to(output_dir)?;
    VolumeDescriptor::export_all_to(output_dir)?;
    GatherRequest::export_all_to(output_dir)?;
    GatherSelector::export_all_to(output_dir)?;
    SectionTileRequest::export_all_to(output_dir)?;
    FrequencyPhaseMode::export_all_to(output_dir)?;
    FrequencyWindowShape::export_all_to(output_dir)?;
    VelocityFunctionSource::export_all_to(output_dir)?;
    GatherInterpolationMode::export_all_to(output_dir)?;
    SectionSpectrumSelection::export_all_to(output_dir)?;
    AmplitudeSpectrumCurve::export_all_to(output_dir)?;
    AmplitudeSpectrumRequest::export_all_to(output_dir)?;
    AmplitudeSpectrumResponse::export_all_to(output_dir)?;
    TraceLocalProcessingOperation::export_all_to(output_dir)?;
    TraceLocalProcessingPipeline::export_all_to(output_dir)?;
    TraceLocalProcessingCheckpoint::export_all_to(output_dir)?;
    TraceLocalVolumeArithmeticOperator::export_all_to(output_dir)?;
    GatherProcessingOperation::export_all_to(output_dir)?;
    GatherProcessingPipeline::export_all_to(output_dir)?;
    ProcessingPipelineFamily::export_all_to(output_dir)?;
    ProcessingPipelineSpec::export_all_to(output_dir)?;
    ProcessingJobState::export_all_to(output_dir)?;
    ProcessingJobProgress::export_all_to(output_dir)?;
    ProcessingJobArtifactKind::export_all_to(output_dir)?;
    ProcessingJobArtifact::export_all_to(output_dir)?;
    ProcessingJobStatus::export_all_to(output_dir)?;
    TraceLocalProcessingPreset::export_all_to(output_dir)?;
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
    GatherView::export_all_to(output_dir)?;
    PreviewView::export_all_to(output_dir)?;
    GatherPreviewView::export_all_to(output_dir)?;
    SectionViewport::export_all_to(output_dir)?;
    GatherViewport::export_all_to(output_dir)?;
    SectionProbe::export_all_to(output_dir)?;
    GatherProbe::export_all_to(output_dir)?;
    SectionProbeChanged::export_all_to(output_dir)?;
    GatherProbeChanged::export_all_to(output_dir)?;
    SectionViewportChanged::export_all_to(output_dir)?;
    GatherViewportChanged::export_all_to(output_dir)?;
    SectionInteractionChanged::export_all_to(output_dir)?;
    SemblancePanel::export_all_to(output_dir)?;
    VelocityScanRequest::export_all_to(output_dir)?;
    VelocityScanResponse::export_all_to(output_dir)?;
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
    PreviewTraceLocalProcessingRequest::export_all_to(output_dir)?;
    PreviewTraceLocalProcessingResponse::export_all_to(output_dir)?;
    RunTraceLocalProcessingRequest::export_all_to(output_dir)?;
    RunTraceLocalProcessingResponse::export_all_to(output_dir)?;
    PreviewGatherProcessingRequest::export_all_to(output_dir)?;
    PreviewGatherProcessingResponse::export_all_to(output_dir)?;
    RunGatherProcessingRequest::export_all_to(output_dir)?;
    RunGatherProcessingResponse::export_all_to(output_dir)?;
    GetProcessingJobRequest::export_all_to(output_dir)?;
    GetProcessingJobResponse::export_all_to(output_dir)?;
    CancelProcessingJobRequest::export_all_to(output_dir)?;
    CancelProcessingJobResponse::export_all_to(output_dir)?;
    ListPipelinePresetsResponse::export_all_to(output_dir)?;
    SavePipelinePresetRequest::export_all_to(output_dir)?;
    SavePipelinePresetResponse::export_all_to(output_dir)?;
    DeletePipelinePresetRequest::export_all_to(output_dir)?;
    DeletePipelinePresetResponse::export_all_to(output_dir)?;
    DatasetRegistryStatus::export_all_to(output_dir)?;
    WorkspacePipelineEntry::export_all_to(output_dir)?;
    DatasetRegistryEntry::export_all_to(output_dir)?;
    WorkspaceSession::export_all_to(output_dir)?;
    LoadWorkspaceStateResponse::export_all_to(output_dir)?;
    UpsertDatasetEntryRequest::export_all_to(output_dir)?;
    UpsertDatasetEntryResponse::export_all_to(output_dir)?;
    RemoveDatasetEntryRequest::export_all_to(output_dir)?;
    RemoveDatasetEntryResponse::export_all_to(output_dir)?;
    SetActiveDatasetEntryRequest::export_all_to(output_dir)?;
    SetActiveDatasetEntryResponse::export_all_to(output_dir)?;
    SaveWorkspaceSessionRequest::export_all_to(output_dir)?;
    SaveWorkspaceSessionResponse::export_all_to(output_dir)?;

    rewrite_generated_numeric_timestamps(&output_dir.join("TraceLocalProcessingPreset.ts"))?;
    rewrite_generated_numeric_timestamps(&output_dir.join("ProcessingJobStatus.ts"))?;
    rewrite_generated_numeric_timestamps(&output_dir.join("DatasetRegistryEntry.ts"))?;
    rewrite_generated_numeric_timestamps(&output_dir.join("WorkspacePipelineEntry.ts"))?;

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
export type { AxisSummaryF32 } from "./AxisSummaryF32";
export type { AxisSummaryI32 } from "./AxisSummaryI32";
export type { GeometryDescriptor } from "./GeometryDescriptor";
export type { GeometryProvenanceSummary } from "./GeometryProvenanceSummary";
export type { GeometrySummary } from "./GeometrySummary";
export type { VolumeDescriptor } from "./VolumeDescriptor";
export type { SectionAxis } from "./SectionAxis";
export type { SectionRequest } from "./SectionRequest";
export type { GatherRequest } from "./GatherRequest";
export type { GatherSelector } from "./GatherSelector";
export type { SectionTileRequest } from "./SectionTileRequest";
export type { FrequencyPhaseMode } from "./FrequencyPhaseMode";
export type { FrequencyWindowShape } from "./FrequencyWindowShape";
export type { VelocityFunctionSource } from "./VelocityFunctionSource";
export type { GatherInterpolationMode } from "./GatherInterpolationMode";
export type { SectionSpectrumSelection } from "./SectionSpectrumSelection";
export type { AmplitudeSpectrumCurve } from "./AmplitudeSpectrumCurve";
export type { AmplitudeSpectrumRequest } from "./AmplitudeSpectrumRequest";
export type { AmplitudeSpectrumResponse } from "./AmplitudeSpectrumResponse";
export type { TraceLocalProcessingOperation } from "./TraceLocalProcessingOperation";
export type { TraceLocalProcessingPipeline } from "./TraceLocalProcessingPipeline";
export type { TraceLocalProcessingCheckpoint } from "./TraceLocalProcessingCheckpoint";
export type { TraceLocalVolumeArithmeticOperator } from "./TraceLocalVolumeArithmeticOperator";
export type { GatherProcessingOperation } from "./GatherProcessingOperation";
export type { GatherProcessingPipeline } from "./GatherProcessingPipeline";
export type { ProcessingPipelineFamily } from "./ProcessingPipelineFamily";
export type { ProcessingPipelineSpec } from "./ProcessingPipelineSpec";
export type { ProcessingJobState } from "./ProcessingJobState";
export type { ProcessingJobProgress } from "./ProcessingJobProgress";
export type { ProcessingJobArtifactKind } from "./ProcessingJobArtifactKind";
export type { ProcessingJobArtifact } from "./ProcessingJobArtifact";
export type { ProcessingJobStatus } from "./ProcessingJobStatus";
export type { TraceLocalProcessingPreset } from "./TraceLocalProcessingPreset";
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
export type { GatherView } from "./GatherView";
export type { PreviewView } from "./PreviewView";
export type { GatherPreviewView } from "./GatherPreviewView";
export type { SectionViewport } from "./SectionViewport";
export type { GatherViewport } from "./GatherViewport";
export type { SectionProbe } from "./SectionProbe";
export type { GatherProbe } from "./GatherProbe";
export type { SectionProbeChanged } from "./SectionProbeChanged";
export type { GatherProbeChanged } from "./GatherProbeChanged";
export type { SectionViewportChanged } from "./SectionViewportChanged";
export type { GatherViewportChanged } from "./GatherViewportChanged";
export type { SectionInteractionChanged } from "./SectionInteractionChanged";
export type { SemblancePanel } from "./SemblancePanel";
export type { VelocityScanRequest } from "./VelocityScanRequest";
export type { VelocityScanResponse } from "./VelocityScanResponse";
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
export type { PreviewTraceLocalProcessingRequest } from "./PreviewTraceLocalProcessingRequest";
export type { PreviewTraceLocalProcessingResponse } from "./PreviewTraceLocalProcessingResponse";
export type { RunTraceLocalProcessingRequest } from "./RunTraceLocalProcessingRequest";
export type { RunTraceLocalProcessingResponse } from "./RunTraceLocalProcessingResponse";
export type { PreviewGatherProcessingRequest } from "./PreviewGatherProcessingRequest";
export type { PreviewGatherProcessingResponse } from "./PreviewGatherProcessingResponse";
export type { RunGatherProcessingRequest } from "./RunGatherProcessingRequest";
export type { RunGatherProcessingResponse } from "./RunGatherProcessingResponse";
export type { GetProcessingJobRequest } from "./GetProcessingJobRequest";
export type { GetProcessingJobResponse } from "./GetProcessingJobResponse";
export type { CancelProcessingJobRequest } from "./CancelProcessingJobRequest";
export type { CancelProcessingJobResponse } from "./CancelProcessingJobResponse";
export type { ListPipelinePresetsResponse } from "./ListPipelinePresetsResponse";
export type { SavePipelinePresetRequest } from "./SavePipelinePresetRequest";
export type { SavePipelinePresetResponse } from "./SavePipelinePresetResponse";
export type { DeletePipelinePresetRequest } from "./DeletePipelinePresetRequest";
export type { DeletePipelinePresetResponse } from "./DeletePipelinePresetResponse";
export type { DatasetRegistryStatus } from "./DatasetRegistryStatus";
export type { WorkspacePipelineEntry } from "./WorkspacePipelineEntry";
export type { DatasetRegistryEntry } from "./DatasetRegistryEntry";
export type { WorkspaceSession } from "./WorkspaceSession";
export type { LoadWorkspaceStateResponse } from "./LoadWorkspaceStateResponse";
export type { UpsertDatasetEntryRequest } from "./UpsertDatasetEntryRequest";
export type { UpsertDatasetEntryResponse } from "./UpsertDatasetEntryResponse";
export type { RemoveDatasetEntryRequest } from "./RemoveDatasetEntryRequest";
export type { RemoveDatasetEntryResponse } from "./RemoveDatasetEntryResponse";
export type { SetActiveDatasetEntryRequest } from "./SetActiveDatasetEntryRequest";
export type { SetActiveDatasetEntryResponse } from "./SetActiveDatasetEntryResponse";
export type { SaveWorkspaceSessionRequest } from "./SaveWorkspaceSessionRequest";
export type { SaveWorkspaceSessionResponse } from "./SaveWorkspaceSessionResponse";
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
            "AxisSummaryF32": schema_for!(AxisSummaryF32),
            "AxisSummaryI32": schema_for!(AxisSummaryI32),
            "GeometryDescriptor": schema_for!(GeometryDescriptor),
            "GeometryProvenanceSummary": schema_for!(GeometryProvenanceSummary),
            "GeometrySummary": schema_for!(GeometrySummary),
            "VolumeDescriptor": schema_for!(VolumeDescriptor),
            "SectionAxis": schema_for!(SectionAxis),
            "SectionRequest": schema_for!(SectionRequest),
            "GatherRequest": schema_for!(GatherRequest),
            "GatherSelector": schema_for!(GatherSelector),
            "SectionTileRequest": schema_for!(SectionTileRequest),
            "FrequencyPhaseMode": schema_for!(FrequencyPhaseMode),
            "FrequencyWindowShape": schema_for!(FrequencyWindowShape),
            "VelocityFunctionSource": schema_for!(VelocityFunctionSource),
            "GatherInterpolationMode": schema_for!(GatherInterpolationMode),
            "SectionSpectrumSelection": schema_for!(SectionSpectrumSelection),
            "AmplitudeSpectrumCurve": schema_for!(AmplitudeSpectrumCurve),
            "AmplitudeSpectrumRequest": schema_for!(AmplitudeSpectrumRequest),
            "AmplitudeSpectrumResponse": schema_for!(AmplitudeSpectrumResponse),
            "TraceLocalProcessingOperation": schema_for!(TraceLocalProcessingOperation),
            "TraceLocalProcessingPipeline": schema_for!(TraceLocalProcessingPipeline),
            "TraceLocalProcessingCheckpoint": schema_for!(TraceLocalProcessingCheckpoint),
            "TraceLocalVolumeArithmeticOperator": schema_for!(TraceLocalVolumeArithmeticOperator),
            "GatherProcessingOperation": schema_for!(GatherProcessingOperation),
            "GatherProcessingPipeline": schema_for!(GatherProcessingPipeline),
            "ProcessingPipelineFamily": schema_for!(ProcessingPipelineFamily),
            "ProcessingPipelineSpec": schema_for!(ProcessingPipelineSpec),
            "ProcessingJobState": schema_for!(ProcessingJobState),
            "ProcessingJobProgress": schema_for!(ProcessingJobProgress),
            "ProcessingJobArtifactKind": schema_for!(ProcessingJobArtifactKind),
            "ProcessingJobArtifact": schema_for!(ProcessingJobArtifact),
            "ProcessingJobStatus": schema_for!(ProcessingJobStatus),
            "TraceLocalProcessingPreset": schema_for!(TraceLocalProcessingPreset),
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
            "GatherView": schema_for!(GatherView),
            "PreviewView": schema_for!(PreviewView),
            "GatherPreviewView": schema_for!(GatherPreviewView),
            "SectionViewport": schema_for!(SectionViewport),
            "GatherViewport": schema_for!(GatherViewport),
            "SectionProbe": schema_for!(SectionProbe),
            "GatherProbe": schema_for!(GatherProbe),
            "SectionProbeChanged": schema_for!(SectionProbeChanged),
            "GatherProbeChanged": schema_for!(GatherProbeChanged),
            "SectionViewportChanged": schema_for!(SectionViewportChanged),
            "GatherViewportChanged": schema_for!(GatherViewportChanged),
            "SectionInteractionChanged": schema_for!(SectionInteractionChanged),
            "SemblancePanel": schema_for!(SemblancePanel),
            "VelocityScanRequest": schema_for!(VelocityScanRequest),
            "VelocityScanResponse": schema_for!(VelocityScanResponse),
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
            "PreviewTraceLocalProcessingRequest": schema_for!(PreviewTraceLocalProcessingRequest),
            "PreviewTraceLocalProcessingResponse": schema_for!(PreviewTraceLocalProcessingResponse),
            "RunTraceLocalProcessingRequest": schema_for!(RunTraceLocalProcessingRequest),
            "RunTraceLocalProcessingResponse": schema_for!(RunTraceLocalProcessingResponse),
            "PreviewGatherProcessingRequest": schema_for!(PreviewGatherProcessingRequest),
            "PreviewGatherProcessingResponse": schema_for!(PreviewGatherProcessingResponse),
            "RunGatherProcessingRequest": schema_for!(RunGatherProcessingRequest),
            "RunGatherProcessingResponse": schema_for!(RunGatherProcessingResponse),
            "GetProcessingJobRequest": schema_for!(GetProcessingJobRequest),
            "GetProcessingJobResponse": schema_for!(GetProcessingJobResponse),
            "CancelProcessingJobRequest": schema_for!(CancelProcessingJobRequest),
            "CancelProcessingJobResponse": schema_for!(CancelProcessingJobResponse),
            "ListPipelinePresetsResponse": schema_for!(ListPipelinePresetsResponse),
            "SavePipelinePresetRequest": schema_for!(SavePipelinePresetRequest),
            "SavePipelinePresetResponse": schema_for!(SavePipelinePresetResponse),
            "DeletePipelinePresetRequest": schema_for!(DeletePipelinePresetRequest),
            "DeletePipelinePresetResponse": schema_for!(DeletePipelinePresetResponse),
            "DatasetRegistryStatus": schema_for!(DatasetRegistryStatus),
            "WorkspacePipelineEntry": schema_for!(WorkspacePipelineEntry),
            "DatasetRegistryEntry": schema_for!(DatasetRegistryEntry),
            "WorkspaceSession": schema_for!(WorkspaceSession),
            "LoadWorkspaceStateResponse": schema_for!(LoadWorkspaceStateResponse),
            "UpsertDatasetEntryRequest": schema_for!(UpsertDatasetEntryRequest),
            "UpsertDatasetEntryResponse": schema_for!(UpsertDatasetEntryResponse),
            "RemoveDatasetEntryRequest": schema_for!(RemoveDatasetEntryRequest),
            "RemoveDatasetEntryResponse": schema_for!(RemoveDatasetEntryResponse),
            "SetActiveDatasetEntryRequest": schema_for!(SetActiveDatasetEntryRequest),
            "SetActiveDatasetEntryResponse": schema_for!(SetActiveDatasetEntryResponse),
            "SaveWorkspaceSessionRequest": schema_for!(SaveWorkspaceSessionRequest),
            "SaveWorkspaceSessionResponse": schema_for!(SaveWorkspaceSessionResponse),
        }
    });

    fs::write(
        output_dir.join("seis-contracts.schema.json"),
        serde_json::to_string_pretty(&bundle)?,
    )?;

    Ok(())
}
