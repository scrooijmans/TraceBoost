mod app_paths;
mod diagnostics;
mod preview_session;
mod processing;
mod processing_cache;
mod workspace;

#[cfg(test)]
mod preview_session_bench;
#[cfg(test)]
mod processing_cache_bench;

use ophiolite::{
    resolve_dataset_summary_survey_map_source, AssetBindingInput, AssetKind, AssetStatus,
    OphioliteProject, ResolveSectionWellOverlaysResponse, SectionWellOverlayRequestDto,
    WellTimeDepthModel1D,
};
use seis_contracts_interop::{
    AmplitudeSpectrumRequest, AmplitudeSpectrumResponse, BuildSurveyTimeDepthTransformRequest,
    CancelProcessingJobRequest, CancelProcessingJobResponse, DeletePipelinePresetRequest,
    DeletePipelinePresetResponse, ExportSegyRequest, ExportSegyResponse, GatherProcessingPipeline,
    GatherRequest, GatherView, GetProcessingJobRequest, GetProcessingJobResponse,
    ImportDatasetRequest, ImportDatasetResponse, ImportHorizonXyzRequest, ImportHorizonXyzResponse,
    ImportPrestackOffsetDatasetRequest, ImportPrestackOffsetDatasetResponse,
    ListPipelinePresetsResponse, LoadSectionHorizonsResponse, LoadVelocityModelsResponse,
    LoadWorkspaceStateResponse, OpenDatasetRequest, OpenDatasetResponse,
    PreviewGatherProcessingRequest, PreviewGatherProcessingResponse,
    PreviewSubvolumeProcessingRequest, PreviewSubvolumeProcessingResponse,
    PreviewTraceLocalProcessingRequest, PreviewTraceLocalProcessingResponse,
    RemoveDatasetEntryRequest, RemoveDatasetEntryResponse, ResolveSurveyMapRequest,
    ResolveSurveyMapResponse, RunGatherProcessingRequest, RunGatherProcessingResponse,
    RunSubvolumeProcessingRequest, RunSubvolumeProcessingResponse, RunTraceLocalProcessingRequest,
    RunTraceLocalProcessingResponse, SavePipelinePresetRequest, SavePipelinePresetResponse,
    SaveWorkspaceSessionRequest, SaveWorkspaceSessionResponse, SetActiveDatasetEntryRequest,
    SetActiveDatasetEntryResponse, SetDatasetNativeCoordinateReferenceRequest,
    SetDatasetNativeCoordinateReferenceResponse, SurveyPreflightRequest, SurveyPreflightResponse,
    UpsertDatasetEntryRequest, UpsertDatasetEntryResponse, VelocityScanRequest,
    VelocityScanResponse, IPC_SCHEMA_VERSION,
};
use seis_runtime::{
    materialize_gather_processing_store_with_progress, materialize_processing_volume_with_progress,
    materialize_subvolume_processing_volume_with_progress, open_store,
    set_any_store_native_coordinate_reference, MaterializeOptions, ProcessingArtifactRole,
    ProcessingJobArtifact, ProcessingJobArtifactKind, ProcessingPipelineSpec, SectionAxis,
    SectionHorizonOverlayView, SectionView, SubvolumeProcessingPipeline, TbvolManifest,
    TimeDepthDomain, TraceLocalProcessingPipeline, VelocityFunctionSource, VelocityQuantityKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::Instant,
};
use tauri::{
    ipc::Response,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Emitter, Manager, State,
};
use traceboost_app::{
    amplitude_spectrum, build_velocity_model_transform, default_export_segy_path,
    default_export_zarr_path, ensure_demo_survey_time_depth_transform, export_dataset_segy,
    export_dataset_zarr, import_dataset, import_horizon_xyz, import_prestack_offset_dataset,
    import_velocity_functions_model, load_depth_converted_section, load_gather,
    load_resolved_section_display, load_section_horizons, load_velocity_models,
    open_dataset_summary, preflight_dataset, preview_gather_processing,
    preview_subvolume_processing, run_velocity_scan, ExportZarrResponse,
};

use crate::app_paths::AppPaths;
use crate::diagnostics::{build_fields, json_value, DiagnosticsState, ExportBundleResponse};
use crate::preview_session::PreviewSessionState;
use crate::processing::{JobRecord, ProcessingState};
use crate::processing_cache::ProcessingCacheState;
use crate::workspace::WorkspaceState;

