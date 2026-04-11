pub use ophiolite_project::{
    CoordinateReferenceBindingDto, CoordinateReferenceDto, CoordinateReferenceSourceDto,
    ProjectedPoint2Dto, ProjectedPolygon2Dto, ProjectedVector2Dto, ResolvedSurveyMapHorizonDto,
    ResolvedSurveyMapSourceDto, ResolvedSurveyMapSurveyDto, ResolvedSurveyMapWellDto,
    SurveyIndexAxisDto, SurveyIndexGridDto, SurveyMapGridTransformDto, SurveyMapScalarFieldDto,
    SurveyMapSpatialAvailabilityDto, SurveyMapSpatialDescriptorDto, SurveyMapTrajectoryDto,
    SurveyMapTrajectoryStationDto, SurveyMapTransformDiagnosticsDto, SurveyMapTransformPolicyDto,
    SurveyMapTransformStatusDto,
};
pub use ophiolite_seismic::{
    AmplitudeSpectrumRequest, AmplitudeSpectrumResponse, BuildSurveyTimeDepthTransformRequest,
    CancelProcessingJobRequest, CancelProcessingJobResponse, DatasetSummary,
    DeletePipelinePresetRequest, DeletePipelinePresetResponse, DepthReferenceKind,
    ExportSegyRequest, ExportSegyResponse, GatherProcessingPipeline, GatherRequest, GatherView,
    GetProcessingJobRequest, GetProcessingJobResponse, ImportDatasetRequest, ImportDatasetResponse,
    ImportHorizonXyzRequest, ImportHorizonXyzResponse, ImportPrestackOffsetDatasetRequest,
    ImportPrestackOffsetDatasetResponse, LateralInterpolationMethod, LayeredVelocityInterval,
    LayeredVelocityModel, ListPipelinePresetsResponse, LoadSectionHorizonsRequest,
    LoadSectionHorizonsResponse, OpenDatasetRequest, OpenDatasetResponse, PrestackThirdAxisField,
    PreviewCommand, PreviewGatherProcessingRequest, PreviewGatherProcessingResponse,
    PreviewResponse, PreviewSubvolumeProcessingRequest, PreviewSubvolumeProcessingResponse,
    PreviewTraceLocalProcessingRequest, PreviewTraceLocalProcessingResponse, ProcessingJobArtifact,
    ProcessingJobArtifactKind, RunGatherProcessingRequest, RunGatherProcessingResponse,
    RunSubvolumeProcessingRequest, RunSubvolumeProcessingResponse, RunTraceLocalProcessingRequest,
    RunTraceLocalProcessingResponse, SavePipelinePresetRequest, SavePipelinePresetResponse,
    SectionAxis, SegyGeometryCandidate, SegyGeometryOverride, SegyHeaderField, SegyHeaderValueType,
    StratigraphicBoundaryReference, SubvolumeCropOperation, SubvolumeProcessingPipeline,
    SuggestedImportAction, SurveyPreflightRequest, SurveyPreflightResponse,
    SurveyTimeDepthTransform3D, TimeDepthDomain, TravelTimeReference, VelocityAutopickParameters,
    VelocityFunctionEstimate, VelocityFunctionSource, VelocityIntervalTrend, VelocityPickStrategy,
    VelocityQuantityKind, VelocityScanRequest, VelocityScanResponse, VerticalInterpolationMethod,
    IPC_SCHEMA_VERSION,
};