const FILE_OPEN_VOLUME_MENU_ID: &str = "file.open_volume";
const FILE_OPEN_VOLUME_MENU_EVENT: &str = "menu:file-open-volume";
const VELOCITY_MODEL_MENU_ID: &str = "velocity.velocity_model";
const VELOCITY_MODEL_MENU_EVENT: &str = "menu:velocity-model";
const TRACE_LOCAL_CACHE_FAMILY: &str = "trace_local";
const TBVOL_STORE_FORMAT_VERSION: &str = "tbvol-v1";
const PROCESSING_CACHE_RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");
const PACKED_PREVIEW_MAGIC: &[u8; 8] = b"TBPRV001";
const PACKED_SECTION_MAGIC: &[u8; 8] = b"TBSEC001";
const PACKED_SECTION_DISPLAY_MAGIC: &[u8; 8] = b"TBSDP001";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrontendDiagnosticsEventRequest {
    stage: String,
    level: String,
    message: String,
    fields: Option<Map<String, Value>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWellboreRequest {
    project_root: String,
    wellbore_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetProjectWellTimeDepthModelRequest {
    project_root: String,
    wellbore_id: String,
    asset_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRootRequest {
    project_root: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectAssetRequest {
    project_root: String,
    asset_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportProjectWellTimeDepthModelRequest {
    project_root: String,
    json_path: String,
    binding: AssetBindingInput,
    collection_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportProjectWellTimeDepthAssetRequest {
    project_root: String,
    json_path: String,
    binding: AssetBindingInput,
    collection_name: Option<String>,
    asset_kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileProjectWellTimeDepthAuthoredModelRequest {
    project_root: String,
    asset_id: String,
    output_collection_name: Option<String>,
    set_active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWellTimeDepthModelDescriptor {
    asset_id: String,
    well_id: String,
    wellbore_id: String,
    status: String,
    name: String,
    source_kind: ophiolite::TimeDepthTransformSourceKind,
    depth_reference: ophiolite::DepthReferenceKind,
    travel_time_reference: ophiolite::TravelTimeReference,
    sample_count: usize,
    is_active_project_model: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWellTimeDepthObservationDescriptor {
    asset_id: String,
    asset_kind: String,
    well_id: String,
    wellbore_id: String,
    status: String,
    name: String,
    depth_reference: ophiolite::DepthReferenceKind,
    travel_time_reference: ophiolite::TravelTimeReference,
    sample_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWellTimeDepthAuthoredModelDescriptor {
    asset_id: String,
    well_id: String,
    wellbore_id: String,
    status: String,
    name: String,
    source_binding_count: usize,
    assumption_interval_count: usize,
    sampling_step_m: Option<f64>,
    resolved_trajectory_fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWellTimeDepthInventoryResponse {
    observation_sets: Vec<ProjectWellTimeDepthObservationDescriptor>,
    authored_models: Vec<ProjectWellTimeDepthAuthoredModelDescriptor>,
    compiled_models: Vec<ProjectWellTimeDepthModelDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportProjectWellTimeDepthModelResponse {
    asset_id: String,
    well_id: String,
    wellbore_id: String,
    created_well: bool,
    created_wellbore: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSurveyAssetDescriptor {
    asset_id: String,
    name: String,
    status: String,
    well_id: String,
    well_name: String,
    wellbore_id: String,
    wellbore_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWellboreInventoryItem {
    well_id: String,
    well_name: String,
    wellbore_id: String,
    wellbore_name: String,
    trajectory_asset_count: usize,
    well_time_depth_model_count: usize,
    active_well_time_depth_model_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectWellOverlayInventoryResponse {
    surveys: Vec<ProjectSurveyAssetDescriptor>,
    wellbores: Vec<ProjectWellboreInventoryItem>,
}

fn asset_status_label(status: &AssetStatus) -> &'static str {
    match status {
        AssetStatus::Imported => "imported",
        AssetStatus::Validated => "validated",
        AssetStatus::Bound => "bound",
        AssetStatus::NeedsReview => "needs_review",
        AssetStatus::Rejected => "rejected",
        AssetStatus::Superseded => "superseded",
    }
}

fn project_active_well_time_depth_model_asset_id(
    project: &OphioliteProject,
    wellbore_id: &str,
) -> Result<Option<String>, String> {
    let active_asset_id = project
        .project_well_overlay_inventory()
        .map_err(|error| error.to_string())?
        .wellbores
        .into_iter()
        .find(|wellbore| wellbore.wellbore_id.0 == wellbore_id)
        .and_then(|wellbore| wellbore.active_well_time_depth_model_asset_id)
        .map(|asset_id| asset_id.0);
    Ok(active_asset_id)
}

fn project_well_time_depth_model_descriptors(
    project: &OphioliteProject,
    wellbore_id: &str,
    active_asset_id: Option<&str>,
) -> Result<Vec<ProjectWellTimeDepthModelDescriptor>, String> {
    let assets = project
        .list_assets(
            &ophiolite::WellboreId(wellbore_id.to_string()),
            Some(AssetKind::WellTimeDepthModel),
        )
        .map_err(|error| error.to_string())?;

    assets
        .into_iter()
        .map(|asset| {
            let asset_id = asset.id.0.clone();
            let model = project
                .read_well_time_depth_model(&asset.id)
                .map_err(|error| error.to_string())?;
            Ok(ProjectWellTimeDepthModelDescriptor {
                asset_id: asset_id.clone(),
                well_id: asset.well_id.0,
                wellbore_id: asset.wellbore_id.0,
                status: asset_status_label(&asset.status).to_string(),
                name: model.name,
                source_kind: model.source_kind,
                depth_reference: model.depth_reference,
                travel_time_reference: model.travel_time_reference,
                sample_count: model.samples.len(),
                is_active_project_model: active_asset_id.is_some_and(|active| active == asset_id),
            })
        })
        .collect()
}

fn project_well_time_depth_observation_descriptors(
    project: &OphioliteProject,
    wellbore_id: &str,
) -> Result<Vec<ProjectWellTimeDepthObservationDescriptor>, String> {
    let mut descriptors = Vec::new();

    for asset_kind in [
        AssetKind::CheckshotVspObservationSet,
        AssetKind::ManualTimeDepthPickSet,
    ] {
        let assets = project
            .list_assets(
                &ophiolite::WellboreId(wellbore_id.to_string()),
                Some(asset_kind.clone()),
            )
            .map_err(|error| error.to_string())?;
        for asset in assets {
            let (name, depth_reference, travel_time_reference, sample_count) = match asset_kind {
                AssetKind::CheckshotVspObservationSet => {
                    let source = project
                        .read_checkshot_vsp_observation_set(&asset.id)
                        .map_err(|error| error.to_string())?;
                    (
                        source.name,
                        source.depth_reference,
                        source.travel_time_reference,
                        source.samples.len(),
                    )
                }
                AssetKind::ManualTimeDepthPickSet => {
                    let source = project
                        .read_manual_time_depth_pick_set(&asset.id)
                        .map_err(|error| error.to_string())?;
                    (
                        source.name,
                        source.depth_reference,
                        source.travel_time_reference,
                        source.samples.len(),
                    )
                }
                _ => continue,
            };
            descriptors.push(ProjectWellTimeDepthObservationDescriptor {
                asset_id: asset.id.0,
                asset_kind: match asset_kind {
                    AssetKind::CheckshotVspObservationSet => {
                        "checkshot_vsp_observation_set".to_string()
                    }
                    AssetKind::ManualTimeDepthPickSet => "manual_time_depth_pick_set".to_string(),
                    _ => unreachable!("unexpected observation-set asset kind"),
                },
                well_id: asset.well_id.0,
                wellbore_id: asset.wellbore_id.0,
                status: asset_status_label(&asset.status).to_string(),
                name,
                depth_reference,
                travel_time_reference,
                sample_count,
            });
        }
    }

    Ok(descriptors)
}

fn project_well_time_depth_authored_model_descriptors(
    project: &OphioliteProject,
    wellbore_id: &str,
) -> Result<Vec<ProjectWellTimeDepthAuthoredModelDescriptor>, String> {
    let assets = project
        .list_assets(
            &ophiolite::WellboreId(wellbore_id.to_string()),
            Some(AssetKind::WellTimeDepthAuthoredModel),
        )
        .map_err(|error| error.to_string())?;

    assets
        .into_iter()
        .map(|asset| {
            let model = project
                .read_well_time_depth_authored_model(&asset.id)
                .map_err(|error| error.to_string())?;
            Ok(ProjectWellTimeDepthAuthoredModelDescriptor {
                asset_id: asset.id.0,
                well_id: asset.well_id.0,
                wellbore_id: asset.wellbore_id.0,
                status: asset_status_label(&asset.status).to_string(),
                name: model.name,
                source_binding_count: model.source_bindings.len(),
                assumption_interval_count: model.assumption_intervals.len(),
                sampling_step_m: model.sampling_step_m,
                resolved_trajectory_fingerprint: model.resolved_trajectory_fingerprint,
            })
        })
        .collect()
}

fn well_time_depth_import_response(
    result: ophiolite::ProjectAssetImportResult,
) -> ImportProjectWellTimeDepthModelResponse {
    ImportProjectWellTimeDepthModelResponse {
        asset_id: result.asset.id.0,
        well_id: result.resolution.well_id.0,
        wellbore_id: result.resolution.wellbore_id.0,
        created_well: result.resolution.created_well,
        created_wellbore: result.resolution.created_wellbore,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackedPreviewResponseHeader {
    preview_ready: bool,
    processing_label: String,
    section: PackedSectionHeader,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackedSectionResponseHeader {
    section: PackedSectionHeader,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackedSectionDisplayResponseHeader {
    section: PackedSectionHeader,
    time_depth_diagnostics: Option<ophiolite::SectionTimeDepthDiagnostics>,
    scalar_overlays: Vec<PackedScalarOverlayHeader>,
    horizon_overlays: Vec<SectionHorizonOverlayView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackedSectionHeader {
    dataset_id: String,
    axis: SectionAxis,
    coordinate: ophiolite::SectionCoordinate,
    traces: usize,
    samples: usize,
    horizontal_axis_bytes: usize,
    inline_axis_bytes: Option<usize>,
    xline_axis_bytes: Option<usize>,
    sample_axis_bytes: usize,
    amplitudes_bytes: usize,
    units: Option<ophiolite::SectionUnits>,
    metadata: Option<ophiolite::SectionMetadata>,
    display_defaults: Option<ophiolite::SectionDisplayDefaults>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackedScalarOverlayHeader {
    id: String,
    name: Option<String>,
    width: usize,
    height: usize,
    values_bytes: usize,
    color_map: ophiolite::SectionScalarOverlayColorMap,
    opacity: f32,
    value_range: ophiolite::SectionScalarOverlayValueRange,
    units: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DatasetExportFormatCapability {
    available: bool,
    reason: Option<String>,
    default_output_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DatasetExportCapabilitiesResponse {
    store_path: String,
    segy: DatasetExportFormatCapability,
    zarr: DatasetExportFormatCapability,
}

fn align_up(value: usize, alignment: usize) -> usize {
    if alignment == 0 {
        return value;
    }
    let remainder = value % alignment;
    if remainder == 0 {
        value
    } else {
        value + (alignment - remainder)
    }
}

fn pack_preview_section_response(
    preview_ready: bool,
    processing_label: String,
    section: SectionView,
) -> Result<Response, String> {
    let header = PackedPreviewResponseHeader {
        preview_ready,
        processing_label,
        section: packed_section_header(&section),
    };

    let header_bytes = serde_json::to_vec(&header).map_err(|error| error.to_string())?;
    let header_end = 16 + header_bytes.len();
    let data_offset = align_up(header_end, 8);
    let total_len = data_offset
        + section.horizontal_axis_f64le.len()
        + section
            .inline_axis_f64le
            .as_ref()
            .map(Vec::len)
            .unwrap_or_default()
        + section
            .xline_axis_f64le
            .as_ref()
            .map(Vec::len)
            .unwrap_or_default()
        + section.sample_axis_f32le.len()
        + section.amplitudes_f32le.len();

    let mut bytes = Vec::with_capacity(total_len);
    bytes.extend_from_slice(PACKED_PREVIEW_MAGIC);
    bytes.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(data_offset as u32).to_le_bytes());
    bytes.extend_from_slice(&header_bytes);
    bytes.resize(data_offset, 0);
    bytes.extend_from_slice(&section.horizontal_axis_f64le);
    if let Some(inline_axis) = section.inline_axis_f64le.as_ref() {
        bytes.extend_from_slice(inline_axis);
    }
    if let Some(xline_axis) = section.xline_axis_f64le.as_ref() {
        bytes.extend_from_slice(xline_axis);
    }
    bytes.extend_from_slice(&section.sample_axis_f32le);
    bytes.extend_from_slice(&section.amplitudes_f32le);
    Ok(Response::new(bytes))
}

fn pack_section_response(section: SectionView) -> Result<Response, String> {
    let header = PackedSectionResponseHeader {
        section: packed_section_header(&section),
    };

    let header_bytes = serde_json::to_vec(&header).map_err(|error| error.to_string())?;
    let header_end = 16 + header_bytes.len();
    let data_offset = align_up(header_end, 8);
    let total_len = data_offset
        + section.horizontal_axis_f64le.len()
        + section
            .inline_axis_f64le
            .as_ref()
            .map(Vec::len)
            .unwrap_or_default()
        + section
            .xline_axis_f64le
            .as_ref()
            .map(Vec::len)
            .unwrap_or_default()
        + section.sample_axis_f32le.len()
        + section.amplitudes_f32le.len();

    let mut bytes = Vec::with_capacity(total_len);
    bytes.extend_from_slice(PACKED_SECTION_MAGIC);
    bytes.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(data_offset as u32).to_le_bytes());
    bytes.extend_from_slice(&header_bytes);
    bytes.resize(data_offset, 0);
    bytes.extend_from_slice(&section.horizontal_axis_f64le);
    if let Some(inline_axis) = section.inline_axis_f64le.as_ref() {
        bytes.extend_from_slice(inline_axis);
    }
    if let Some(xline_axis) = section.xline_axis_f64le.as_ref() {
        bytes.extend_from_slice(xline_axis);
    }
    bytes.extend_from_slice(&section.sample_axis_f32le);
    bytes.extend_from_slice(&section.amplitudes_f32le);
    Ok(Response::new(bytes))
}

fn pack_section_display_response(
    display: ophiolite::ResolvedSectionDisplayView,
) -> Result<Response, String> {
    let header = PackedSectionDisplayResponseHeader {
        section: packed_section_header(&display.section),
        time_depth_diagnostics: display.time_depth_diagnostics.clone(),
        scalar_overlays: display
            .scalar_overlays
            .iter()
            .map(|overlay| PackedScalarOverlayHeader {
                id: overlay.id.clone(),
                name: overlay.name.clone(),
                width: overlay.width,
                height: overlay.height,
                values_bytes: overlay.values_f32le.len(),
                color_map: overlay.color_map,
                opacity: overlay.opacity,
                value_range: overlay.value_range.clone(),
                units: overlay.units.clone(),
            })
            .collect(),
        horizon_overlays: display.horizon_overlays.clone(),
    };

    let header_bytes = serde_json::to_vec(&header).map_err(|error| error.to_string())?;
    let header_end = 16 + header_bytes.len();
    let data_offset = align_up(header_end, 8);
    let total_len = data_offset
        + display.section.horizontal_axis_f64le.len()
        + display
            .section
            .inline_axis_f64le
            .as_ref()
            .map(Vec::len)
            .unwrap_or_default()
        + display
            .section
            .xline_axis_f64le
            .as_ref()
            .map(Vec::len)
            .unwrap_or_default()
        + display.section.sample_axis_f32le.len()
        + display.section.amplitudes_f32le.len()
        + display
            .scalar_overlays
            .iter()
            .map(|overlay| overlay.values_f32le.len())
            .sum::<usize>();

    let mut bytes = Vec::with_capacity(total_len);
    bytes.extend_from_slice(PACKED_SECTION_DISPLAY_MAGIC);
    bytes.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(data_offset as u32).to_le_bytes());
    bytes.extend_from_slice(&header_bytes);
    bytes.resize(data_offset, 0);
    bytes.extend_from_slice(&display.section.horizontal_axis_f64le);
    if let Some(inline_axis) = display.section.inline_axis_f64le.as_ref() {
        bytes.extend_from_slice(inline_axis);
    }
    if let Some(xline_axis) = display.section.xline_axis_f64le.as_ref() {
        bytes.extend_from_slice(xline_axis);
    }
    bytes.extend_from_slice(&display.section.sample_axis_f32le);
    bytes.extend_from_slice(&display.section.amplitudes_f32le);
    for overlay in &display.scalar_overlays {
        bytes.extend_from_slice(&overlay.values_f32le);
    }
    Ok(Response::new(bytes))
}

fn packed_section_header(section: &SectionView) -> PackedSectionHeader {
    PackedSectionHeader {
        dataset_id: section.dataset_id.0.clone(),
        axis: section.axis,
        coordinate: section.coordinate.clone(),
        traces: section.traces,
        samples: section.samples,
        horizontal_axis_bytes: section.horizontal_axis_f64le.len(),
        inline_axis_bytes: section.inline_axis_f64le.as_ref().map(Vec::len),
        xline_axis_bytes: section.xline_axis_f64le.as_ref().map(Vec::len),
        sample_axis_bytes: section.sample_axis_f32le.len(),
        amplitudes_bytes: section.amplitudes_f32le.len(),
        units: section.units.clone(),
        metadata: section.metadata.clone(),
        display_defaults: section.display_defaults.clone(),
    }
}

fn sanitized_stem(value: &str, fallback: &str) -> String {
    let sanitized: String = value
        .trim()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let collapsed = sanitized
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        fallback.to_string()
    } else {
        collapsed
    }
}

fn section_axis_values(handle: &seis_runtime::StoreHandle, axis: SectionAxis) -> &[f64] {
    match axis {
        SectionAxis::Inline => &handle.manifest.volume.axes.ilines,
        SectionAxis::Xline => &handle.manifest.volume.axes.xlines,
    }
}

fn section_axis_length(handle: &seis_runtime::StoreHandle, axis: SectionAxis) -> usize {
    section_axis_values(handle, axis).len()
}

fn section_axis_range(handle: &seis_runtime::StoreHandle, axis: SectionAxis) -> Option<(f64, f64)> {
    let values = section_axis_values(handle, axis);
    Some((*values.first()?, *values.last()?))
}

fn sample_axis_range_ms(handle: &seis_runtime::StoreHandle) -> Option<(f32, f32)> {
    let values = &handle.manifest.volume.axes.sample_axis_ms;
    Some((*values.first()?, *values.last()?))
}

fn section_coordinate_value(
    handle: &seis_runtime::StoreHandle,
    axis: SectionAxis,
    index: usize,
) -> Option<f64> {
    section_axis_values(handle, axis).get(index).copied()
}

fn section_coordinate_within_crop(
    pipeline: &SubvolumeProcessingPipeline,
    axis: SectionAxis,
    coordinate_value: f64,
) -> bool {
    match axis {
        SectionAxis::Inline => {
            coordinate_value >= f64::from(pipeline.crop.inline_min)
                && coordinate_value <= f64::from(pipeline.crop.inline_max)
        }
        SectionAxis::Xline => {
            coordinate_value >= f64::from(pipeline.crop.xline_min)
                && coordinate_value <= f64::from(pipeline.crop.xline_max)
        }
    }
}

fn append_section_debug_fields(
    fields: &mut Vec<(&'static str, Value)>,
    handle: &seis_runtime::StoreHandle,
    axis: SectionAxis,
    index: usize,
    axis_length_key: &'static str,
    axis_range_key: &'static str,
    coordinate_key: &'static str,
) {
    fields.push((
        axis_length_key,
        json_value(section_axis_length(handle, axis)),
    ));
    if let Some((axis_min, axis_max)) = section_axis_range(handle, axis) {
        fields.push((axis_range_key, json_value([axis_min, axis_max])));
    }
    if let Some(coordinate_value) = section_coordinate_value(handle, axis, index) {
        fields.push((coordinate_key, json_value(coordinate_value)));
    }
}

fn append_subvolume_preview_debug_fields(
    fields: &mut Vec<(&'static str, Value)>,
    handle: &seis_runtime::StoreHandle,
    request: &PreviewSubvolumeProcessingRequest,
) {
    fields.push(("executionOrder", json_value("trace_local_then_crop")));
    fields.push(("sourceShape", json_value(handle.manifest.volume.shape)));
    if let Some((inline_min, inline_max)) = section_axis_range(handle, SectionAxis::Inline) {
        fields.push(("sourceInlineRange", json_value([inline_min, inline_max])));
    }
    if let Some((xline_min, xline_max)) = section_axis_range(handle, SectionAxis::Xline) {
        fields.push(("sourceXlineRange", json_value([xline_min, xline_max])));
    }
    if let Some((z_min, z_max)) = sample_axis_range_ms(handle) {
        fields.push(("sourceZRangeMs", json_value([z_min, z_max])));
    }
    append_section_debug_fields(
        fields,
        handle,
        request.section.axis,
        request.section.index,
        "sourceSectionAxisLength",
        "sourceSectionAxisRange",
        "requestedSectionCoordinateValue",
    );
    if let Some(coordinate_value) =
        section_coordinate_value(handle, request.section.axis, request.section.index)
    {
        fields.push((
            "sectionInsideCropWindow",
            json_value(section_coordinate_within_crop(
                &request.pipeline,
                request.section.axis,
                coordinate_value,
            )),
        ));
    }
}

fn pipeline_output_slug(pipeline: &seis_runtime::TraceLocalProcessingPipeline) -> String {
    if let Some(name) = pipeline
        .name
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return sanitized_stem(name, "pipeline");
    }

    let mut parts = Vec::with_capacity(pipeline.operation_count());
    for operation in pipeline.operations() {
        let part = match operation {
            seis_runtime::ProcessingOperation::AmplitudeScalar { factor } => {
                format!("amplitude-scalar-{}", format_factor(*factor))
            }
            seis_runtime::ProcessingOperation::TraceRmsNormalize => {
                "trace-rms-normalize".to_string()
            }
            seis_runtime::ProcessingOperation::AgcRms { window_ms } => {
                format!("agc-rms-{}", format_factor(*window_ms))
            }
            seis_runtime::ProcessingOperation::PhaseRotation { angle_degrees } => {
                format!("phase-rotation-{}", format_factor(*angle_degrees))
            }
            seis_runtime::ProcessingOperation::LowpassFilter { f3_hz, f4_hz, .. } => format!(
                "lowpass-{}-{}",
                format_factor(*f3_hz),
                format_factor(*f4_hz)
            ),
            seis_runtime::ProcessingOperation::HighpassFilter { f1_hz, f2_hz, .. } => format!(
                "highpass-{}-{}",
                format_factor(*f1_hz),
                format_factor(*f2_hz)
            ),
            seis_runtime::ProcessingOperation::BandpassFilter {
                f1_hz,
                f2_hz,
                f3_hz,
                f4_hz,
                ..
            } => format!(
                "bandpass-{}-{}-{}-{}",
                format_factor(*f1_hz),
                format_factor(*f2_hz),
                format_factor(*f3_hz),
                format_factor(*f4_hz)
            ),
            seis_runtime::ProcessingOperation::VolumeArithmetic {
                operator,
                secondary_store_path,
            } => format!(
                "volume-{}-{}",
                volume_arithmetic_operator_slug(*operator),
                sanitized_stem(
                    Path::new(secondary_store_path)
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or("volume"),
                    "volume",
                )
            ),
        };
        parts.push(part);
    }

    if parts.is_empty() {
        "pipeline".to_string()
    } else {
        sanitized_stem(&parts.join("-"), "pipeline")
    }
}

fn gather_pipeline_output_slug(pipeline: &seis_runtime::GatherProcessingPipeline) -> String {
    if let Some(name) = pipeline
        .name
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return sanitized_stem(name, "gather-pipeline");
    }

    let mut parts = Vec::new();
    if let Some(trace_local) = &pipeline.trace_local_pipeline {
        parts.push(pipeline_output_slug(trace_local));
    }
    for operation in &pipeline.operations {
        let part = match operation {
            seis_runtime::GatherProcessingOperation::NmoCorrection {
                velocity_model,
                interpolation,
            } => format!(
                "nmo-{}-{}",
                velocity_model_output_slug(velocity_model),
                gather_interpolation_output_slug(*interpolation)
            ),
            seis_runtime::GatherProcessingOperation::StretchMute {
                velocity_model,
                max_stretch_ratio,
            } => format!(
                "stretch-mute-{}-{}",
                velocity_model_output_slug(velocity_model),
                format_factor(*max_stretch_ratio)
            ),
            seis_runtime::GatherProcessingOperation::OffsetMute {
                min_offset,
                max_offset,
            } => format!(
                "offset-mute-{}-{}",
                optional_factor_output_slug(*min_offset),
                optional_factor_output_slug(*max_offset)
            ),
        };
        parts.push(part);
    }

    if parts.is_empty() {
        "gather-pipeline".to_string()
    } else {
        sanitized_stem(&parts.join("-"), "gather-pipeline")
    }
}

fn subvolume_pipeline_output_slug(pipeline: &SubvolumeProcessingPipeline) -> String {
    if let Some(name) = pipeline
        .name
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return sanitized_stem(name, "crop-subvolume");
    }

    let mut parts = Vec::new();
    if let Some(trace_local_pipeline) = pipeline.trace_local_pipeline.as_ref() {
        parts.push(pipeline_output_slug(trace_local_pipeline));
    }
    parts.push(format!(
        "crop-il-{}-{}-xl-{}-{}-z-{}-{}",
        pipeline.crop.inline_min,
        pipeline.crop.inline_max,
        pipeline.crop.xline_min,
        pipeline.crop.xline_max,
        format_factor(pipeline.crop.z_min_ms),
        format_factor(pipeline.crop.z_max_ms)
    ));
    sanitized_stem(&parts.join("-"), "crop-subvolume")
}

fn volume_arithmetic_operator_slug(
    operator: seis_runtime::TraceLocalVolumeArithmeticOperator,
) -> &'static str {
    match operator {
        seis_runtime::TraceLocalVolumeArithmeticOperator::Add => "add",
        seis_runtime::TraceLocalVolumeArithmeticOperator::Subtract => "subtract",
        seis_runtime::TraceLocalVolumeArithmeticOperator::Multiply => "multiply",
        seis_runtime::TraceLocalVolumeArithmeticOperator::Divide => "divide",
    }
}

fn gather_interpolation_output_slug(mode: seis_runtime::GatherInterpolationMode) -> &'static str {
    match mode {
        seis_runtime::GatherInterpolationMode::Linear => "linear",
    }
}

fn velocity_model_output_slug(model: &seis_runtime::VelocityFunctionSource) -> String {
    match model {
        seis_runtime::VelocityFunctionSource::ConstantVelocity { velocity_m_per_s } => {
            format!("constant-{}", format_factor(*velocity_m_per_s))
        }
        seis_runtime::VelocityFunctionSource::TimeVelocityPairs { .. } => {
            "time-velocity-pairs".to_string()
        }
        seis_runtime::VelocityFunctionSource::VelocityAssetReference { asset_id } => {
            sanitized_stem(&format!("velocity-asset-{asset_id}"), "velocity-asset")
        }
    }
}

fn optional_factor_output_slug(value: Option<f32>) -> String {
    value
        .map(format_factor)
        .unwrap_or_else(|| "none".to_string())
}

fn format_factor(value: f32) -> String {
    let mut formatted = format!("{value:.4}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    formatted.replace('.', "_")
}

fn source_store_stem(store_path: &str) -> String {
    let path = Path::new(store_path);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("dataset");
    sanitized_stem(stem, "dataset")
}

fn unique_store_candidate(dir: &Path, base_name: &str, extension: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{base_name}.{extension}"));
    let mut index = 2usize;
    while candidate.exists() {
        candidate = dir.join(format!("{base_name}-{index}.{extension}"));
        index += 1;
    }
    candidate
}

fn default_processing_store_path(
    app_paths: &AppPaths,
    input_store_path: &str,
    pipeline: &seis_runtime::TraceLocalProcessingPipeline,
) -> Result<String, String> {
    fs::create_dir_all(app_paths.derived_volumes_dir()).map_err(|error| error.to_string())?;
    let source_stem = source_store_stem(input_store_path);
    let pipeline_stem = pipeline_output_slug(pipeline);
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let base_name = format!("{source_stem}.{pipeline_stem}.{timestamp}");
    Ok(
        unique_store_candidate(app_paths.derived_volumes_dir(), &base_name, "tbvol")
            .display()
            .to_string(),
    )
}

fn default_subvolume_processing_store_path(
    app_paths: &AppPaths,
    input_store_path: &str,
    pipeline: &SubvolumeProcessingPipeline,
) -> Result<String, String> {
    fs::create_dir_all(app_paths.derived_volumes_dir()).map_err(|error| error.to_string())?;
    let source_stem = source_store_stem(input_store_path);
    let pipeline_stem = subvolume_pipeline_output_slug(pipeline);
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let base_name = format!("{source_stem}.{pipeline_stem}.{timestamp}");
    Ok(
        unique_store_candidate(app_paths.derived_volumes_dir(), &base_name, "tbvol")
            .display()
            .to_string(),
    )
}

fn default_gather_processing_store_path(
    app_paths: &AppPaths,
    input_store_path: &str,
    pipeline: &GatherProcessingPipeline,
) -> Result<String, String> {
    fs::create_dir_all(app_paths.derived_gathers_dir()).map_err(|error| error.to_string())?;
    let source_stem = source_store_stem(input_store_path);
    let pipeline_stem = gather_pipeline_output_slug(pipeline);
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let base_name = format!("{source_stem}.{pipeline_stem}.{timestamp}");
    Ok(
        unique_store_candidate(app_paths.derived_gathers_dir(), &base_name, "tbgath")
            .display()
            .to_string(),
    )
}

#[derive(Debug, Clone)]
struct TraceLocalProcessingStage {
    segment_pipeline: TraceLocalProcessingPipeline,
    lineage_pipeline: TraceLocalProcessingPipeline,
    stage_label: String,
    artifact: ProcessingJobArtifact,
}

fn processing_operation_display_label(operation: &seis_runtime::ProcessingOperation) -> String {
    match operation {
        seis_runtime::ProcessingOperation::AmplitudeScalar { factor } => {
            format!("amplitude scalar ({factor})")
        }
        seis_runtime::ProcessingOperation::TraceRmsNormalize => "trace RMS normalize".to_string(),
        seis_runtime::ProcessingOperation::AgcRms { window_ms } => {
            format!("RMS AGC ({window_ms} ms)")
        }
        seis_runtime::ProcessingOperation::PhaseRotation { angle_degrees } => {
            format!("phase rotation ({angle_degrees} deg)")
        }
        seis_runtime::ProcessingOperation::LowpassFilter { f3_hz, f4_hz, .. } => {
            format!("lowpass ({f3_hz}/{f4_hz} Hz)")
        }
        seis_runtime::ProcessingOperation::HighpassFilter { f1_hz, f2_hz, .. } => {
            format!("highpass ({f1_hz}/{f2_hz} Hz)")
        }
        seis_runtime::ProcessingOperation::BandpassFilter {
            f1_hz,
            f2_hz,
            f3_hz,
            f4_hz,
            ..
        } => {
            format!("bandpass ({f1_hz}/{f2_hz}/{f3_hz}/{f4_hz} Hz)")
        }
        seis_runtime::ProcessingOperation::VolumeArithmetic {
            operator,
            secondary_store_path,
        } => format!(
            "{} volume ({})",
            volume_arithmetic_operator_slug(*operator),
            display_store_stem(secondary_store_path)
        ),
    }
}

fn preview_processing_operation_ids(pipeline: &TraceLocalProcessingPipeline) -> Vec<&'static str> {
    pipeline
        .operations()
        .map(seis_runtime::ProcessingOperation::operator_id)
        .collect()
}

fn preview_processing_operation_labels(pipeline: &TraceLocalProcessingPipeline) -> Vec<String> {
    pipeline
        .operations()
        .map(processing_operation_display_label)
        .collect()
}

fn display_store_stem(store_path: &str) -> String {
    Path::new(store_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("volume")
        .to_string()
}

fn trace_local_operations(
    pipeline: &TraceLocalProcessingPipeline,
) -> Vec<seis_runtime::ProcessingOperation> {
    pipeline.operations().cloned().collect()
}

fn clone_pipeline_with_steps(
    pipeline: &TraceLocalProcessingPipeline,
    steps: Vec<seis_runtime::TraceLocalProcessingStep>,
) -> TraceLocalProcessingPipeline {
    TraceLocalProcessingPipeline {
        schema_version: pipeline.schema_version,
        revision: pipeline.revision,
        preset_id: pipeline.preset_id.clone(),
        name: pipeline.name.clone(),
        description: pipeline.description.clone(),
        steps,
    }
}

fn pipeline_prefix(
    pipeline: &TraceLocalProcessingPipeline,
    end_operation_index: usize,
) -> TraceLocalProcessingPipeline {
    clone_pipeline_with_steps(pipeline, pipeline.steps[..=end_operation_index].to_vec())
}

fn pipeline_segment(
    pipeline: &TraceLocalProcessingPipeline,
    start_operation_index: usize,
    end_operation_index: usize,
) -> TraceLocalProcessingPipeline {
    clone_pipeline_with_steps(
        pipeline,
        pipeline.steps[start_operation_index..=end_operation_index].to_vec(),
    )
}

fn resolve_trace_local_checkpoint_indexes(
    pipeline: &TraceLocalProcessingPipeline,
    allow_final_checkpoint: bool,
) -> Result<Vec<usize>, String> {
    if pipeline.steps.is_empty() {
        return Ok(Vec::new());
    }

    let last_index = pipeline.steps.len() - 1;
    let indexes = pipeline.checkpoint_indexes();

    for index in &indexes {
        if *index >= pipeline.steps.len() {
            return Err(format!(
                "Checkpoint index {index} is out of range for a pipeline with {} steps.",
                pipeline.steps.len()
            ));
        }
        if *index == last_index && !allow_final_checkpoint {
            return Err(
                "Checkpoint markers cannot target the final step because the final output is emitted automatically."
                    .to_string(),
            );
        }
    }

    Ok(indexes)
}

fn checkpoint_output_store_path(
    final_output_store_path: &str,
    job_id: &str,
    step_index: usize,
    operation: &seis_runtime::ProcessingOperation,
) -> String {
    let output_path = Path::new(final_output_store_path);
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("processed");
    let job_stem = sanitized_stem(job_id, "job");
    let operation_stem = sanitized_stem(operation.operator_id(), "step");
    parent
        .join(format!(
            "{stem}.{job_stem}.step-{:02}-{operation_stem}.tbvol",
            step_index + 1
        ))
        .display()
        .to_string()
}

fn build_trace_local_processing_stages_from(
    request: &RunTraceLocalProcessingRequest,
    final_output_store_path: &str,
    job_id: &str,
    start_operation_index: usize,
) -> Result<Vec<TraceLocalProcessingStage>, String> {
    let checkpoint_indexes = resolve_trace_local_checkpoint_indexes(&request.pipeline, false)?;
    let mut stage_end_indexes = checkpoint_indexes;
    let final_step_index = request.pipeline.operation_count().saturating_sub(1);
    stage_end_indexes.push(final_step_index);
    stage_end_indexes.retain(|index| *index >= start_operation_index);

    let mut stages = Vec::with_capacity(stage_end_indexes.len());
    let mut segment_start = start_operation_index;
    for end_index in stage_end_indexes {
        let operation = request
            .pipeline
            .steps
            .get(end_index)
            .map(|step| &step.operation)
            .ok_or_else(|| format!("Missing operation at stage end index {end_index}"))?;
        let stage_label = format!(
            "Step {}: {}",
            end_index + 1,
            processing_operation_display_label(operation)
        );
        let artifact = ProcessingJobArtifact {
            kind: if end_index == final_step_index {
                ProcessingJobArtifactKind::FinalOutput
            } else {
                ProcessingJobArtifactKind::Checkpoint
            },
            step_index: end_index,
            label: stage_label.clone(),
            store_path: if end_index == final_step_index {
                final_output_store_path.to_string()
            } else {
                checkpoint_output_store_path(final_output_store_path, job_id, end_index, operation)
            },
        };
        stages.push(TraceLocalProcessingStage {
            segment_pipeline: pipeline_segment(&request.pipeline, segment_start, end_index),
            lineage_pipeline: pipeline_prefix(&request.pipeline, end_index),
            stage_label,
            artifact,
        });
        segment_start = end_index + 1;
    }

    Ok(stages)
}

fn build_trace_local_checkpoint_stages_from_pipeline(
    pipeline: &TraceLocalProcessingPipeline,
    final_output_store_path: &str,
    job_id: &str,
    start_operation_index: usize,
    allow_final_checkpoint: bool,
) -> Result<Vec<TraceLocalProcessingStage>, String> {
    let stage_end_indexes =
        resolve_trace_local_checkpoint_indexes(pipeline, allow_final_checkpoint)?
            .into_iter()
            .filter(|index| *index >= start_operation_index)
            .collect::<Vec<_>>();

    let mut stages = Vec::with_capacity(stage_end_indexes.len());
    let mut segment_start = start_operation_index;
    for end_index in stage_end_indexes {
        let operation = pipeline
            .steps
            .get(end_index)
            .map(|step| &step.operation)
            .ok_or_else(|| format!("Missing operation at checkpoint index {end_index}"))?;
        let stage_label = format!(
            "Step {}: {}",
            end_index + 1,
            processing_operation_display_label(operation)
        );
        let artifact = ProcessingJobArtifact {
            kind: ProcessingJobArtifactKind::Checkpoint,
            step_index: end_index,
            label: stage_label.clone(),
            store_path: checkpoint_output_store_path(
                final_output_store_path,
                job_id,
                end_index,
                operation,
            ),
        };
        stages.push(TraceLocalProcessingStage {
            segment_pipeline: pipeline_segment(pipeline, segment_start, end_index),
            lineage_pipeline: pipeline_prefix(pipeline, end_index),
            stage_label,
            artifact,
        });
        segment_start = end_index + 1;
    }

    Ok(stages)
}

#[derive(Debug, Clone)]
struct ReusedTraceLocalCheckpoint {
    after_operation_index: usize,
    path: String,
    artifact: ProcessingJobArtifact,
}

fn resolve_reused_trace_local_checkpoint(
    processing_cache: &ProcessingCacheState,
    request: &RunTraceLocalProcessingRequest,
    allow_final_checkpoint: bool,
) -> Result<Option<ReusedTraceLocalCheckpoint>, String> {
    if !processing_cache.enabled() {
        return Ok(None);
    }

    let checkpoint_indexes =
        resolve_trace_local_checkpoint_indexes(&request.pipeline, allow_final_checkpoint)?;
    if checkpoint_indexes.is_empty() {
        return Ok(None);
    }

    let source_fingerprint = trace_local_source_fingerprint(&request.store_path)?;
    for checkpoint_index in checkpoint_indexes.into_iter().rev() {
        let lineage_pipeline = pipeline_prefix(&request.pipeline, checkpoint_index);
        let prefix_hash = trace_local_pipeline_hash(&lineage_pipeline)?;
        if let Some(hit) = processing_cache.lookup_prefix_artifact(
            TRACE_LOCAL_CACHE_FAMILY,
            &source_fingerprint,
            &prefix_hash,
            checkpoint_index + 1,
        )? {
            let operation = request
                .pipeline
                .steps
                .get(checkpoint_index)
                .map(|step| &step.operation)
                .ok_or_else(|| {
                    format!("Missing operation at checkpoint index {checkpoint_index}")
                })?;
            let artifact = ProcessingJobArtifact {
                kind: ProcessingJobArtifactKind::Checkpoint,
                step_index: checkpoint_index,
                label: format!(
                    "Reused checkpoint after step {}: {}",
                    checkpoint_index + 1,
                    processing_operation_display_label(operation)
                ),
                store_path: hit.path.clone(),
            };
            return Ok(Some(ReusedTraceLocalCheckpoint {
                after_operation_index: checkpoint_index,
                path: hit.path,
                artifact,
            }));
        }
    }

    Ok(None)
}

fn rewrite_trace_local_processing_lineage(
    store_path: &str,
    pipeline: &TraceLocalProcessingPipeline,
    artifact_kind: ProcessingJobArtifactKind,
) -> Result<(), String> {
    let manifest_path = Path::new(store_path).join("manifest.json");
    let mut manifest: TbvolManifest =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let lineage = manifest
        .volume
        .processing_lineage
        .as_mut()
        .ok_or_else(|| format!("Derived store is missing processing lineage: {store_path}"))?;
    lineage.pipeline = ProcessingPipelineSpec::TraceLocal {
        pipeline: pipeline.clone(),
    };
    lineage.artifact_role = match artifact_kind {
        ProcessingJobArtifactKind::Checkpoint => ProcessingArtifactRole::Checkpoint,
        ProcessingJobArtifactKind::FinalOutput => ProcessingArtifactRole::FinalOutput,
    };
    fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn rewrite_subvolume_processing_lineage(
    store_path: &str,
    pipeline: &SubvolumeProcessingPipeline,
    artifact_kind: ProcessingJobArtifactKind,
) -> Result<(), String> {
    let manifest_path = Path::new(store_path).join("manifest.json");
    let mut manifest: TbvolManifest =
        serde_json::from_slice(&fs::read(&manifest_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let lineage = manifest
        .volume
        .processing_lineage
        .as_mut()
        .ok_or_else(|| format!("Derived store is missing processing lineage: {store_path}"))?;
    lineage.pipeline = ProcessingPipelineSpec::Subvolume {
        pipeline: pipeline.clone(),
    };
    lineage.artifact_role = match artifact_kind {
        ProcessingJobArtifactKind::Checkpoint => ProcessingArtifactRole::Checkpoint,
        ProcessingJobArtifactKind::FinalOutput => ProcessingArtifactRole::FinalOutput,
    };
    fs::write(
        manifest_path,
        serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

fn normalized_path_key(path: &str) -> String {
    path.trim().replace('/', "\\").to_ascii_lowercase()
}

#[derive(serde::Serialize)]
struct TraceLocalPipelineCacheIdentity<'a> {
    schema_version: u32,
    revision: u32,
    operations: &'a [seis_runtime::ProcessingOperation],
}

fn trace_local_source_fingerprint(store_path: &str) -> Result<String, String> {
    let manifest_path = Path::new(store_path).join("manifest.json");
    let manifest = fs::read(&manifest_path).map_err(|error| {
        format!(
            "Failed to read store manifest for cache fingerprint ({}): {error}",
            manifest_path.display()
        )
    })?;
    Ok(ProcessingCacheState::fingerprint_bytes(&manifest))
}

fn trace_local_pipeline_hash(pipeline: &TraceLocalProcessingPipeline) -> Result<String, String> {
    let operations = trace_local_operations(pipeline);
    ProcessingCacheState::fingerprint_json(&TraceLocalPipelineCacheIdentity {
        schema_version: pipeline.schema_version,
        revision: pipeline.revision,
        operations: &operations,
    })
}

fn materialize_options_for_store(input_store_path: &str) -> Result<MaterializeOptions, String> {
    let chunk_shape = open_store(input_store_path)
        .map_err(|error| error.to_string())?
        .manifest
        .tile_shape;
    Ok(MaterializeOptions {
        chunk_shape,
        ..MaterializeOptions::default()
    })
}

fn register_processing_store_artifact(
    app: &AppHandle,
    input_store_path: &str,
    artifact: &ProcessingJobArtifact,
) -> Result<(), String> {
    let workspace = match app.try_state::<WorkspaceState>() {
        Some(state) => state,
        None => return Ok(()),
    };
    let source_state = workspace.load_state()?;
    let source_key = normalized_path_key(input_store_path);
    let source_entry = source_state.entries.iter().find(|entry| {
        entry
            .imported_store_path
            .as_deref()
            .map(normalized_path_key)
            .as_deref()
            == Some(source_key.as_str())
            || entry
                .preferred_store_path
                .as_deref()
                .map(normalized_path_key)
                .as_deref()
                == Some(source_key.as_str())
            || entry
                .last_dataset
                .as_ref()
                .map(|dataset| normalized_path_key(&dataset.store_path))
                .as_deref()
                == Some(source_key.as_str())
    });
    let source_label = source_entry
        .map(|entry| entry.display_name.clone())
        .unwrap_or_else(|| display_store_stem(input_store_path));
    let dataset = open_dataset_summary(OpenDatasetRequest {
        schema_version: IPC_SCHEMA_VERSION,
        store_path: artifact.store_path.clone(),
    })
    .map_err(|error| error.to_string())?
    .dataset;

    workspace.upsert_entry(UpsertDatasetEntryRequest {
        schema_version: IPC_SCHEMA_VERSION,
        entry_id: None,
        display_name: Some(format!("{source_label} · {}", artifact.label)),
        source_path: None,
        preferred_store_path: Some(dataset.store_path.clone()),
        imported_store_path: Some(dataset.store_path.clone()),
        dataset: Some(dataset),
        session_pipelines: source_entry.map(|entry| entry.session_pipelines.clone()),
        active_session_pipeline_id: source_entry
            .and_then(|entry| entry.active_session_pipeline_id.clone()),
        make_active: false,
    })?;

    Ok(())
}

fn import_store_path_for_input(
    dir: &Path,
    input_path: &str,
    extension: &str,
) -> Result<String, String> {
    let input_path = input_path.trim();
    if input_path.is_empty() {
        return Err("Input path is required.".to_string());
    }

    fs::create_dir_all(dir).map_err(|error| error.to_string())?;

    let source = Path::new(input_path);
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("volume");
    let sanitized_stem = sanitized_stem(stem, "volume");

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input_path.to_ascii_lowercase().hash(&mut hasher);
    let fingerprint = hasher.finish();
    let store_name = format!("{sanitized_stem}-{fingerprint:016x}.{extension}");
    Ok(dir.join(store_name).display().to_string())
}

fn import_volume_store_path_for_input(
    app_paths: &AppPaths,
    input_path: &str,
) -> Result<String, String> {
    import_store_path_for_input(app_paths.imported_volumes_dir(), input_path, "tbvol")
}

fn import_prestack_store_path_for_input(
    app_paths: &AppPaths,
    input_path: &str,
) -> Result<String, String> {
    import_store_path_for_input(app_paths.imported_gathers_dir(), input_path, "tbgath")
}

#[tauri::command]
fn default_import_store_path_command(app: AppHandle, input_path: String) -> Result<String, String> {
    let app_paths = AppPaths::resolve(&app)?;
    import_volume_store_path_for_input(&app_paths, &input_path)
}

#[tauri::command]
fn default_import_prestack_store_path_command(
    app: AppHandle,
    input_path: String,
) -> Result<String, String> {
    let app_paths = AppPaths::resolve(&app)?;
    import_prestack_store_path_for_input(&app_paths, &input_path)
}

#[tauri::command]
fn default_processing_store_path_command(
    app: AppHandle,
    store_path: String,
    pipeline: seis_runtime::TraceLocalProcessingPipeline,
) -> Result<String, String> {
    let app_paths = AppPaths::resolve(&app)?;
    default_processing_store_path(&app_paths, &store_path, &pipeline)
}

#[tauri::command]
fn default_subvolume_processing_store_path_command(
    app: AppHandle,
    store_path: String,
    pipeline: SubvolumeProcessingPipeline,
) -> Result<String, String> {
    let app_paths = AppPaths::resolve(&app)?;
    default_subvolume_processing_store_path(&app_paths, &store_path, &pipeline)
}

#[tauri::command]
fn default_gather_processing_store_path_command(
    app: AppHandle,
    store_path: String,
    pipeline: seis_runtime::GatherProcessingPipeline,
) -> Result<String, String> {
    let app_paths = AppPaths::resolve(&app)?;
    default_gather_processing_store_path(&app_paths, &store_path, &pipeline)
}

fn build_app_menu<R: tauri::Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let open_volume = MenuItem::with_id(
        app,
        FILE_OPEN_VOLUME_MENU_ID,
        "&Open Volume...",
        true,
        None::<&str>,
    )?;
    let velocity_model = MenuItem::with_id(
        app,
        VELOCITY_MODEL_MENU_ID,
        "&Velocity Model...",
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let close_window = PredefinedMenuItem::close_window(app, None)?;

    Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                "&File",
                true,
                &[&open_volume, &separator, &close_window],
            )?,
            &Submenu::with_items(app, "&Velocity", true, &[&velocity_model])?,
        ],
    )
}

#[tauri::command]
fn preflight_import_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    input_path: String,
    geometry_override: Option<seis_contracts_interop::SegyGeometryOverride>,
) -> Result<SurveyPreflightResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "preflight_import",
        "Starting survey preflight",
        Some(build_fields([
            ("inputPath", json_value(&input_path)),
            ("stage", json_value("validate_input")),
        ])),
    );
    diagnostics.verbose_progress(
        &app,
        &operation,
        "Validated preflight inputs",
        Some(build_fields([("inputPath", json_value(&input_path))])),
    );

    diagnostics.progress(
        &app,
        &operation,
        "Inspecting SEG-Y survey metadata",
        Some(build_fields([("stage", json_value("inspect_segy"))])),
    );

    let result = preflight_dataset(SurveyPreflightRequest {
        schema_version: IPC_SCHEMA_VERSION,
        input_path,
        geometry_override,
    });

    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Survey preflight completed",
                Some(build_fields([
                    ("stage", json_value("summarize")),
                    ("classification", json_value(&response.classification)),
                    ("stackingState", json_value(&response.stacking_state)),
                    ("organization", json_value(&response.organization)),
                    ("layout", json_value(&response.layout)),
                    ("traceCount", json_value(response.trace_count)),
                    ("samplesPerTrace", json_value(response.samples_per_trace)),
                    ("completenessRatio", json_value(response.completeness_ratio)),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Survey preflight failed",
                Some(build_fields([
                    ("stage", json_value("inspect_segy")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn import_dataset_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    input_path: String,
    output_store_path: String,
    geometry_override: Option<seis_contracts_interop::SegyGeometryOverride>,
    overwrite_existing: bool,
) -> Result<ImportDatasetResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "import_dataset",
        "Starting volume import",
        Some(build_fields([
            ("inputPath", json_value(&input_path)),
            ("outputStorePath", json_value(&output_store_path)),
            ("overwriteExisting", json_value(overwrite_existing)),
            ("stage", json_value("validate_input")),
        ])),
    );
    diagnostics.verbose_progress(
        &app,
        &operation,
        "Validated import inputs",
        Some(build_fields([
            ("inputPath", json_value(&input_path)),
            ("outputStorePath", json_value(&output_store_path)),
            ("overwriteExisting", json_value(overwrite_existing)),
        ])),
    );
    diagnostics.progress(
        &app,
        &operation,
        "Reading input volume and building runtime store",
        Some(build_fields([("stage", json_value("read_input"))])),
    );

    let result = import_dataset(ImportDatasetRequest {
        schema_version: IPC_SCHEMA_VERSION,
        input_path,
        output_store_path,
        geometry_override,
        overwrite_existing,
    });

    match result {
        Ok(response) => {
            diagnostics.progress(
                &app,
                &operation,
                "Finalizing runtime store metadata",
                Some(build_fields([
                    ("stage", json_value("finalize_store")),
                    ("storePath", json_value(&response.dataset.store_path)),
                ])),
            );
            diagnostics.complete(
                &app,
                &operation,
                "Volume import completed",
                Some(build_fields([
                    ("storePath", json_value(&response.dataset.store_path)),
                    ("datasetId", json_value(&response.dataset.descriptor.id.0)),
                    (
                        "datasetLabel",
                        json_value(&response.dataset.descriptor.label),
                    ),
                    ("shape", json_value(response.dataset.descriptor.shape)),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Volume import failed",
                Some(build_fields([
                    ("stage", json_value("build_store")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn import_prestack_offset_dataset_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    request: ImportPrestackOffsetDatasetRequest,
) -> Result<ImportPrestackOffsetDatasetResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "import_prestack_offset_dataset",
        "Starting prestack SEG-Y import",
        Some(build_fields([
            ("inputPath", json_value(&request.input_path)),
            ("outputStorePath", json_value(&request.output_store_path)),
            ("overwriteExisting", json_value(request.overwrite_existing)),
            (
                "thirdAxisField",
                json_value(format!("{:?}", request.third_axis_field).to_ascii_lowercase()),
            ),
            ("stage", json_value("validate_input")),
        ])),
    );
    diagnostics.verbose_progress(
        &app,
        &operation,
        "Validated prestack import inputs",
        Some(build_fields([
            ("inputPath", json_value(&request.input_path)),
            ("outputStorePath", json_value(&request.output_store_path)),
            ("overwriteExisting", json_value(request.overwrite_existing)),
        ])),
    );
    diagnostics.progress(
        &app,
        &operation,
        "Reading SEG-Y gather survey and building prestack runtime store",
        Some(build_fields([("stage", json_value("read_segy"))])),
    );

    let result = import_prestack_offset_dataset(request);
    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Prestack SEG-Y import completed",
                Some(build_fields([
                    ("storePath", json_value(&response.dataset.store_path)),
                    ("datasetId", json_value(&response.dataset.descriptor.id.0)),
                    (
                        "datasetLabel",
                        json_value(&response.dataset.descriptor.label),
                    ),
                    ("shape", json_value(response.dataset.descriptor.shape)),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Prestack SEG-Y import failed",
                Some(build_fields([
                    ("stage", json_value("build_store")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn open_dataset_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
) -> Result<OpenDatasetResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "open_dataset",
        "Opening runtime store",
        Some(build_fields([
            ("storePath", json_value(&store_path)),
            ("stage", json_value("validate_input")),
        ])),
    );
    diagnostics.progress(
        &app,
        &operation,
        "Loading runtime store summary",
        Some(build_fields([("stage", json_value("open_store"))])),
    );

    let result = open_dataset_summary(OpenDatasetRequest {
        schema_version: IPC_SCHEMA_VERSION,
        store_path,
    });

    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Runtime store opened",
                Some(build_fields([
                    ("stage", json_value("summarize")),
                    ("storePath", json_value(&response.dataset.store_path)),
                    ("datasetId", json_value(&response.dataset.descriptor.id.0)),
                    (
                        "datasetLabel",
                        json_value(&response.dataset.descriptor.label),
                    ),
                    ("shape", json_value(response.dataset.descriptor.shape)),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Opening runtime store failed",
                Some(build_fields([
                    ("stage", json_value("open_store")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn export_dataset_segy_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    output_path: String,
    overwrite_existing: bool,
) -> Result<ExportSegyResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "export_dataset_segy",
        "Starting SEG-Y export",
        Some(build_fields([
            ("storePath", json_value(&store_path)),
            ("outputPath", json_value(&output_path)),
            ("overwriteExisting", json_value(overwrite_existing)),
            ("stage", json_value("validate_input")),
        ])),
    );
    diagnostics.progress(
        &app,
        &operation,
        "Reading tbvol export metadata and writing SEG-Y",
        Some(build_fields([("stage", json_value("write_segy"))])),
    );

    let result = export_dataset_segy(ExportSegyRequest {
        schema_version: IPC_SCHEMA_VERSION,
        store_path,
        output_path,
        overwrite_existing,
    });

    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "SEG-Y export completed",
                Some(build_fields([
                    ("stage", json_value("complete")),
                    ("storePath", json_value(&response.store_path)),
                    ("outputPath", json_value(&response.output_path)),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "SEG-Y export failed",
                Some(build_fields([
                    ("stage", json_value("write_segy")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn ensure_demo_survey_time_depth_transform_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
) -> Result<String, String> {
    let operation = diagnostics.start_operation(
        &app,
        "ensure_demo_survey_time_depth_transform",
        "Ensuring synthetic survey 3D time-depth transform",
        Some(build_fields([
            ("storePath", json_value(&store_path)),
            ("stage", json_value("validate_input")),
        ])),
    );
    diagnostics.progress(
        &app,
        &operation,
        "Building or refreshing the synthetic survey-aligned transform asset",
        Some(build_fields([("stage", json_value("build_transform"))])),
    );

    match ensure_demo_survey_time_depth_transform(store_path.clone()) {
        Ok(asset_id) => {
            diagnostics.complete(
                &app,
                &operation,
                "Synthetic survey 3D transform is ready",
                Some(build_fields([
                    ("storePath", json_value(&store_path)),
                    ("assetId", json_value(&asset_id)),
                    ("stage", json_value("complete")),
                ])),
            );
            Ok(asset_id)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Ensuring synthetic survey 3D transform failed",
                Some(build_fields([
                    ("storePath", json_value(&store_path)),
                    ("stage", json_value("build_transform")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn load_velocity_models_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
) -> Result<LoadVelocityModelsResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "load_velocity_models",
        "Loading survey velocity models",
        Some(build_fields([
            ("storePath", json_value(&store_path)),
            ("stage", json_value("load_velocity_models")),
        ])),
    );

    match load_velocity_models(store_path.clone()) {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Velocity models loaded",
                Some(build_fields([
                    ("storePath", json_value(&store_path)),
                    ("modelCount", json_value(response.models.len())),
                    ("stage", json_value("complete")),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Loading velocity models failed",
                Some(build_fields([
                    ("storePath", json_value(&store_path)),
                    ("stage", json_value("load_velocity_models")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn import_velocity_functions_model_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    input_path: String,
    velocity_kind: VelocityQuantityKind,
) -> Result<traceboost_app::ImportVelocityFunctionsModelResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "import_velocity_functions_model",
        "Importing sparse velocity functions into a survey transform",
        Some(build_fields([
            ("storePath", json_value(&store_path)),
            ("inputPath", json_value(&input_path)),
            (
                "velocityKind",
                json_value(format!("{velocity_kind:?}").to_ascii_lowercase()),
            ),
            ("stage", json_value("parse_and_build")),
        ])),
    );

    match import_velocity_functions_model(store_path.clone(), input_path.clone(), velocity_kind) {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Velocity functions imported and compiled into a survey transform",
                Some(build_fields([
                    ("storePath", json_value(&store_path)),
                    ("inputPath", json_value(&input_path)),
                    ("assetId", json_value(&response.model.id)),
                    ("profileCount", json_value(response.profile_count)),
                    ("sampleCount", json_value(response.sample_count)),
                    ("stage", json_value("complete")),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Velocity functions import failed",
                Some(build_fields([
                    ("storePath", json_value(&store_path)),
                    ("inputPath", json_value(&input_path)),
                    ("stage", json_value("parse_and_build")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn build_velocity_model_transform_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    request: BuildSurveyTimeDepthTransformRequest,
) -> Result<seis_contracts_interop::SurveyTimeDepthTransform3D, String> {
    let operation = diagnostics.start_operation(
        &app,
        "build_velocity_model_transform",
        "Building authored velocity model into a survey transform",
        Some(build_fields([
            ("storePath", json_value(&request.store_path)),
            ("modelId", json_value(&request.model.id)),
            ("modelName", json_value(&request.model.name)),
            ("intervalCount", json_value(request.model.intervals.len())),
            ("stage", json_value("build_transform")),
        ])),
    );

    match build_velocity_model_transform(request) {
        Ok(model) => {
            diagnostics.complete(
                &app,
                &operation,
                "Velocity model compiled into a survey transform",
                Some(build_fields([
                    ("assetId", json_value(&model.id)),
                    ("modelName", json_value(&model.name)),
                    ("stage", json_value("complete")),
                ])),
            );
            Ok(model)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Velocity model build failed",
                Some(build_fields([
                    ("stage", json_value("build_transform")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn export_dataset_zarr_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    output_path: String,
    overwrite_existing: bool,
) -> Result<ExportZarrResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "export_dataset_zarr",
        "Starting Zarr export",
        Some(build_fields([
            ("storePath", json_value(&store_path)),
            ("outputPath", json_value(&output_path)),
            ("overwriteExisting", json_value(overwrite_existing)),
            ("stage", json_value("validate_input")),
        ])),
    );
    diagnostics.progress(
        &app,
        &operation,
        "Reading tbvol data and writing Zarr",
        Some(build_fields([("stage", json_value("write_zarr"))])),
    );

    match export_dataset_zarr(store_path, output_path, overwrite_existing) {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Zarr export completed",
                Some(build_fields([
                    ("stage", json_value("complete")),
                    ("storePath", json_value(&response.store_path)),
                    ("outputPath", json_value(&response.output_path)),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Zarr export failed",
                Some(build_fields([
                    ("stage", json_value("write_zarr")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn get_dataset_export_capabilities_command(
    store_path: String,
) -> Result<DatasetExportCapabilitiesResponse, String> {
    let handle = open_store(&store_path).map_err(|error| error.to_string())?;
    let segy = match handle.manifest.volume.segy_export.as_ref() {
        Some(descriptor) if descriptor.contains_synthetic_traces => DatasetExportFormatCapability {
            available: false,
            reason: Some(
                "SEG-Y export is unavailable because this volume contains synthetic or regularized traces."
                    .to_string(),
            ),
            default_output_path: default_export_segy_path(&store_path)
                .display()
                .to_string(),
        },
        Some(_) => DatasetExportFormatCapability {
            available: true,
            reason: None,
            default_output_path: default_export_segy_path(&store_path)
                .display()
                .to_string(),
        },
        None => DatasetExportFormatCapability {
            available: false,
            reason: Some(
                "SEG-Y export is unavailable because this tbvol does not carry captured SEG-Y provenance."
                    .to_string(),
            ),
            default_output_path: default_export_segy_path(&store_path)
                .display()
                .to_string(),
        },
    };
    let zarr = DatasetExportFormatCapability {
        available: true,
        reason: None,
        default_output_path: default_export_zarr_path(&store_path).display().to_string(),
    };

    Ok(DatasetExportCapabilitiesResponse {
        store_path,
        segy,
        zarr,
    })
}

#[tauri::command]
fn import_horizon_xyz_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    input_paths: Vec<String>,
    source_coordinate_reference_id: Option<String>,
    source_coordinate_reference_name: Option<String>,
    assume_same_as_survey: bool,
) -> Result<ImportHorizonXyzResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "import_horizon_xyz",
        "Importing horizon xyz files",
        Some(build_fields([
            ("storePath", json_value(&store_path)),
            ("inputPathCount", json_value(input_paths.len())),
            (
                "sourceCoordinateReferenceId",
                json_value(source_coordinate_reference_id.as_deref()),
            ),
            ("assumeSameAsSurvey", json_value(assume_same_as_survey)),
            ("stage", json_value("validate_input")),
        ])),
    );

    let result = import_horizon_xyz(ImportHorizonXyzRequest {
        schema_version: IPC_SCHEMA_VERSION,
        store_path,
        input_paths,
        source_coordinate_reference_id,
        source_coordinate_reference_name,
        assume_same_as_survey,
    });

    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Horizon xyz files imported",
                Some(build_fields([
                    ("stage", json_value("import_horizons")),
                    ("horizonCount", json_value(response.imported.len())),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Horizon xyz import failed",
                Some(build_fields([
                    ("stage", json_value("import_horizons")),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn load_section_horizons_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    axis: SectionAxis,
    index: usize,
) -> Result<LoadSectionHorizonsResponse, String> {
    let axis_name = format!("{axis:?}").to_ascii_lowercase();
    let operation = diagnostics.start_operation(
        &app,
        "load_section_horizons",
        "Loading section horizon overlays",
        Some(build_fields([
            ("storePath", json_value(&store_path)),
            ("axis", json_value(&axis_name)),
            ("index", json_value(index)),
            ("stage", json_value("validate_input")),
        ])),
    );

    let result = load_section_horizons(seis_contracts_interop::LoadSectionHorizonsRequest {
        schema_version: IPC_SCHEMA_VERSION,
        store_path,
        axis,
        index,
    });

    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Section horizon overlays loaded",
                Some(build_fields([
                    ("stage", json_value("load_section_horizons")),
                    ("axis", json_value(&axis_name)),
                    ("index", json_value(index)),
                    ("horizonCount", json_value(response.overlays.len())),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Loading section horizon overlays failed",
                Some(build_fields([
                    ("stage", json_value("load_section_horizons")),
                    ("axis", json_value(&axis_name)),
                    ("index", json_value(index)),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn load_section_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    axis: SectionAxis,
    index: usize,
) -> Result<SectionView, String> {
    let axis_name = format!("{axis:?}").to_ascii_lowercase();
    let handle = open_store(&store_path).ok();
    let mut start_fields = vec![
        ("storePath", json_value(&store_path)),
        ("axis", json_value(&axis_name)),
        ("index", json_value(index)),
        ("stage", json_value("validate_input")),
    ];
    if let Some(handle) = handle.as_ref() {
        start_fields.push(("datasetId", json_value(&handle.dataset_id().0)));
        start_fields.push(("shape", json_value(handle.manifest.volume.shape)));
        append_section_debug_fields(
            &mut start_fields,
            handle,
            axis,
            index,
            "axisLength",
            "axisRange",
            "requestedCoordinateValue",
        );
    }
    let operation = diagnostics.start_operation(
        &app,
        "load_section",
        "Loading section view",
        Some(build_fields(start_fields)),
    );
    diagnostics.progress(
        &app,
        &operation,
        "Opening runtime store for section load",
        Some(build_fields([("stage", json_value("open_store"))])),
    );

    let result = match handle {
        Some(handle) => handle.section_view(axis, index),
        None => open_store(store_path.clone()).and_then(|handle| handle.section_view(axis, index)),
    };
    match result {
        Ok(section) => {
            diagnostics.complete(
                &app,
                &operation,
                "Section view loaded",
                Some(build_fields([
                    ("stage", json_value("load_section")),
                    ("axis", json_value(&axis_name)),
                    ("index", json_value(index)),
                    ("traces", json_value(section.traces)),
                    ("samples", json_value(section.samples)),
                ])),
            );
            Ok(section)
        }
        Err(error) => {
            let message = error.to_string();
            let mut failure_fields = vec![
                ("stage", json_value("load_section")),
                ("axis", json_value(&axis_name)),
                ("index", json_value(index)),
                ("error", json_value(&message)),
            ];
            if let Some(handle) = open_store(&store_path).ok() {
                failure_fields.push(("datasetId", json_value(&handle.dataset_id().0)));
                failure_fields.push(("shape", json_value(handle.manifest.volume.shape)));
                append_section_debug_fields(
                    &mut failure_fields,
                    &handle,
                    axis,
                    index,
                    "axisLength",
                    "axisRange",
                    "requestedCoordinateValue",
                );
            }
            diagnostics.fail(
                &app,
                &operation,
                "Section load failed",
                Some(build_fields(failure_fields)),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn load_section_binary_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    axis: SectionAxis,
    index: usize,
) -> Result<Response, String> {
    let axis_name = format!("{axis:?}").to_ascii_lowercase();
    let handle = open_store(&store_path).ok();
    let mut start_fields = vec![
        ("storePath", json_value(&store_path)),
        ("axis", json_value(&axis_name)),
        ("index", json_value(index)),
        ("stage", json_value("validate_input")),
    ];
    if let Some(handle) = handle.as_ref() {
        start_fields.push(("datasetId", json_value(&handle.dataset_id().0)));
        start_fields.push(("shape", json_value(handle.manifest.volume.shape)));
        append_section_debug_fields(
            &mut start_fields,
            handle,
            axis,
            index,
            "axisLength",
            "axisRange",
            "requestedCoordinateValue",
        );
    }
    let operation = diagnostics.start_operation(
        &app,
        "load_section_binary",
        "Loading section view (binary)",
        Some(build_fields(start_fields)),
    );
    diagnostics.progress(
        &app,
        &operation,
        "Opening runtime store for binary section load",
        Some(build_fields([("stage", json_value("open_store"))])),
    );

    let result = match handle {
        Some(handle) => handle.section_view(axis, index),
        None => open_store(store_path.clone()).and_then(|handle| handle.section_view(axis, index)),
    };
    match result {
        Ok(section) => {
            let payload_bytes = section.horizontal_axis_f64le.len()
                + section
                    .inline_axis_f64le
                    .as_ref()
                    .map(Vec::len)
                    .unwrap_or_default()
                + section
                    .xline_axis_f64le
                    .as_ref()
                    .map(Vec::len)
                    .unwrap_or_default()
                + section.sample_axis_f32le.len()
                + section.amplitudes_f32le.len();
            diagnostics.complete(
                &app,
                &operation,
                "Section view loaded (binary)",
                Some(build_fields([
                    ("stage", json_value("load_section_binary")),
                    ("axis", json_value(&axis_name)),
                    ("index", json_value(index)),
                    ("traces", json_value(section.traces)),
                    ("samples", json_value(section.samples)),
                    ("payloadBytes", json_value(payload_bytes)),
                ])),
            );
            pack_section_response(section)
        }
        Err(error) => {
            let message = error.to_string();
            let mut failure_fields = vec![
                ("stage", json_value("load_section_binary")),
                ("axis", json_value(&axis_name)),
                ("index", json_value(index)),
                ("error", json_value(&message)),
            ];
            if let Some(handle) = open_store(&store_path).ok() {
                failure_fields.push(("datasetId", json_value(&handle.dataset_id().0)));
                failure_fields.push(("shape", json_value(handle.manifest.volume.shape)));
                append_section_debug_fields(
                    &mut failure_fields,
                    &handle,
                    axis,
                    index,
                    "axisLength",
                    "axisRange",
                    "requestedCoordinateValue",
                );
            }
            diagnostics.fail(
                &app,
                &operation,
                "Loading section view (binary) failed",
                Some(build_fields(failure_fields)),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn load_depth_converted_section_binary_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    axis: SectionAxis,
    index: usize,
    velocity_model: VelocityFunctionSource,
    velocity_kind: VelocityQuantityKind,
) -> Result<Response, String> {
    let axis_name = format!("{axis:?}").to_ascii_lowercase();
    let velocity_kind_name = format!("{velocity_kind:?}").to_ascii_lowercase();
    let handle = open_store(&store_path).ok();
    let mut start_fields = vec![
        ("storePath", json_value(&store_path)),
        ("axis", json_value(&axis_name)),
        ("index", json_value(index)),
        ("velocityKind", json_value(&velocity_kind_name)),
        ("stage", json_value("validate_input")),
    ];
    if let Some(handle) = handle.as_ref() {
        start_fields.push(("datasetId", json_value(&handle.dataset_id().0)));
        start_fields.push(("shape", json_value(handle.manifest.volume.shape)));
        append_section_debug_fields(
            &mut start_fields,
            handle,
            axis,
            index,
            "axisLength",
            "axisRange",
            "requestedCoordinateValue",
        );
    }
    let operation = diagnostics.start_operation(
        &app,
        "load_depth_converted_section_binary",
        "Loading depth-converted section view (binary)",
        Some(build_fields(start_fields)),
    );

    let result = load_depth_converted_section(
        store_path.clone(),
        axis,
        index,
        velocity_model,
        velocity_kind,
    );
    match result {
        Ok(section) => {
            let payload_bytes = section.horizontal_axis_f64le.len()
                + section
                    .inline_axis_f64le
                    .as_ref()
                    .map(Vec::len)
                    .unwrap_or_default()
                + section
                    .xline_axis_f64le
                    .as_ref()
                    .map(Vec::len)
                    .unwrap_or_default()
                + section.sample_axis_f32le.len()
                + section.amplitudes_f32le.len();
            diagnostics.complete(
                &app,
                &operation,
                "Depth-converted section view loaded (binary)",
                Some(build_fields([
                    ("stage", json_value("load_depth_converted_section_binary")),
                    ("axis", json_value(&axis_name)),
                    ("index", json_value(index)),
                    ("velocityKind", json_value(&velocity_kind_name)),
                    ("traces", json_value(section.traces)),
                    ("samples", json_value(section.samples)),
                    ("payloadBytes", json_value(payload_bytes)),
                ])),
            );
            pack_section_response(section)
        }
        Err(error) => {
            let message = error.to_string();
            let mut failure_fields = vec![
                ("stage", json_value("load_depth_converted_section_binary")),
                ("axis", json_value(&axis_name)),
                ("index", json_value(index)),
                ("velocityKind", json_value(&velocity_kind_name)),
                ("error", json_value(&message)),
            ];
            if let Some(handle) = open_store(&store_path).ok() {
                failure_fields.push(("datasetId", json_value(&handle.dataset_id().0)));
                failure_fields.push(("shape", json_value(handle.manifest.volume.shape)));
                append_section_debug_fields(
                    &mut failure_fields,
                    &handle,
                    axis,
                    index,
                    "axisLength",
                    "axisRange",
                    "requestedCoordinateValue",
                );
            }
            diagnostics.fail(
                &app,
                &operation,
                "Loading depth-converted section view (binary) failed",
                Some(build_fields(failure_fields)),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn load_resolved_section_display_binary_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    axis: SectionAxis,
    index: usize,
    domain: TimeDepthDomain,
    velocity_model: Option<VelocityFunctionSource>,
    velocity_kind: Option<VelocityQuantityKind>,
    include_velocity_overlay: bool,
) -> Result<Response, String> {
    let axis_name = format!("{axis:?}").to_ascii_lowercase();
    let domain_name = format!("{domain:?}").to_ascii_lowercase();
    let velocity_kind_name = velocity_kind
        .map(|kind| format!("{kind:?}").to_ascii_lowercase())
        .unwrap_or_else(|| "none".to_string());
    let handle = open_store(&store_path).ok();
    let mut start_fields = vec![
        ("storePath", json_value(&store_path)),
        ("axis", json_value(&axis_name)),
        ("index", json_value(index)),
        ("domain", json_value(&domain_name)),
        ("velocityKind", json_value(&velocity_kind_name)),
        (
            "includeVelocityOverlay",
            json_value(include_velocity_overlay),
        ),
        ("stage", json_value("validate_input")),
    ];
    if let Some(handle) = handle.as_ref() {
        start_fields.push(("datasetId", json_value(&handle.dataset_id().0)));
        start_fields.push(("shape", json_value(handle.manifest.volume.shape)));
        append_section_debug_fields(
            &mut start_fields,
            handle,
            axis,
            index,
            "axisLength",
            "axisRange",
            "requestedCoordinateValue",
        );
    }
    let operation = diagnostics.start_operation(
        &app,
        "load_resolved_section_display_binary",
        "Loading resolved section display (binary)",
        Some(build_fields(start_fields)),
    );

    let result = load_resolved_section_display(
        store_path.clone(),
        axis,
        index,
        domain,
        velocity_model,
        velocity_kind,
        include_velocity_overlay,
    );
    match result {
        Ok(display) => {
            let payload_bytes = display.section.horizontal_axis_f64le.len()
                + display
                    .section
                    .inline_axis_f64le
                    .as_ref()
                    .map(Vec::len)
                    .unwrap_or_default()
                + display
                    .section
                    .xline_axis_f64le
                    .as_ref()
                    .map(Vec::len)
                    .unwrap_or_default()
                + display.section.sample_axis_f32le.len()
                + display.section.amplitudes_f32le.len()
                + display
                    .scalar_overlays
                    .iter()
                    .map(|overlay| overlay.values_f32le.len())
                    .sum::<usize>();
            diagnostics.complete(
                &app,
                &operation,
                "Resolved section display loaded (binary)",
                Some(build_fields([
                    ("stage", json_value("load_resolved_section_display_binary")),
                    ("axis", json_value(&axis_name)),
                    ("index", json_value(index)),
                    ("domain", json_value(&domain_name)),
                    ("velocityKind", json_value(&velocity_kind_name)),
                    (
                        "includeVelocityOverlay",
                        json_value(include_velocity_overlay),
                    ),
                    ("traces", json_value(display.section.traces)),
                    ("samples", json_value(display.section.samples)),
                    (
                        "scalarOverlayCount",
                        json_value(display.scalar_overlays.len()),
                    ),
                    (
                        "horizonOverlayCount",
                        json_value(display.horizon_overlays.len()),
                    ),
                    ("payloadBytes", json_value(payload_bytes)),
                ])),
            );
            pack_section_display_response(display)
        }
        Err(error) => {
            let message = error.to_string();
            let mut failure_fields = vec![
                ("stage", json_value("load_resolved_section_display_binary")),
                ("axis", json_value(&axis_name)),
                ("index", json_value(index)),
                ("domain", json_value(&domain_name)),
                ("velocityKind", json_value(&velocity_kind_name)),
                (
                    "includeVelocityOverlay",
                    json_value(include_velocity_overlay),
                ),
                ("error", json_value(&message)),
            ];
            if let Some(handle) = open_store(&store_path).ok() {
                failure_fields.push(("datasetId", json_value(&handle.dataset_id().0)));
                failure_fields.push(("shape", json_value(handle.manifest.volume.shape)));
                append_section_debug_fields(
                    &mut failure_fields,
                    &handle,
                    axis,
                    index,
                    "axisLength",
                    "axisRange",
                    "requestedCoordinateValue",
                );
            }
            diagnostics.fail(
                &app,
                &operation,
                "Loading resolved section display (binary) failed",
                Some(build_fields(failure_fields)),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn load_gather_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    request: GatherRequest,
) -> Result<GatherView, String> {
    let operation = diagnostics.start_operation(
        &app,
        "load_gather",
        "Loading gather view",
        Some(build_fields([
            ("storePath", json_value(&store_path)),
            ("datasetId", json_value(&request.dataset_id.0)),
            ("stage", json_value("load_gather")),
        ])),
    );

    let result = load_gather(store_path, request);
    match result {
        Ok(gather) => {
            diagnostics.complete(
                &app,
                &operation,
                "Gather view loaded",
                Some(build_fields([
                    ("traces", json_value(gather.traces)),
                    ("samples", json_value(gather.samples)),
                    ("label", json_value(&gather.label)),
                ])),
            );
            Ok(gather)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Gather load failed",
                Some(build_fields([("error", json_value(&message))])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
async fn preview_processing_command(
    app: AppHandle,
    diagnostics: State<'_, DiagnosticsState>,
    preview_sessions: State<'_, PreviewSessionState>,
    request: PreviewTraceLocalProcessingRequest,
) -> Result<PreviewTraceLocalProcessingResponse, String> {
    let axis_name = format!("{:?}", request.section.axis).to_ascii_lowercase();
    let operator_ids = preview_processing_operation_ids(&request.pipeline);
    let operator_labels = preview_processing_operation_labels(&request.pipeline);
    let pipeline_name = request.pipeline.name.clone();
    let pipeline_revision = request.pipeline.revision;
    let dataset_id = request.section.dataset_id.0.clone();
    let operation = diagnostics.start_operation(
        &app,
        "preview_processing",
        "Generating processing preview",
        Some(build_fields([
            ("storePath", json_value(&request.store_path)),
            ("datasetId", json_value(&dataset_id)),
            ("axis", json_value(&axis_name)),
            ("index", json_value(request.section.index)),
            (
                "operatorCount",
                json_value(request.pipeline.operation_count()),
            ),
            ("pipelineRevision", json_value(pipeline_revision)),
            ("pipelineName", json_value(&pipeline_name)),
            ("operatorIds", json_value(&operator_ids)),
            ("operatorLabels", json_value(&operator_labels)),
            ("stage", json_value("preview_section")),
        ])),
    );

    diagnostics.verbose_progress(
        &app,
        &operation,
        "Dispatching processing preview to runtime session",
        Some(build_fields([
            ("datasetId", json_value(&dataset_id)),
            ("axis", json_value(&axis_name)),
            ("index", json_value(request.section.index)),
            ("operatorCount", json_value(operator_ids.len())),
            ("pipelineRevision", json_value(pipeline_revision)),
            ("operatorIds", json_value(&operator_ids)),
        ])),
    );

    let preview_sessions = preview_sessions.inner().clone();
    let request_for_compute = request;
    let compute_started = Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        preview_sessions.preview_processing(request_for_compute)
    })
    .await
    .map_err(|error| error.to_string())?;
    let compute_duration_ms = compute_started.elapsed().as_millis();

    match result {
        Ok((response, reuse)) => {
            diagnostics.complete(
                &app,
                &operation,
                "Processing preview ready",
                Some(build_fields([
                    ("pipelineRevision", json_value(pipeline_revision)),
                    ("pipelineName", json_value(&pipeline_name)),
                    ("operatorIds", json_value(&operator_ids)),
                    ("operatorLabels", json_value(&operator_labels)),
                    ("previewReady", json_value(response.preview.preview_ready)),
                    ("traces", json_value(response.preview.section.traces)),
                    ("samples", json_value(response.preview.section.samples)),
                    ("computeDurationMs", json_value(compute_duration_ms)),
                    ("cacheHit", json_value(reuse.cache_hit)),
                    (
                        "reusedPrefixOperations",
                        json_value(reuse.reused_prefix_operations),
                    ),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Processing preview failed",
                Some(build_fields([
                    ("pipelineRevision", json_value(pipeline_revision)),
                    ("pipelineName", json_value(&pipeline_name)),
                    ("operatorIds", json_value(&operator_ids)),
                    ("computeDurationMs", json_value(compute_duration_ms)),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
async fn preview_processing_binary_command(
    app: AppHandle,
    diagnostics: State<'_, DiagnosticsState>,
    preview_sessions: State<'_, PreviewSessionState>,
    request: PreviewTraceLocalProcessingRequest,
) -> Result<Response, String> {
    let axis_name = format!("{:?}", request.section.axis).to_ascii_lowercase();
    let operator_ids = preview_processing_operation_ids(&request.pipeline);
    let operator_labels = preview_processing_operation_labels(&request.pipeline);
    let pipeline_name = request.pipeline.name.clone();
    let pipeline_revision = request.pipeline.revision;
    let dataset_id = request.section.dataset_id.0.clone();
    let operation = diagnostics.start_operation(
        &app,
        "preview_processing_binary",
        "Generating processing preview (binary)",
        Some(build_fields([
            ("storePath", json_value(&request.store_path)),
            ("datasetId", json_value(&dataset_id)),
            ("axis", json_value(&axis_name)),
            ("index", json_value(request.section.index)),
            (
                "operatorCount",
                json_value(request.pipeline.operation_count()),
            ),
            ("pipelineRevision", json_value(pipeline_revision)),
            ("pipelineName", json_value(&pipeline_name)),
            ("operatorIds", json_value(&operator_ids)),
            ("operatorLabels", json_value(&operator_labels)),
            ("stage", json_value("preview_section_binary")),
        ])),
    );

    let preview_sessions = preview_sessions.inner().clone();
    let request_for_compute = request;
    let compute_started = Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        preview_sessions.preview_processing(request_for_compute)
    })
    .await
    .map_err(|error| error.to_string())?;
    let compute_duration_ms = compute_started.elapsed().as_millis();

    match result {
        Ok((response, reuse)) => {
            let traces = response.preview.section.traces;
            let samples = response.preview.section.samples;
            let packed = pack_preview_section_response(
                response.preview.preview_ready,
                response.preview.processing_label.clone(),
                response.preview.section,
            )?;
            diagnostics.complete(
                &app,
                &operation,
                "Processing preview ready (binary)",
                Some(build_fields([
                    ("pipelineRevision", json_value(pipeline_revision)),
                    ("pipelineName", json_value(&pipeline_name)),
                    ("operatorIds", json_value(&operator_ids)),
                    ("operatorLabels", json_value(&operator_labels)),
                    ("previewReady", json_value(true)),
                    ("traces", json_value(traces)),
                    ("samples", json_value(samples)),
                    ("computeDurationMs", json_value(compute_duration_ms)),
                    ("cacheHit", json_value(reuse.cache_hit)),
                    (
                        "reusedPrefixOperations",
                        json_value(reuse.reused_prefix_operations),
                    ),
                ])),
            );
            Ok(packed)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Processing preview failed (binary)",
                Some(build_fields([
                    ("pipelineRevision", json_value(pipeline_revision)),
                    ("pipelineName", json_value(&pipeline_name)),
                    ("operatorIds", json_value(&operator_ids)),
                    ("computeDurationMs", json_value(compute_duration_ms)),
                    ("error", json_value(&message)),
                ])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn preview_subvolume_processing_command(
    app: AppHandle,
    diagnostics: State<'_, DiagnosticsState>,
    request: PreviewSubvolumeProcessingRequest,
) -> Result<PreviewSubvolumeProcessingResponse, String> {
    let axis_name = format!("{:?}", request.section.axis).to_ascii_lowercase();
    let trace_local_count = request
        .pipeline
        .trace_local_pipeline
        .as_ref()
        .map(|pipeline| pipeline.operation_count())
        .unwrap_or(0);
    let source_handle = open_store(&request.store_path).ok();
    let mut start_fields = vec![
        ("storePath", json_value(&request.store_path)),
        ("datasetId", json_value(&request.section.dataset_id.0)),
        ("axis", json_value(&axis_name)),
        ("index", json_value(request.section.index)),
        ("traceLocalOperatorCount", json_value(trace_local_count)),
        ("inlineMin", json_value(request.pipeline.crop.inline_min)),
        ("inlineMax", json_value(request.pipeline.crop.inline_max)),
        ("xlineMin", json_value(request.pipeline.crop.xline_min)),
        ("xlineMax", json_value(request.pipeline.crop.xline_max)),
        ("zMinMs", json_value(request.pipeline.crop.z_min_ms)),
        ("zMaxMs", json_value(request.pipeline.crop.z_max_ms)),
        ("stage", json_value("preview_subvolume")),
    ];
    if let Some(handle) = source_handle.as_ref() {
        append_subvolume_preview_debug_fields(&mut start_fields, handle, &request);
    }
    let preview_debug_fields = start_fields.clone();
    let operation = diagnostics.start_operation(
        &app,
        "preview_subvolume_processing",
        "Generating cropped processing preview",
        Some(build_fields(start_fields)),
    );

    let result = preview_subvolume_processing(request);
    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Subvolume processing preview ready",
                Some(build_fields([
                    ("previewReady", json_value(response.preview.preview_ready)),
                    ("traces", json_value(response.preview.section.traces)),
                    ("samples", json_value(response.preview.section.samples)),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            let mut failure_fields = preview_debug_fields;
            failure_fields.push(("error", json_value(&message)));
            diagnostics.fail(
                &app,
                &operation,
                "Subvolume processing preview failed",
                Some(build_fields(failure_fields)),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn preview_subvolume_processing_binary_command(
    app: AppHandle,
    diagnostics: State<'_, DiagnosticsState>,
    request: PreviewSubvolumeProcessingRequest,
) -> Result<Response, String> {
    let axis_name = format!("{:?}", request.section.axis).to_ascii_lowercase();
    let trace_local_count = request
        .pipeline
        .trace_local_pipeline
        .as_ref()
        .map(|pipeline| pipeline.operation_count())
        .unwrap_or(0);
    let source_handle = open_store(&request.store_path).ok();
    let mut start_fields = vec![
        ("storePath", json_value(&request.store_path)),
        ("datasetId", json_value(&request.section.dataset_id.0)),
        ("axis", json_value(&axis_name)),
        ("index", json_value(request.section.index)),
        ("traceLocalOperatorCount", json_value(trace_local_count)),
        ("inlineMin", json_value(request.pipeline.crop.inline_min)),
        ("inlineMax", json_value(request.pipeline.crop.inline_max)),
        ("xlineMin", json_value(request.pipeline.crop.xline_min)),
        ("xlineMax", json_value(request.pipeline.crop.xline_max)),
        ("zMinMs", json_value(request.pipeline.crop.z_min_ms)),
        ("zMaxMs", json_value(request.pipeline.crop.z_max_ms)),
        ("stage", json_value("preview_subvolume_binary")),
    ];
    if let Some(handle) = source_handle.as_ref() {
        append_subvolume_preview_debug_fields(&mut start_fields, handle, &request);
    }
    let preview_debug_fields = start_fields.clone();
    let operation = diagnostics.start_operation(
        &app,
        "preview_subvolume_processing_binary",
        "Generating cropped processing preview (binary)",
        Some(build_fields(start_fields)),
    );

    let result = preview_subvolume_processing(request);
    match result {
        Ok(response) => {
            let traces = response.preview.section.traces;
            let samples = response.preview.section.samples;
            let packed = pack_preview_section_response(
                response.preview.preview_ready,
                response.preview.processing_label.clone(),
                response.preview.section,
            )?;
            diagnostics.complete(
                &app,
                &operation,
                "Subvolume processing preview ready (binary)",
                Some(build_fields([
                    ("previewReady", json_value(true)),
                    ("traces", json_value(traces)),
                    ("samples", json_value(samples)),
                ])),
            );
            Ok(packed)
        }
        Err(error) => {
            let message = error.to_string();
            let mut failure_fields = preview_debug_fields;
            failure_fields.push(("error", json_value(&message)));
            diagnostics.fail(
                &app,
                &operation,
                "Subvolume processing preview failed (binary)",
                Some(build_fields(failure_fields)),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn preview_gather_processing_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    request: PreviewGatherProcessingRequest,
) -> Result<PreviewGatherProcessingResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "preview_gather_processing",
        "Generating gather processing preview",
        Some(build_fields([
            ("storePath", json_value(&request.store_path)),
            ("datasetId", json_value(&request.gather.dataset_id.0)),
            (
                "operatorCount",
                json_value(request.pipeline.operations.len()),
            ),
            (
                "traceLocalOperatorCount",
                json_value(
                    request
                        .pipeline
                        .trace_local_pipeline
                        .as_ref()
                        .map(|pipeline| pipeline.operation_count())
                        .unwrap_or(0),
                ),
            ),
            ("stage", json_value("preview_gather")),
        ])),
    );

    let result = preview_gather_processing(request);
    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Gather processing preview ready",
                Some(build_fields([
                    ("previewReady", json_value(response.preview.preview_ready)),
                    ("traces", json_value(response.preview.gather.traces)),
                    ("samples", json_value(response.preview.gather.samples)),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Gather processing preview failed",
                Some(build_fields([("error", json_value(&message))])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn amplitude_spectrum_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    request: AmplitudeSpectrumRequest,
) -> Result<AmplitudeSpectrumResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "amplitude_spectrum",
        "Generating amplitude spectrum",
        Some(build_fields([
            ("storePath", json_value(&request.store_path)),
            (
                "axis",
                json_value(format!("{:?}", request.section.axis).to_ascii_lowercase()),
            ),
            ("index", json_value(request.section.index)),
            ("pipelineEnabled", json_value(request.pipeline.is_some())),
            ("stage", json_value("spectrum_analysis")),
        ])),
    );

    let result = amplitude_spectrum(request);
    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Amplitude spectrum ready",
                Some(build_fields([
                    ("bins", json_value(response.curve.frequencies_hz.len())),
                    ("sampleIntervalMs", json_value(response.sample_interval_ms)),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Amplitude spectrum failed",
                Some(build_fields([("error", json_value(&message))])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn velocity_scan_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    request: VelocityScanRequest,
) -> Result<VelocityScanResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "velocity_scan",
        "Running prestack velocity scan",
        Some(build_fields([
            ("storePath", json_value(&request.store_path)),
            ("datasetId", json_value(&request.gather.dataset_id.0)),
            ("minVelocity", json_value(request.min_velocity_m_per_s)),
            ("maxVelocity", json_value(request.max_velocity_m_per_s)),
            ("velocityStep", json_value(request.velocity_step_m_per_s)),
            ("autopickEnabled", json_value(request.autopick.is_some())),
            ("stage", json_value("velocity_scan")),
        ])),
    );

    let result = run_velocity_scan(request);
    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Velocity scan ready",
                Some(build_fields([
                    (
                        "velocityBins",
                        json_value(response.panel.velocities_m_per_s.len()),
                    ),
                    (
                        "sampleCount",
                        json_value(response.panel.sample_axis_ms.len()),
                    ),
                    (
                        "autopickCount",
                        json_value(
                            response
                                .autopicked_velocity_function
                                .as_ref()
                                .map(|estimate| estimate.times_ms.len())
                                .unwrap_or(0),
                        ),
                    ),
                ])),
            );
            Ok(response)
        }
        Err(error) => {
            let message = error.to_string();
            diagnostics.fail(
                &app,
                &operation,
                "Velocity scan failed",
                Some(build_fields([("error", json_value(&message))])),
            );
            Err(message)
        }
    }
}

#[tauri::command]
fn run_processing_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    processing: State<ProcessingState>,
    processing_cache: State<ProcessingCacheState>,
    request: RunTraceLocalProcessingRequest,
) -> Result<RunTraceLocalProcessingResponse, String> {
    let pipeline_spec = seis_runtime::ProcessingPipelineSpec::TraceLocal {
        pipeline: request.pipeline.clone(),
    };
    let allow_exact_reuse = processing_cache.enabled()
        && request.output_store_path.is_none()
        && request.pipeline.checkpoint_indexes().is_empty();

    if allow_exact_reuse {
        let source_fingerprint = trace_local_source_fingerprint(&request.store_path)?;
        let full_pipeline_hash = trace_local_pipeline_hash(&request.pipeline)?;
        if let Some(hit) = processing_cache.lookup_exact_visible_output(
            TRACE_LOCAL_CACHE_FAMILY,
            &source_fingerprint,
            &full_pipeline_hash,
        )? {
            let final_artifact = ProcessingJobArtifact {
                kind: ProcessingJobArtifactKind::FinalOutput,
                step_index: request.pipeline.operation_count().saturating_sub(1),
                label: "Exact output reuse".to_string(),
                store_path: hit.path.clone(),
            };
            let reused = processing.enqueue_completed_job(
                request.store_path.clone(),
                hit.path.clone(),
                pipeline_spec.clone(),
                vec![final_artifact],
            );
            diagnostics.emit_session_event(
                &app,
                "processing_job_reused",
                log::Level::Info,
                "Processing job reused an existing derived output",
                Some(build_fields([
                    ("jobId", json_value(&reused.job_id)),
                    ("storePath", json_value(&request.store_path)),
                    ("outputStorePath", json_value(&hit.path)),
                ])),
            );
            return Ok(RunTraceLocalProcessingResponse {
                schema_version: IPC_SCHEMA_VERSION,
                job: reused,
            });
        }
    }

    let app_paths = AppPaths::resolve(&app)?;
    let output_store_path =
        request
            .output_store_path
            .clone()
            .unwrap_or(default_processing_store_path(
                &app_paths,
                &request.store_path,
                &request.pipeline,
            )?);
    let queued = processing.enqueue_job(
        request.store_path.clone(),
        Some(output_store_path.clone()),
        pipeline_spec,
    );
    let job_id = queued.job_id.clone();
    let record = processing.job_record(&job_id)?;

    diagnostics.emit_session_event(
        &app,
        "processing_job_queued",
        log::Level::Info,
        "Processing job queued",
        Some(build_fields([
            ("jobId", json_value(&job_id)),
            ("storePath", json_value(&request.store_path)),
            ("outputStorePath", json_value(&output_store_path)),
            (
                "operatorCount",
                json_value(request.pipeline.operation_count()),
            ),
        ])),
    );

    let worker_app = app.clone();
    let worker_request = RunTraceLocalProcessingRequest {
        output_store_path: Some(output_store_path.clone()),
        ..request
    };
    std::thread::spawn(move || {
        run_processing_job(&worker_app, &record, worker_request);
    });

    Ok(RunTraceLocalProcessingResponse {
        schema_version: IPC_SCHEMA_VERSION,
        job: queued,
    })
}

#[tauri::command]
fn run_subvolume_processing_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    processing: State<ProcessingState>,
    request: RunSubvolumeProcessingRequest,
) -> Result<RunSubvolumeProcessingResponse, String> {
    let app_paths = AppPaths::resolve(&app)?;
    let output_store_path =
        request
            .output_store_path
            .clone()
            .unwrap_or(default_subvolume_processing_store_path(
                &app_paths,
                &request.store_path,
                &request.pipeline,
            )?);
    let queued = processing.enqueue_job(
        request.store_path.clone(),
        Some(output_store_path.clone()),
        seis_runtime::ProcessingPipelineSpec::Subvolume {
            pipeline: request.pipeline.clone(),
        },
    );
    let job_id = queued.job_id.clone();
    let record = processing.job_record(&job_id)?;

    diagnostics.emit_session_event(
        &app,
        "subvolume_processing_job_queued",
        log::Level::Info,
        "Subvolume processing job queued",
        Some(build_fields([
            ("jobId", json_value(&job_id)),
            ("storePath", json_value(&request.store_path)),
            ("outputStorePath", json_value(&output_store_path)),
            (
                "traceLocalOperatorCount",
                json_value(
                    request
                        .pipeline
                        .trace_local_pipeline
                        .as_ref()
                        .map(|pipeline| pipeline.operation_count())
                        .unwrap_or(0),
                ),
            ),
        ])),
    );

    let worker_app = app.clone();
    let worker_request = RunSubvolumeProcessingRequest {
        output_store_path: Some(output_store_path.clone()),
        ..request
    };
    std::thread::spawn(move || {
        run_subvolume_processing_job(&worker_app, &record, worker_request);
    });

    Ok(RunSubvolumeProcessingResponse {
        schema_version: IPC_SCHEMA_VERSION,
        job: queued,
    })
}

#[tauri::command]
fn run_gather_processing_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    processing: State<ProcessingState>,
    request: RunGatherProcessingRequest,
) -> Result<RunGatherProcessingResponse, String> {
    let app_paths = AppPaths::resolve(&app)?;
    let output_store_path =
        request
            .output_store_path
            .clone()
            .unwrap_or(default_gather_processing_store_path(
                &app_paths,
                &request.store_path,
                &request.pipeline,
            )?);
    let queued = processing.enqueue_job(
        request.store_path.clone(),
        Some(output_store_path.clone()),
        seis_runtime::ProcessingPipelineSpec::Gather {
            pipeline: request.pipeline.clone(),
        },
    );
    let job_id = queued.job_id.clone();
    let record = processing.job_record(&job_id)?;

    diagnostics.emit_session_event(
        &app,
        "gather_processing_job_queued",
        log::Level::Info,
        "Gather processing job queued",
        Some(build_fields([
            ("jobId", json_value(&job_id)),
            ("storePath", json_value(&request.store_path)),
            ("outputStorePath", json_value(&output_store_path)),
            (
                "operatorCount",
                json_value(request.pipeline.operations.len()),
            ),
        ])),
    );

    let worker_app = app.clone();
    let worker_request = RunGatherProcessingRequest {
        output_store_path: Some(output_store_path.clone()),
        ..request
    };
    std::thread::spawn(move || {
        run_gather_processing_job(&worker_app, &record, worker_request);
    });

    Ok(RunGatherProcessingResponse {
        schema_version: IPC_SCHEMA_VERSION,
        job: queued,
    })
}

#[tauri::command]
fn get_processing_job_command(
    processing: State<ProcessingState>,
    request: GetProcessingJobRequest,
) -> Result<GetProcessingJobResponse, String> {
    Ok(GetProcessingJobResponse {
        schema_version: IPC_SCHEMA_VERSION,
        job: processing.job_status(&request.job_id)?,
    })
}

#[tauri::command]
fn cancel_processing_job_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    processing: State<ProcessingState>,
    request: CancelProcessingJobRequest,
) -> Result<CancelProcessingJobResponse, String> {
    let job = processing.cancel_job(&request.job_id)?;
    diagnostics.emit_session_event(
        &app,
        "processing_job_cancel_requested",
        log::Level::Warn,
        "Processing job cancellation requested",
        Some(build_fields([("jobId", json_value(&request.job_id))])),
    );
    Ok(CancelProcessingJobResponse {
        schema_version: IPC_SCHEMA_VERSION,
        job,
    })
}

#[tauri::command]
fn list_pipeline_presets_command(
    processing: State<ProcessingState>,
) -> Result<ListPipelinePresetsResponse, String> {
    Ok(ListPipelinePresetsResponse {
        schema_version: IPC_SCHEMA_VERSION,
        presets: processing.list_presets()?,
    })
}

#[tauri::command]
fn save_pipeline_preset_command(
    processing: State<ProcessingState>,
    request: SavePipelinePresetRequest,
) -> Result<SavePipelinePresetResponse, String> {
    Ok(SavePipelinePresetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        preset: processing.save_preset(request.preset)?,
    })
}

#[tauri::command]
fn delete_pipeline_preset_command(
    processing: State<ProcessingState>,
    request: DeletePipelinePresetRequest,
) -> Result<DeletePipelinePresetResponse, String> {
    Ok(DeletePipelinePresetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        deleted: processing.delete_preset(&request.preset_id)?,
    })
}

#[tauri::command]
fn load_workspace_state_command(
    workspace: State<WorkspaceState>,
) -> Result<LoadWorkspaceStateResponse, String> {
    workspace.load_state()
}

#[tauri::command]
fn upsert_dataset_entry_command(
    workspace: State<WorkspaceState>,
    request: UpsertDatasetEntryRequest,
) -> Result<UpsertDatasetEntryResponse, String> {
    workspace.upsert_entry(request)
}

#[tauri::command]
fn remove_dataset_entry_command(
    workspace: State<WorkspaceState>,
    request: RemoveDatasetEntryRequest,
) -> Result<RemoveDatasetEntryResponse, String> {
    workspace.remove_entry(request)
}

#[tauri::command]
fn set_active_dataset_entry_command(
    workspace: State<WorkspaceState>,
    request: SetActiveDatasetEntryRequest,
) -> Result<SetActiveDatasetEntryResponse, String> {
    workspace.set_active_entry(request)
}

#[tauri::command]
fn save_workspace_session_command(
    workspace: State<WorkspaceState>,
    request: SaveWorkspaceSessionRequest,
) -> Result<SaveWorkspaceSessionResponse, String> {
    workspace.save_session(request)
}

#[tauri::command]
fn set_dataset_native_coordinate_reference_command(
    request: SetDatasetNativeCoordinateReferenceRequest,
) -> Result<SetDatasetNativeCoordinateReferenceResponse, String> {
    set_any_store_native_coordinate_reference(
        &request.store_path,
        request.coordinate_reference_id.as_deref(),
        request.coordinate_reference_name.as_deref(),
    )
    .map_err(|error| error.to_string())?;
    let response = open_dataset_summary(OpenDatasetRequest {
        schema_version: IPC_SCHEMA_VERSION,
        store_path: request.store_path,
    })
    .map_err(|error| error.to_string())?;
    Ok(SetDatasetNativeCoordinateReferenceResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: response.dataset,
    })
}

#[tauri::command]
fn resolve_survey_map_command(
    app: AppHandle,
    request: ResolveSurveyMapRequest,
) -> Result<ResolveSurveyMapResponse, String> {
    let app_paths = AppPaths::resolve(&app)?;
    let store_path = request.store_path.clone();
    let dataset = open_dataset_summary(OpenDatasetRequest {
        schema_version: IPC_SCHEMA_VERSION,
        store_path,
    })
    .map_err(|error| error.to_string())?
    .dataset;
    let survey_map = resolve_dataset_summary_survey_map_source(
        &dataset,
        request.display_coordinate_reference_id.as_deref(),
        Some(app_paths.map_transform_cache_dir()),
        Some(Path::new(&request.store_path)),
    )
    .map_err(|error| error.to_string())?;
    Ok(ResolveSurveyMapResponse {
        schema_version: IPC_SCHEMA_VERSION,
        survey_map,
    })
}

#[tauri::command]
fn list_project_well_time_depth_models_command(
    request: ProjectWellboreRequest,
) -> Result<Vec<ProjectWellTimeDepthModelDescriptor>, String> {
    let project = OphioliteProject::open(Path::new(&request.project_root))
        .map_err(|error| error.to_string())?;
    let active_asset_id =
        project_active_well_time_depth_model_asset_id(&project, &request.wellbore_id)?;
    project_well_time_depth_model_descriptors(
        &project,
        &request.wellbore_id,
        active_asset_id.as_deref(),
    )
}

#[tauri::command]
fn list_project_well_time_depth_inventory_command(
    request: ProjectWellboreRequest,
) -> Result<ProjectWellTimeDepthInventoryResponse, String> {
    let project = OphioliteProject::open(Path::new(&request.project_root))
        .map_err(|error| error.to_string())?;
    let active_asset_id =
        project_active_well_time_depth_model_asset_id(&project, &request.wellbore_id)?;
    Ok(ProjectWellTimeDepthInventoryResponse {
        observation_sets: project_well_time_depth_observation_descriptors(
            &project,
            &request.wellbore_id,
        )?,
        authored_models: project_well_time_depth_authored_model_descriptors(
            &project,
            &request.wellbore_id,
        )?,
        compiled_models: project_well_time_depth_model_descriptors(
            &project,
            &request.wellbore_id,
            active_asset_id.as_deref(),
        )?,
    })
}

#[tauri::command]
fn list_project_well_overlay_inventory_command(
    request: ProjectRootRequest,
) -> Result<ProjectWellOverlayInventoryResponse, String> {
    let project = OphioliteProject::open(Path::new(&request.project_root))
        .map_err(|error| error.to_string())?;
    let inventory = project
        .project_well_overlay_inventory()
        .map_err(|error| error.to_string())?;
    Ok(ProjectWellOverlayInventoryResponse {
        surveys: inventory
            .surveys
            .into_iter()
            .map(|survey| ProjectSurveyAssetDescriptor {
                asset_id: survey.asset_id.0,
                name: survey.name,
                status: asset_status_label(&survey.status).to_string(),
                well_id: survey.well_id.0,
                well_name: survey.well_name,
                wellbore_id: survey.wellbore_id.0,
                wellbore_name: survey.wellbore_name,
            })
            .collect(),
        wellbores: inventory
            .wellbores
            .into_iter()
            .map(|wellbore| ProjectWellboreInventoryItem {
                well_id: wellbore.well_id.0,
                well_name: wellbore.well_name,
                wellbore_id: wellbore.wellbore_id.0,
                wellbore_name: wellbore.wellbore_name,
                trajectory_asset_count: wellbore.trajectory_asset_count,
                well_time_depth_model_count: wellbore.well_time_depth_model_count,
                active_well_time_depth_model_asset_id: wellbore
                    .active_well_time_depth_model_asset_id
                    .map(|asset_id| asset_id.0),
            })
            .collect(),
    })
}

#[tauri::command]
fn set_project_active_well_time_depth_model_command(
    request: SetProjectWellTimeDepthModelRequest,
) -> Result<(), String> {
    let project = OphioliteProject::open(Path::new(&request.project_root))
        .map_err(|error| error.to_string())?;
    project
        .set_active_well_time_depth_model(
            &ophiolite::WellboreId(request.wellbore_id),
            request
                .asset_id
                .as_ref()
                .map(|value| ophiolite::AssetId(value.clone()))
                .as_ref(),
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn import_project_well_time_depth_model_command(
    request: ImportProjectWellTimeDepthModelRequest,
) -> Result<ImportProjectWellTimeDepthModelResponse, String> {
    let mut project = OphioliteProject::open(Path::new(&request.project_root))
        .map_err(|error| error.to_string())?;
    let result = project
        .import_well_time_depth_model_json(
            Path::new(&request.json_path),
            request.binding,
            request.collection_name.as_deref(),
        )
        .map_err(|error| error.to_string())?;

    Ok(well_time_depth_import_response(result))
}

#[tauri::command]
fn import_project_well_time_depth_asset_command(
    request: ImportProjectWellTimeDepthAssetRequest,
) -> Result<ImportProjectWellTimeDepthModelResponse, String> {
    let mut project = OphioliteProject::open(Path::new(&request.project_root))
        .map_err(|error| error.to_string())?;
    let result = match request.asset_kind.as_str() {
        "checkshot_vsp_observation_set" => project.import_checkshot_vsp_observation_set_json(
            Path::new(&request.json_path),
            request.binding,
            request.collection_name.as_deref(),
        ),
        "manual_time_depth_pick_set" => project.import_manual_time_depth_pick_set_json(
            Path::new(&request.json_path),
            request.binding,
            request.collection_name.as_deref(),
        ),
        "well_time_depth_authored_model" => project.import_well_time_depth_authored_model_json(
            Path::new(&request.json_path),
            request.binding,
            request.collection_name.as_deref(),
        ),
        "well_time_depth_model" => project.import_well_time_depth_model_json(
            Path::new(&request.json_path),
            request.binding,
            request.collection_name.as_deref(),
        ),
        other => return Err(format!("unsupported well time-depth asset kind '{other}'")),
    }
    .map_err(|error| error.to_string())?;

    Ok(well_time_depth_import_response(result))
}

#[tauri::command]
fn compile_project_well_time_depth_authored_model_command(
    request: CompileProjectWellTimeDepthAuthoredModelRequest,
) -> Result<ImportProjectWellTimeDepthModelResponse, String> {
    let mut project = OphioliteProject::open(Path::new(&request.project_root))
        .map_err(|error| error.to_string())?;
    let result = project
        .compile_well_time_depth_authored_model_to_asset(
            &ophiolite::AssetId(request.asset_id),
            request.output_collection_name.as_deref(),
            request.set_active,
        )
        .map_err(|error| error.to_string())?;
    Ok(well_time_depth_import_response(result))
}

#[tauri::command]
fn read_project_well_time_depth_model_command(
    request: ProjectAssetRequest,
) -> Result<WellTimeDepthModel1D, String> {
    let project = OphioliteProject::open(Path::new(&request.project_root))
        .map_err(|error| error.to_string())?;
    project
        .read_well_time_depth_model(&ophiolite::AssetId(request.asset_id))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn resolve_project_section_well_overlays_command(
    request: SectionWellOverlayRequestDto,
) -> Result<ResolveSectionWellOverlaysResponse, String> {
    let project = OphioliteProject::open(Path::new(&request.project_root))
        .map_err(|error| error.to_string())?;
    project
        .resolve_section_well_overlays(&request)
        .map_err(|error| error.to_string())
}

fn run_processing_job(
    app: &AppHandle,
    record: &JobRecord,
    request: RunTraceLocalProcessingRequest,
) {
    let app_paths = match AppPaths::resolve(app) {
        Ok(paths) => paths,
        Err(error) => {
            let _ = record.mark_failed(error.clone());
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "processing_job_failed",
                    log::Level::Error,
                    "Processing job failed before initialization",
                    Some(build_fields([("error", json_value(&error))])),
                );
            }
            return;
        }
    };
    let output_store_path = request.output_store_path.clone().unwrap_or_else(|| {
        default_processing_store_path(&app_paths, &request.store_path, &request.pipeline)
            .unwrap_or_else(|_| "derived-output.tbvol".to_string())
    });
    let job_id = record.snapshot().job_id;
    let reused_checkpoint = match app.try_state::<ProcessingCacheState>() {
        Some(processing_cache) => {
            match resolve_reused_trace_local_checkpoint(&processing_cache, &request, false) {
                Ok(value) => value,
                Err(error) => {
                    if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                        diagnostics.emit_session_event(
                            app,
                            "processing_checkpoint_reuse_failed",
                            log::Level::Warn,
                            "Checkpoint reuse lookup failed; processing will continue from source",
                            Some(build_fields([
                                ("jobId", json_value(&job_id)),
                                ("error", json_value(error)),
                            ])),
                        );
                    }
                    None
                }
            }
        }
        None => None,
    };
    let source_fingerprint = match app.try_state::<ProcessingCacheState>() {
        Some(processing_cache) if processing_cache.enabled() => {
            match trace_local_source_fingerprint(&request.store_path) {
                Ok(value) => Some(value),
                Err(error) => {
                    if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                        diagnostics.emit_session_event(
                            app,
                            "processing_cache_fingerprint_failed",
                            log::Level::Warn,
                            "Processing cache fingerprinting failed; processing will continue without prefix registration",
                            Some(build_fields([
                                ("jobId", json_value(&job_id)),
                                ("error", json_value(error)),
                            ])),
                        );
                    }
                    None
                }
            }
        }
        _ => None,
    };
    let stages = match build_trace_local_processing_stages_from(
        &request,
        &output_store_path,
        &job_id,
        reused_checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.after_operation_index + 1)
            .unwrap_or(0),
    ) {
        Ok(stages) => stages,
        Err(error) => {
            let final_status = record.mark_failed(error);
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "processing_job_failed",
                    log::Level::Error,
                    "Processing job failed",
                    Some(build_fields([
                        ("jobId", json_value(&final_status.job_id)),
                        (
                            "error",
                            json_value(final_status.error_message.clone().unwrap_or_default()),
                        ),
                    ])),
                );
            }
            return;
        }
    };
    let job_started_at = Instant::now();
    let _ = record.mark_running(stages.first().map(|stage| stage.stage_label.clone()));
    if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
        diagnostics.emit_session_event(
            app,
            "processing_job_started",
            log::Level::Info,
            "Processing job started",
            Some(build_fields([
                ("jobId", json_value(&job_id)),
                ("storePath", json_value(&request.store_path)),
                ("outputStorePath", json_value(&output_store_path)),
                ("stageCount", json_value(stages.len())),
                (
                    "operatorCount",
                    json_value(request.pipeline.operation_count()),
                ),
                ("reusedCheckpoint", json_value(reused_checkpoint.is_some())),
            ])),
        );
    }
    if let Err(error) = prepare_processing_output_store(
        &request.store_path,
        &output_store_path,
        request.overwrite_existing,
    ) {
        let final_status = record.mark_failed(error);
        if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
            diagnostics.emit_session_event(
                app,
                "processing_job_failed",
                log::Level::Error,
                "Processing job failed",
                Some(build_fields([
                    ("jobId", json_value(&final_status.job_id)),
                    (
                        "error",
                        json_value(final_status.error_message.clone().unwrap_or_default()),
                    ),
                ])),
            );
        }
        return;
    }
    if let Some(reused_checkpoint) = reused_checkpoint.as_ref() {
        let _ = record.push_artifact(reused_checkpoint.artifact.clone());
        if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
            diagnostics.emit_session_event(
                app,
                "processing_job_checkpoint_reused",
                log::Level::Info,
                "Processing job reused a cached checkpoint",
                Some(build_fields([
                    ("jobId", json_value(&job_id)),
                    ("storePath", json_value(&reused_checkpoint.path)),
                    (
                        "afterOperationIndex",
                        json_value(reused_checkpoint.after_operation_index),
                    ),
                ])),
            );
        }
    }
    let mut current_input_store_path = reused_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.path.clone())
        .unwrap_or_else(|| request.store_path.clone());
    let result = stages.iter().try_for_each(|stage| {
        let stage_started_at = Instant::now();
        if record.cancel_requested() {
            return Err(seis_runtime::SeisRefineError::Message(
                "processing cancelled".to_string(),
            ));
        }
        if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
            diagnostics.emit_session_event(
                app,
                "processing_job_stage_started",
                log::Level::Info,
                "Processing stage started",
                Some(build_fields([
                    ("jobId", json_value(&job_id)),
                    ("label", json_value(&stage.stage_label)),
                    ("inputStorePath", json_value(&current_input_store_path)),
                    ("outputStorePath", json_value(&stage.artifact.store_path)),
                    (
                        "artifactKind",
                        json_value(match stage.artifact.kind {
                            ProcessingJobArtifactKind::Checkpoint => "checkpoint",
                            ProcessingJobArtifactKind::FinalOutput => "final_output",
                        }),
                    ),
                    (
                        "segmentOperatorCount",
                        json_value(stage.segment_pipeline.operation_count()),
                    ),
                    (
                        "lineageOperatorCount",
                        json_value(stage.lineage_pipeline.operation_count()),
                    ),
                ])),
            );
        }
        if !matches!(stage.artifact.kind, ProcessingJobArtifactKind::FinalOutput) {
            prepare_processing_output_store(
                &current_input_store_path,
                &stage.artifact.store_path,
                false,
            )
            .map_err(seis_runtime::SeisRefineError::Message)?;
        }
        let materialize_options = materialize_options_for_store(&current_input_store_path)
            .map_err(seis_runtime::SeisRefineError::Message)?;
        let materialize_started_at = Instant::now();
        materialize_processing_volume_with_progress(
            &current_input_store_path,
            &stage.artifact.store_path,
            &stage.segment_pipeline,
            materialize_options,
            |completed, total| {
                if record.cancel_requested() {
                    return Err(seis_runtime::SeisRefineError::Message(
                        "processing cancelled".to_string(),
                    ));
                }
                let _ = record.mark_progress(completed, total, Some(&stage.stage_label));
                Ok(())
            },
        )?;
        let materialize_duration_ms = materialize_started_at.elapsed().as_millis() as u64;
        let lineage_rewrite_started_at = Instant::now();
        rewrite_trace_local_processing_lineage(
            &stage.artifact.store_path,
            &stage.lineage_pipeline,
            stage.artifact.kind,
        )
        .map_err(seis_runtime::SeisRefineError::Message)?;
        let lineage_rewrite_duration_ms =
            lineage_rewrite_started_at.elapsed().as_millis() as u64;
        let _ = record.push_artifact(stage.artifact.clone());
        let artifact_register_started_at = Instant::now();
        if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
            diagnostics.emit_session_event(
                app,
                if matches!(stage.artifact.kind, ProcessingJobArtifactKind::FinalOutput) {
                    "processing_job_output_emitted"
                } else {
                    "processing_job_checkpoint_emitted"
                },
                log::Level::Info,
                if matches!(stage.artifact.kind, ProcessingJobArtifactKind::FinalOutput) {
                    "Processing output emitted"
                } else {
                    "Processing checkpoint emitted"
                },
                Some(build_fields([
                    ("jobId", json_value(&job_id)),
                    ("storePath", json_value(&stage.artifact.store_path)),
                    ("label", json_value(&stage.artifact.label)),
                    (
                        "artifactKind",
                        json_value(match stage.artifact.kind {
                            ProcessingJobArtifactKind::Checkpoint => "checkpoint",
                            ProcessingJobArtifactKind::FinalOutput => "final_output",
                        }),
                    ),
                ])),
            );
        }
        if let Err(error) = register_processing_store_artifact(app, &request.store_path, &stage.artifact)
        {
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "processing_job_artifact_register_failed",
                    log::Level::Warn,
                    "Processing output emitted but workspace registration failed",
                    Some(build_fields([
                        ("jobId", json_value(&job_id)),
                        ("storePath", json_value(&stage.artifact.store_path)),
                        ("error", json_value(error)),
                    ])),
                );
            }
        }
        let artifact_register_duration_ms =
            artifact_register_started_at.elapsed().as_millis() as u64;
        if matches!(stage.artifact.kind, ProcessingJobArtifactKind::Checkpoint) {
            if let (Some(processing_cache), Some(source_fingerprint)) =
                (app.try_state::<ProcessingCacheState>(), source_fingerprint.as_ref())
            {
                if processing_cache.enabled() {
                    match trace_local_pipeline_hash(&stage.lineage_pipeline) {
                        Ok(prefix_hash) => {
                            if let Err(error) = processing_cache.register_visible_checkpoint(
                                TRACE_LOCAL_CACHE_FAMILY,
                                &stage.artifact.store_path,
                                source_fingerprint,
                                &prefix_hash,
                                stage.artifact.step_index + 1,
                                PROCESSING_CACHE_RUNTIME_VERSION,
                                TBVOL_STORE_FORMAT_VERSION,
                            ) {
                                if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                                    diagnostics.emit_session_event(
                                        app,
                                        "processing_cache_checkpoint_register_failed",
                                        log::Level::Warn,
                                        "Processing checkpoint emitted but cache registration failed",
                                        Some(build_fields([
                                            ("jobId", json_value(&job_id)),
                                            ("storePath", json_value(&stage.artifact.store_path)),
                                            ("error", json_value(error)),
                                        ])),
                                    );
                                }
                            }
                        }
                        Err(error) => {
                            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                                diagnostics.emit_session_event(
                                    app,
                                    "processing_cache_checkpoint_hash_failed",
                                    log::Level::Warn,
                                    "Processing checkpoint emitted but cache hashing failed",
                                    Some(build_fields([
                                        ("jobId", json_value(&job_id)),
                                        ("storePath", json_value(&stage.artifact.store_path)),
                                        ("error", json_value(error)),
                                    ])),
                                );
                            }
                        }
                    }
                }
            }
        }
        if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
            diagnostics.emit_session_event(
                app,
                "processing_job_stage_completed",
                log::Level::Info,
                "Processing stage completed",
                Some(build_fields([
                    ("jobId", json_value(&job_id)),
                    ("label", json_value(&stage.stage_label)),
                    ("storePath", json_value(&stage.artifact.store_path)),
                    (
                        "artifactKind",
                        json_value(match stage.artifact.kind {
                            ProcessingJobArtifactKind::Checkpoint => "checkpoint",
                            ProcessingJobArtifactKind::FinalOutput => "final_output",
                        }),
                    ),
                    (
                        "stageDurationMs",
                        json_value(stage_started_at.elapsed().as_millis() as u64),
                    ),
                    ("materializeDurationMs", json_value(materialize_duration_ms)),
                    (
                        "lineageRewriteDurationMs",
                        json_value(lineage_rewrite_duration_ms),
                    ),
                    (
                        "artifactRegisterDurationMs",
                        json_value(artifact_register_duration_ms),
                    ),
                ])),
            );
        }
        current_input_store_path = stage.artifact.store_path.clone();
        Ok(())
    });

    match result {
        Ok(_) => {
            if let Some(processing_cache) = app.try_state::<ProcessingCacheState>() {
                if processing_cache.enabled() {
                    match (
                        source_fingerprint.as_ref(),
                        trace_local_pipeline_hash(&request.pipeline),
                    ) {
                        (Some(source_fingerprint), Ok(full_pipeline_hash)) => {
                            if let Err(error) = processing_cache.register_visible_output(
                                TRACE_LOCAL_CACHE_FAMILY,
                                &output_store_path,
                                source_fingerprint,
                                &full_pipeline_hash,
                                &full_pipeline_hash,
                                request.pipeline.operation_count(),
                                PROCESSING_CACHE_RUNTIME_VERSION,
                                TBVOL_STORE_FORMAT_VERSION,
                            ) {
                                if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                                    diagnostics.emit_session_event(
                                        app,
                                        "processing_cache_register_failed",
                                        log::Level::Warn,
                                        "Processing output completed but cache registration failed",
                                        Some(build_fields([
                                            ("jobId", json_value(&job_id)),
                                            ("outputStorePath", json_value(&output_store_path)),
                                            ("error", json_value(error)),
                                        ])),
                                    );
                                }
                            }
                        }
                        (_, Err(error)) => {
                            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                                diagnostics.emit_session_event(
                                    app,
                                    "processing_cache_fingerprint_failed",
                                    log::Level::Warn,
                                    "Processing output completed but cache fingerprinting failed",
                                    Some(build_fields([
                                        ("jobId", json_value(&job_id)),
                                        ("outputStorePath", json_value(&output_store_path)),
                                        ("error", json_value(error)),
                                    ])),
                                );
                            }
                        }
                        (None, _) => {}
                    }
                }
            }
            let final_status = record.mark_completed(output_store_path.clone());
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "processing_job_completed",
                    log::Level::Info,
                    "Processing job completed",
                    Some(build_fields([
                        ("jobId", json_value(&final_status.job_id)),
                        ("outputStorePath", json_value(&output_store_path)),
                        (
                            "jobDurationMs",
                            json_value(job_started_at.elapsed().as_millis() as u64),
                        ),
                    ])),
                );
            }
        }
        Err(error) => {
            let final_status = if record.cancel_requested() {
                record.mark_cancelled()
            } else {
                record.mark_failed(error.to_string())
            };
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "processing_job_failed",
                    if matches!(
                        final_status.state,
                        seis_runtime::ProcessingJobState::Cancelled
                    ) {
                        log::Level::Warn
                    } else {
                        log::Level::Error
                    },
                    if matches!(
                        final_status.state,
                        seis_runtime::ProcessingJobState::Cancelled
                    ) {
                        "Processing job cancelled"
                    } else {
                        "Processing job failed"
                    },
                    Some(build_fields([
                        ("jobId", json_value(&final_status.job_id)),
                        (
                            "jobDurationMs",
                            json_value(job_started_at.elapsed().as_millis() as u64),
                        ),
                        (
                            "state",
                            json_value(format!("{:?}", final_status.state).to_ascii_lowercase()),
                        ),
                        (
                            "error",
                            json_value(final_status.error_message.clone().unwrap_or_default()),
                        ),
                    ])),
                );
            }
        }
    }
}

fn run_subvolume_processing_job(
    app: &AppHandle,
    record: &JobRecord,
    request: RunSubvolumeProcessingRequest,
) {
    let app_paths = match AppPaths::resolve(app) {
        Ok(paths) => paths,
        Err(error) => {
            let _ = record.mark_failed(error.clone());
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "subvolume_processing_job_failed",
                    log::Level::Error,
                    "Subvolume processing job failed before initialization",
                    Some(build_fields([("error", json_value(&error))])),
                );
            }
            return;
        }
    };

    let output_store_path = request.output_store_path.clone().unwrap_or_else(|| {
        default_subvolume_processing_store_path(&app_paths, &request.store_path, &request.pipeline)
            .unwrap_or_else(|_| "derived-output.tbvol".to_string())
    });
    let job_started_at = Instant::now();
    let job_id = record.snapshot().job_id;
    let prefix_request = request
        .pipeline
        .trace_local_pipeline
        .as_ref()
        .map(|pipeline| RunTraceLocalProcessingRequest {
            schema_version: IPC_SCHEMA_VERSION,
            store_path: request.store_path.clone(),
            output_store_path: None,
            overwrite_existing: false,
            pipeline: pipeline.clone(),
        });
    let reused_checkpoint = match (
        app.try_state::<ProcessingCacheState>(),
        prefix_request.as_ref(),
    ) {
        (Some(processing_cache), Some(prefix_request)) => {
            match resolve_reused_trace_local_checkpoint(&processing_cache, prefix_request, true) {
                Ok(value) => value,
                Err(error) => {
                    if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                        diagnostics.emit_session_event(
                            app,
                            "subvolume_processing_checkpoint_reuse_failed",
                            log::Level::Warn,
                            "Checkpoint reuse lookup failed; subvolume processing will continue from source",
                            Some(build_fields([
                                ("jobId", json_value(&job_id)),
                                ("error", json_value(error)),
                            ])),
                        );
                    }
                    None
                }
            }
        }
        _ => None,
    };
    let source_fingerprint = match (
        app.try_state::<ProcessingCacheState>(),
        request.pipeline.trace_local_pipeline.as_ref(),
    ) {
        (Some(processing_cache), Some(_)) if processing_cache.enabled() => {
            match trace_local_source_fingerprint(&request.store_path) {
                Ok(value) => Some(value),
                Err(error) => {
                    if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                        diagnostics.emit_session_event(
                            app,
                            "subvolume_processing_cache_fingerprint_failed",
                            log::Level::Warn,
                            "Processing cache fingerprinting failed; subvolume processing will continue without prefix registration",
                            Some(build_fields([
                                ("jobId", json_value(&job_id)),
                                ("error", json_value(error)),
                            ])),
                        );
                    }
                    None
                }
            }
        }
        _ => None,
    };
    let checkpoint_stages = match request.pipeline.trace_local_pipeline.as_ref() {
        Some(pipeline) => match build_trace_local_checkpoint_stages_from_pipeline(
            pipeline,
            &output_store_path,
            &job_id,
            reused_checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.after_operation_index + 1)
                .unwrap_or(0),
            true,
        ) {
            Ok(stages) => stages,
            Err(error) => {
                let final_status = record.mark_failed(error);
                if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                    diagnostics.emit_session_event(
                        app,
                        "subvolume_processing_job_failed",
                        log::Level::Error,
                        "Subvolume processing job failed",
                        Some(build_fields([
                            ("jobId", json_value(&final_status.job_id)),
                            (
                                "error",
                                json_value(final_status.error_message.clone().unwrap_or_default()),
                            ),
                        ])),
                    );
                }
                return;
            }
        },
        None => Vec::new(),
    };
    let _ = record.mark_running(
        checkpoint_stages
            .first()
            .map(|stage| stage.stage_label.clone())
            .or_else(|| Some("Crop Subvolume".to_string())),
    );
    if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
        diagnostics.emit_session_event(
            app,
            "subvolume_processing_job_started",
            log::Level::Info,
            "Subvolume processing job started",
            Some(build_fields([
                ("jobId", json_value(&record.snapshot().job_id)),
                ("storePath", json_value(&request.store_path)),
                ("outputStorePath", json_value(&output_store_path)),
                (
                    "traceLocalOperatorCount",
                    json_value(
                        request
                            .pipeline
                            .trace_local_pipeline
                            .as_ref()
                            .map(|pipeline| pipeline.operation_count())
                            .unwrap_or(0),
                    ),
                ),
                ("checkpointCount", json_value(checkpoint_stages.len())),
                ("reusedCheckpoint", json_value(reused_checkpoint.is_some())),
            ])),
        );
    }
    if let Err(error) = prepare_processing_output_store(
        &request.store_path,
        &output_store_path,
        request.overwrite_existing,
    ) {
        let final_status = record.mark_failed(error);
        if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
            diagnostics.emit_session_event(
                app,
                "subvolume_processing_job_failed",
                log::Level::Error,
                "Subvolume processing job failed",
                Some(build_fields([
                    ("jobId", json_value(&final_status.job_id)),
                    (
                        "error",
                        json_value(final_status.error_message.clone().unwrap_or_default()),
                    ),
                ])),
            );
        }
        return;
    }

    let final_materialize_options = match materialize_options_for_store(&request.store_path) {
        Ok(options) => options,
        Err(error) => {
            let final_status = record.mark_failed(error.clone());
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "subvolume_processing_job_failed",
                    log::Level::Error,
                    "Subvolume processing job failed",
                    Some(build_fields([
                        ("jobId", json_value(&final_status.job_id)),
                        ("error", json_value(&error)),
                    ])),
                );
            }
            return;
        }
    };
    if let Some(reused_checkpoint) = reused_checkpoint.as_ref() {
        let _ = record.push_artifact(reused_checkpoint.artifact.clone());
        if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
            diagnostics.emit_session_event(
                app,
                "subvolume_processing_checkpoint_reused",
                log::Level::Info,
                "Subvolume processing reused a cached checkpoint",
                Some(build_fields([
                    ("jobId", json_value(&job_id)),
                    ("storePath", json_value(&reused_checkpoint.path)),
                    (
                        "afterOperationIndex",
                        json_value(reused_checkpoint.after_operation_index),
                    ),
                ])),
            );
        }
    }

    let mut current_input_store_path = reused_checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.path.clone())
        .unwrap_or_else(|| request.store_path.clone());
    let checkpoint_result: Result<(), seis_runtime::SeisRefineError> =
        checkpoint_stages.iter().try_for_each(|stage| {
            let stage_started_at = Instant::now();
            if record.cancel_requested() {
                return Err(seis_runtime::SeisRefineError::Message(
                    "processing cancelled".to_string(),
                ));
            }
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "subvolume_processing_checkpoint_stage_started",
                    log::Level::Info,
                    "Subvolume checkpoint stage started",
                    Some(build_fields([
                        ("jobId", json_value(&job_id)),
                        ("label", json_value(&stage.stage_label)),
                        ("inputStorePath", json_value(&current_input_store_path)),
                        ("outputStorePath", json_value(&stage.artifact.store_path)),
                        (
                            "segmentOperatorCount",
                            json_value(stage.segment_pipeline.operation_count()),
                        ),
                        (
                            "lineageOperatorCount",
                            json_value(stage.lineage_pipeline.operation_count()),
                        ),
                    ])),
                );
            }
            prepare_processing_output_store(
                &current_input_store_path,
                &stage.artifact.store_path,
                false,
            )
            .map_err(seis_runtime::SeisRefineError::Message)?;
            let stage_materialize_options = materialize_options_for_store(&current_input_store_path)
                .map_err(seis_runtime::SeisRefineError::Message)?;
            let materialize_started_at = Instant::now();
            materialize_processing_volume_with_progress(
                &current_input_store_path,
                &stage.artifact.store_path,
                &stage.segment_pipeline,
                stage_materialize_options,
                |completed, total| {
                    if record.cancel_requested() {
                        return Err(seis_runtime::SeisRefineError::Message(
                            "processing cancelled".to_string(),
                        ));
                    }
                    let _ = record.mark_progress(completed, total, Some(&stage.stage_label));
                    Ok(())
                },
            )?;
            let materialize_duration_ms = materialize_started_at.elapsed().as_millis() as u64;
            let lineage_rewrite_started_at = Instant::now();
            rewrite_trace_local_processing_lineage(
                &stage.artifact.store_path,
                &stage.lineage_pipeline,
                stage.artifact.kind,
            )
            .map_err(seis_runtime::SeisRefineError::Message)?;
            let lineage_rewrite_duration_ms =
                lineage_rewrite_started_at.elapsed().as_millis() as u64;
            let _ = record.push_artifact(stage.artifact.clone());
            let artifact_register_started_at = Instant::now();
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "subvolume_processing_checkpoint_emitted",
                    log::Level::Info,
                    "Subvolume checkpoint emitted",
                    Some(build_fields([
                        ("jobId", json_value(&job_id)),
                        ("storePath", json_value(&stage.artifact.store_path)),
                        ("label", json_value(&stage.artifact.label)),
                    ])),
                );
            }
            if let Err(error) =
                register_processing_store_artifact(app, &request.store_path, &stage.artifact)
            {
                if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                    diagnostics.emit_session_event(
                        app,
                        "subvolume_processing_artifact_register_failed",
                        log::Level::Warn,
                        "Subvolume checkpoint emitted but workspace registration failed",
                        Some(build_fields([
                            ("jobId", json_value(&job_id)),
                            ("storePath", json_value(&stage.artifact.store_path)),
                            ("error", json_value(error)),
                        ])),
                    );
                }
            }
            let artifact_register_duration_ms =
                artifact_register_started_at.elapsed().as_millis() as u64;
            if let (Some(processing_cache), Some(source_fingerprint)) =
                (app.try_state::<ProcessingCacheState>(), source_fingerprint.as_ref())
            {
                if processing_cache.enabled() {
                    match trace_local_pipeline_hash(&stage.lineage_pipeline) {
                        Ok(prefix_hash) => {
                            if let Err(error) = processing_cache.register_visible_checkpoint(
                                TRACE_LOCAL_CACHE_FAMILY,
                                &stage.artifact.store_path,
                                source_fingerprint,
                                &prefix_hash,
                                stage.artifact.step_index + 1,
                                PROCESSING_CACHE_RUNTIME_VERSION,
                                TBVOL_STORE_FORMAT_VERSION,
                            ) {
                                if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                                    diagnostics.emit_session_event(
                                        app,
                                        "subvolume_processing_cache_checkpoint_register_failed",
                                        log::Level::Warn,
                                        "Subvolume checkpoint emitted but cache registration failed",
                                        Some(build_fields([
                                            ("jobId", json_value(&job_id)),
                                            ("storePath", json_value(&stage.artifact.store_path)),
                                            ("error", json_value(error)),
                                        ])),
                                    );
                                }
                            }
                        }
                        Err(error) => {
                            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                                diagnostics.emit_session_event(
                                    app,
                                    "subvolume_processing_cache_hash_failed",
                                    log::Level::Warn,
                                    "Subvolume checkpoint emitted but cache prefix hashing failed",
                                    Some(build_fields([
                                        ("jobId", json_value(&job_id)),
                                        ("storePath", json_value(&stage.artifact.store_path)),
                                        ("error", json_value(error)),
                                    ])),
                                );
                            }
                        }
                    }
                }
            }
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "subvolume_processing_checkpoint_stage_completed",
                    log::Level::Info,
                    "Subvolume checkpoint stage completed",
                    Some(build_fields([
                        ("jobId", json_value(&job_id)),
                        ("label", json_value(&stage.stage_label)),
                        (
                            "stageDurationMs",
                            json_value(stage_started_at.elapsed().as_millis() as u64),
                        ),
                        (
                            "materializeDurationMs",
                            json_value(materialize_duration_ms),
                        ),
                        (
                            "lineageRewriteDurationMs",
                            json_value(lineage_rewrite_duration_ms),
                        ),
                        (
                            "artifactRegisterDurationMs",
                            json_value(artifact_register_duration_ms),
                        ),
                    ])),
                );
            }
            current_input_store_path = stage.artifact.store_path.clone();
            Ok(())
        });

    let result = checkpoint_result.and_then(|_| {
        let remaining_trace_local_pipeline = request
            .pipeline
            .trace_local_pipeline
            .as_ref()
            .and_then(|pipeline| {
                let start_index = checkpoint_stages
                    .last()
                    .map(|stage| stage.artifact.step_index + 1)
                    .or_else(|| {
                        reused_checkpoint
                            .as_ref()
                            .map(|checkpoint| checkpoint.after_operation_index + 1)
                    })
                    .unwrap_or(0);
                (start_index < pipeline.operation_count()).then(|| {
                    pipeline_segment(pipeline, start_index, pipeline.operation_count() - 1)
                })
            });
        let execution_pipeline = SubvolumeProcessingPipeline {
            schema_version: request.pipeline.schema_version,
            revision: request.pipeline.revision,
            preset_id: request.pipeline.preset_id.clone(),
            name: request.pipeline.name.clone(),
            description: request.pipeline.description.clone(),
            trace_local_pipeline: remaining_trace_local_pipeline,
            crop: request.pipeline.crop.clone(),
        };
        materialize_subvolume_processing_volume_with_progress(
            &current_input_store_path,
            &output_store_path,
            &execution_pipeline,
            final_materialize_options,
            |completed, total| {
                if record.cancel_requested() {
                    return Err(seis_runtime::SeismicStoreError::Message(
                        "processing cancelled".to_string(),
                    ));
                }
                let _ = record.mark_progress(completed, total, Some("Crop Subvolume"));
                Ok(())
            },
        )
        .map_err(|error| seis_runtime::SeisRefineError::Message(error.to_string()))
    });

    match result {
        Ok(_) => {
            if let Err(error) = rewrite_subvolume_processing_lineage(
                &output_store_path,
                &request.pipeline,
                ProcessingJobArtifactKind::FinalOutput,
            ) {
                let final_status = record.mark_failed(error.clone());
                if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                    diagnostics.emit_session_event(
                        app,
                        "subvolume_processing_job_failed",
                        log::Level::Error,
                        "Subvolume processing job failed",
                        Some(build_fields([
                            ("jobId", json_value(&final_status.job_id)),
                            ("error", json_value(&error)),
                        ])),
                    );
                }
                return;
            }
            let final_artifact = ProcessingJobArtifact {
                kind: ProcessingJobArtifactKind::FinalOutput,
                step_index: request
                    .pipeline
                    .trace_local_pipeline
                    .as_ref()
                    .map(|pipeline| pipeline.operation_count())
                    .unwrap_or(0),
                label: "Crop Subvolume".to_string(),
                store_path: output_store_path.clone(),
            };
            let _ = record.push_artifact(final_artifact.clone());
            if let Err(error) =
                register_processing_store_artifact(app, &request.store_path, &final_artifact)
            {
                if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                    diagnostics.emit_session_event(
                        app,
                        "subvolume_processing_artifact_register_failed",
                        log::Level::Warn,
                        "Subvolume output emitted but workspace registration failed",
                        Some(build_fields([
                            ("jobId", json_value(&job_id)),
                            ("storePath", json_value(&final_artifact.store_path)),
                            ("error", json_value(error)),
                        ])),
                    );
                }
            }
            let final_status = record.mark_completed(output_store_path.clone());
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                let mut completion_fields = vec![
                    ("jobId", json_value(&final_status.job_id)),
                    ("outputStorePath", json_value(&output_store_path)),
                    ("executionOrder", json_value("trace_local_then_crop")),
                    (
                        "jobDurationMs",
                        json_value(job_started_at.elapsed().as_millis() as u64),
                    ),
                    ("checkpointCount", json_value(checkpoint_stages.len())),
                ];
                if let Ok(handle) = open_store(&output_store_path) {
                    completion_fields.push(("datasetId", json_value(&handle.dataset_id().0)));
                    completion_fields.push(("shape", json_value(handle.manifest.volume.shape)));
                    if let Some((inline_min, inline_max)) =
                        section_axis_range(&handle, SectionAxis::Inline)
                    {
                        completion_fields
                            .push(("inlineRange", json_value([inline_min, inline_max])));
                    }
                    if let Some((xline_min, xline_max)) =
                        section_axis_range(&handle, SectionAxis::Xline)
                    {
                        completion_fields.push(("xlineRange", json_value([xline_min, xline_max])));
                    }
                    if let Some((z_min, z_max)) = sample_axis_range_ms(&handle) {
                        completion_fields.push(("zRangeMs", json_value([z_min, z_max])));
                    }
                }
                diagnostics.emit_session_event(
                    app,
                    "subvolume_processing_job_completed",
                    log::Level::Info,
                    "Subvolume processing job completed",
                    Some(build_fields(completion_fields)),
                );
            }
        }
        Err(error) => {
            let final_status = if record.cancel_requested() {
                record.mark_cancelled()
            } else {
                record.mark_failed(error.to_string())
            };
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "subvolume_processing_job_failed",
                    if matches!(
                        final_status.state,
                        seis_runtime::ProcessingJobState::Cancelled
                    ) {
                        log::Level::Warn
                    } else {
                        log::Level::Error
                    },
                    if matches!(
                        final_status.state,
                        seis_runtime::ProcessingJobState::Cancelled
                    ) {
                        "Subvolume processing job cancelled"
                    } else {
                        "Subvolume processing job failed"
                    },
                    Some(build_fields([
                        ("jobId", json_value(&final_status.job_id)),
                        (
                            "jobDurationMs",
                            json_value(job_started_at.elapsed().as_millis() as u64),
                        ),
                        ("checkpointCount", json_value(checkpoint_stages.len())),
                        (
                            "state",
                            json_value(format!("{:?}", final_status.state).to_ascii_lowercase()),
                        ),
                        (
                            "error",
                            json_value(final_status.error_message.clone().unwrap_or_default()),
                        ),
                    ])),
                );
            }
        }
    }
}

fn run_gather_processing_job(
    app: &AppHandle,
    record: &JobRecord,
    request: RunGatherProcessingRequest,
) {
    let app_paths = match AppPaths::resolve(app) {
        Ok(paths) => paths,
        Err(error) => {
            let _ = record.mark_failed(error.clone());
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "gather_processing_job_failed",
                    log::Level::Error,
                    "Gather processing job failed before initialization",
                    Some(build_fields([("error", json_value(&error))])),
                );
            }
            return;
        }
    };
    let output_store_path = request.output_store_path.clone().unwrap_or_else(|| {
        default_gather_processing_store_path(&app_paths, &request.store_path, &request.pipeline)
            .unwrap_or_else(|_| "derived-output.tbgath".to_string())
    });
    let job_started_at = Instant::now();
    let _ = record.mark_running(None);
    if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
        diagnostics.emit_session_event(
            app,
            "gather_processing_job_started",
            log::Level::Info,
            "Gather processing job started",
            Some(build_fields([
                ("jobId", json_value(&record.snapshot().job_id)),
                ("storePath", json_value(&request.store_path)),
                ("outputStorePath", json_value(&output_store_path)),
                (
                    "operatorCount",
                    json_value(request.pipeline.operations.len()),
                ),
            ])),
        );
    }
    if let Err(error) = prepare_processing_output_store(
        &request.store_path,
        &output_store_path,
        request.overwrite_existing,
    ) {
        let final_status = record.mark_failed(error);
        if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
            diagnostics.emit_session_event(
                app,
                "gather_processing_job_failed",
                log::Level::Error,
                "Gather processing job failed",
                Some(build_fields([
                    ("jobId", json_value(&final_status.job_id)),
                    (
                        "error",
                        json_value(final_status.error_message.clone().unwrap_or_default()),
                    ),
                ])),
            );
        }
        return;
    }

    let result = materialize_gather_processing_store_with_progress(
        &request.store_path,
        &output_store_path,
        &request.pipeline,
        |completed, total| {
            if record.cancel_requested() {
                return Err(seis_runtime::SeisRefineError::Message(
                    "processing cancelled".to_string(),
                ));
            }
            let _ = record.mark_progress(completed, total, None);
            Ok(())
        },
    );

    match result {
        Ok(_) => {
            let final_status = record.mark_completed(output_store_path.clone());
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "gather_processing_job_completed",
                    log::Level::Info,
                    "Gather processing job completed",
                    Some(build_fields([
                        ("jobId", json_value(&final_status.job_id)),
                        ("outputStorePath", json_value(&output_store_path)),
                        (
                            "jobDurationMs",
                            json_value(job_started_at.elapsed().as_millis() as u64),
                        ),
                    ])),
                );
            }
        }
        Err(error) => {
            let final_status = if record.cancel_requested() {
                record.mark_cancelled()
            } else {
                record.mark_failed(error.to_string())
            };
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "gather_processing_job_failed",
                    if matches!(
                        final_status.state,
                        seis_runtime::ProcessingJobState::Cancelled
                    ) {
                        log::Level::Warn
                    } else {
                        log::Level::Error
                    },
                    if matches!(
                        final_status.state,
                        seis_runtime::ProcessingJobState::Cancelled
                    ) {
                        "Gather processing job cancelled"
                    } else {
                        "Gather processing job failed"
                    },
                    Some(build_fields([
                        ("jobId", json_value(&final_status.job_id)),
                        (
                            "jobDurationMs",
                            json_value(job_started_at.elapsed().as_millis() as u64),
                        ),
                        (
                            "state",
                            json_value(format!("{:?}", final_status.state).to_ascii_lowercase()),
                        ),
                        (
                            "error",
                            json_value(final_status.error_message.clone().unwrap_or_default()),
                        ),
                    ])),
                );
            }
        }
    }
}

fn prepare_processing_output_store(
    input_store_path: &str,
    output_store_path: &str,
    overwrite_existing: bool,
) -> Result<(), String> {
    let input_path = std::path::Path::new(input_store_path);
    let output_path = std::path::Path::new(output_store_path);
    let input_canonical = input_path
        .canonicalize()
        .unwrap_or_else(|_| input_path.to_path_buf());
    let output_canonical = output_path
        .canonicalize()
        .unwrap_or_else(|_| output_path.to_path_buf());
    if input_canonical == output_canonical {
        return Err("Output store path cannot overwrite the input store.".to_string());
    }
    if !output_path.exists() {
        return Ok(());
    }
    if !overwrite_existing {
        return Err(format!(
            "Output processing store already exists: {}",
            output_path.display()
        ));
    }
    let metadata = std::fs::symlink_metadata(output_path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_dir() {
        std::fs::remove_dir_all(output_path).map_err(|error| error.to_string())?;
    } else {
        std::fs::remove_file(output_path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_diagnostics_status_command(
    diagnostics: State<DiagnosticsState>,
) -> Result<diagnostics::DiagnosticsStatus, String> {
    Ok(diagnostics.status())
}

#[tauri::command]
fn set_diagnostics_verbosity_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    enabled: bool,
) -> Result<(), String> {
    diagnostics.set_verbose_enabled(enabled);
    diagnostics.emit_session_event(
        &app,
        "config",
        if enabled {
            log::Level::Info
        } else {
            log::Level::Warn
        },
        if enabled {
            "Verbose diagnostics enabled for this session"
        } else {
            "Verbose diagnostics disabled for this session"
        },
        Some(build_fields([("verboseEnabled", json_value(enabled))])),
    );
    Ok(())
}

#[tauri::command]
fn export_diagnostics_bundle_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
) -> Result<ExportBundleResponse, String> {
    let bundle_path = diagnostics.export_bundle(&app)?;
    diagnostics.emit_session_event(
        &app,
        "exported",
        log::Level::Info,
        "Exported diagnostics bundle",
        Some(build_fields([(
            "bundlePath",
            json_value(bundle_path.display().to_string()),
        )])),
    );
    Ok(ExportBundleResponse {
        bundle_path: bundle_path.display().to_string(),
    })
}

#[tauri::command]
fn emit_frontend_diagnostics_event_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    request: FrontendDiagnosticsEventRequest,
) -> Result<(), String> {
    let level = match request.level.trim().to_ascii_lowercase().as_str() {
        "error" => log::Level::Error,
        "warn" | "warning" => log::Level::Warn,
        "debug" => log::Level::Debug,
        _ => log::Level::Info,
    };
    let mut fields = request.fields.unwrap_or_default();
    fields.insert("frontendStage".to_string(), json_value(request.stage));

    diagnostics.emit_session_event(
        &app,
        "frontend_profile",
        level,
        request.message,
        Some(fields),
    );
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let session_basename = DiagnosticsState::session_basename();
    let enable_devtools = cfg!(debug_assertions)
        && std::env::var("TRACEBOOST_ENABLE_DEVTOOLS")
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false);

    let log_plugin = tauri_plugin_log::Builder::default()
        .clear_targets()
        .level(log::LevelFilter::Info)
        .level_for("traceboost_desktop_lib", log::LevelFilter::Debug)
        .level_for(
            "traceboost_desktop_lib::diagnostics",
            log::LevelFilter::Debug,
        )
        .level_for("traceboost_app", log::LevelFilter::Debug)
        .level_for("seis_runtime", log::LevelFilter::Info)
        .target(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::Stdout,
        ))
        .target(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::LogDir {
                file_name: Some(session_basename.clone()),
            },
        ))
        .build();

    let builder = tauri::Builder::default()
        .menu(build_app_menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            FILE_OPEN_VOLUME_MENU_ID => {
                if let Err(error) = app.emit(FILE_OPEN_VOLUME_MENU_EVENT, ()) {
                    log::warn!("failed to emit native open-volume menu event: {error}");
                }
            }
            VELOCITY_MODEL_MENU_ID => {
                if let Err(error) = app.emit(VELOCITY_MODEL_MENU_EVENT, ()) {
                    log::warn!("failed to emit native velocity-model menu event: {error}");
                }
            }
            _ => {}
        })
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let app_paths = AppPaths::resolve(&app.handle().clone())?;
            let diagnostics =
                DiagnosticsState::initialize(app_paths.logs_dir(), session_basename.clone())?;
            let processing = ProcessingState::initialize(app_paths.pipeline_presets_dir())?;
            fs::create_dir_all(app_paths.map_transform_cache_dir())
                .map_err(|error| error.to_string())?;
            let processing_cache = ProcessingCacheState::initialize(
                app_paths.processing_cache_dir(),
                app_paths.processing_cache_volumes_dir(),
                app_paths.processing_cache_index_path(),
                app_paths.settings_path(),
            )?;
            let preview_sessions = PreviewSessionState::default();
            let workspace = WorkspaceState::initialize(
                app_paths.dataset_registry_path(),
                app_paths.workspace_session_path(),
            )?;
            diagnostics.emit_session_event(
                &app.handle().clone(),
                "started",
                log::Level::Info,
                "Diagnostics session started",
                Some(build_fields([
                    (
                        "sessionLogPath",
                        json_value(diagnostics.session_log_path().display().to_string()),
                    ),
                    ("verboseEnabled", json_value(false)),
                ])),
            );
            app.manage(diagnostics);
            app.manage(processing);
            app.manage(processing_cache);
            app.manage(preview_sessions);
            app.manage(workspace);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            preflight_import_command,
            import_dataset_command,
            import_prestack_offset_dataset_command,
            open_dataset_command,
            ensure_demo_survey_time_depth_transform_command,
            load_velocity_models_command,
            import_velocity_functions_model_command,
            build_velocity_model_transform_command,
            get_dataset_export_capabilities_command,
            export_dataset_segy_command,
            export_dataset_zarr_command,
            import_horizon_xyz_command,
            load_section_horizons_command,
            load_section_command,
            load_section_binary_command,
            load_depth_converted_section_binary_command,
            load_resolved_section_display_binary_command,
            load_gather_command,
            preview_processing_command,
            preview_processing_binary_command,
            preview_subvolume_processing_command,
            preview_subvolume_processing_binary_command,
            preview_gather_processing_command,
            amplitude_spectrum_command,
            velocity_scan_command,
            run_processing_command,
            run_subvolume_processing_command,
            run_gather_processing_command,
            get_processing_job_command,
            cancel_processing_job_command,
            list_pipeline_presets_command,
            save_pipeline_preset_command,
            delete_pipeline_preset_command,
            load_workspace_state_command,
            upsert_dataset_entry_command,
            remove_dataset_entry_command,
            set_active_dataset_entry_command,
            save_workspace_session_command,
            set_dataset_native_coordinate_reference_command,
            resolve_survey_map_command,
            list_project_well_overlay_inventory_command,
            list_project_well_time_depth_models_command,
            list_project_well_time_depth_inventory_command,
            set_project_active_well_time_depth_model_command,
            import_project_well_time_depth_model_command,
            import_project_well_time_depth_asset_command,
            compile_project_well_time_depth_authored_model_command,
            read_project_well_time_depth_model_command,
            resolve_project_section_well_overlays_command,
            default_import_store_path_command,
            default_import_prestack_store_path_command,
            default_processing_store_path_command,
            default_subvolume_processing_store_path_command,
            default_gather_processing_store_path_command,
            get_diagnostics_status_command,
            set_diagnostics_verbosity_command,
            export_diagnostics_bundle_command,
            emit_frontend_diagnostics_event_command
        ]);

    #[cfg(debug_assertions)]
    let builder = if enable_devtools {
        builder.plugin(tauri_plugin_devtools::init())
    } else {
        builder.plugin(log_plugin)
    };

    #[cfg(not(debug_assertions))]
    let builder = builder.plugin(log_plugin);

    builder
        .run(tauri::generate_context!())
        .expect("error while running traceboost desktop shell");
}