use schemars::JsonSchema;
use seis_contracts_core::TraceLocalProcessingPipeline;
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
pub struct WorkspacePipelineEntry {
    pub pipeline_id: String,
    pub pipeline: TraceLocalProcessingPipeline,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subvolume_crop: Option<SubvolumeCropOperation>,
    #[ts(type = "number")]
    pub updated_at_unix_s: u64,
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
    #[serde(default)]
    pub session_pipelines: Vec<WorkspacePipelineEntry>,
    #[serde(default)]
    pub active_session_pipeline_id: Option<String>,
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
    pub display_coordinate_reference_id: Option<String>,
    pub active_velocity_model_asset_id: Option<String>,
    pub project_root: Option<String>,
    pub project_survey_asset_id: Option<String>,
    pub project_wellbore_id: Option<String>,
    pub project_section_tolerance_m: Option<f64>,
    pub selected_project_well_time_depth_model_asset_id: Option<String>,
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
    #[serde(default)]
    pub session_pipelines: Option<Vec<WorkspacePipelineEntry>>,
    #[serde(default)]
    pub active_session_pipeline_id: Option<String>,
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
    pub display_coordinate_reference_id: Option<String>,
    pub active_velocity_model_asset_id: Option<String>,
    pub project_root: Option<String>,
    pub project_survey_asset_id: Option<String>,
    pub project_wellbore_id: Option<String>,
    pub project_section_tolerance_m: Option<f64>,
    pub selected_project_well_time_depth_model_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct SaveWorkspaceSessionResponse {
    pub schema_version: u32,
    pub session: WorkspaceSession,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct LoadVelocityModelsRequest {
    pub schema_version: u32,
    pub store_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct LoadVelocityModelsResponse {
    pub schema_version: u32,
    pub models: Vec<SurveyTimeDepthTransform3D>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct SetDatasetNativeCoordinateReferenceRequest {
    pub schema_version: u32,
    pub store_path: String,
    pub coordinate_reference_id: Option<String>,
    pub coordinate_reference_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct SetDatasetNativeCoordinateReferenceResponse {
    pub schema_version: u32,
    pub dataset: DatasetSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct ResolveSurveyMapRequest {
    pub schema_version: u32,
    pub store_path: String,
    pub display_coordinate_reference_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(export)]
pub struct ResolveSurveyMapResponse {
    pub schema_version: u32,
    pub survey_map: ResolvedSurveyMapSourceDto,
}

pub fn encode_preview_command(command: &PreviewCommand) -> serde_json::Result<String> {
    serde_json::to_string(command)
}

pub fn decode_preview_command(json: &str) -> serde_json::Result<PreviewCommand> {
    serde_json::from_str(json)
}

pub mod datasets {
    pub use super::{
        DatasetRegistryEntry, DatasetRegistryStatus, LoadWorkspaceStateResponse,
        RemoveDatasetEntryRequest, RemoveDatasetEntryResponse, SetActiveDatasetEntryRequest,
        SetActiveDatasetEntryResponse, UpsertDatasetEntryRequest, UpsertDatasetEntryResponse,
    };
    pub use ophiolite_seismic::contracts::operations::{
        DatasetSummary, OpenDatasetRequest, OpenDatasetResponse,
    };
}

pub mod import_ops {
    pub use ophiolite_seismic::contracts::operations::{
        ExportSegyRequest, ExportSegyResponse, ImportDatasetRequest, ImportDatasetResponse,
        ImportHorizonXyzRequest, ImportHorizonXyzResponse, ImportPrestackOffsetDatasetRequest,
        ImportPrestackOffsetDatasetResponse, LoadSectionHorizonsRequest,
        LoadSectionHorizonsResponse, PrestackThirdAxisField, SegyGeometryCandidate,
        SegyGeometryOverride, SegyHeaderField, SegyHeaderValueType, SuggestedImportAction,
        SurveyPreflightRequest, SurveyPreflightResponse,
    };
}

pub mod processing_ops {
    pub use ophiolite_seismic::contracts::operations::{
        AmplitudeSpectrumRequest, AmplitudeSpectrumResponse, CancelProcessingJobRequest,
        CancelProcessingJobResponse, DeletePipelinePresetRequest, DeletePipelinePresetResponse,
        GetProcessingJobRequest, GetProcessingJobResponse, ListPipelinePresetsResponse,
        PreviewCommand, PreviewGatherProcessingRequest, PreviewGatherProcessingResponse,
        PreviewResponse, PreviewSubvolumeProcessingRequest, PreviewSubvolumeProcessingResponse,
        PreviewTraceLocalProcessingRequest, PreviewTraceLocalProcessingResponse,
        RunGatherProcessingRequest, RunGatherProcessingResponse, RunSubvolumeProcessingRequest,
        RunSubvolumeProcessingResponse, RunTraceLocalProcessingRequest,
        RunTraceLocalProcessingResponse, SavePipelinePresetRequest, SavePipelinePresetResponse,
        VelocityScanRequest, VelocityScanResponse,
    };
}

pub mod workspace {
    pub use super::{
        LoadVelocityModelsRequest, LoadVelocityModelsResponse, SaveWorkspaceSessionRequest,
        SaveWorkspaceSessionResponse, WorkspacePipelineEntry, WorkspaceSession,
    };
}

pub mod resolve {
    pub use super::{
        ResolveSurveyMapRequest, ResolveSurveyMapResponse,
        SetDatasetNativeCoordinateReferenceRequest, SetDatasetNativeCoordinateReferenceResponse,
    };
    pub use ophiolite_project::{
        ResolvedSurveyMapHorizonDto, ResolvedSurveyMapSourceDto, ResolvedSurveyMapSurveyDto,
        ResolvedSurveyMapWellDto, SurveyIndexAxisDto, SurveyIndexGridDto,
        SurveyMapGridTransformDto, SurveyMapScalarFieldDto, SurveyMapSpatialAvailabilityDto,
        SurveyMapSpatialDescriptorDto, SurveyMapTrajectoryDto, SurveyMapTrajectoryStationDto,
        SurveyMapTransformDiagnosticsDto, SurveyMapTransformPolicyDto, SurveyMapTransformStatusDto,
    };
    pub use ophiolite_seismic::contracts::operations::IPC_SCHEMA_VERSION;
}
