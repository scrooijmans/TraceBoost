mod app_paths;
mod diagnostics;
mod processing;
mod processing_cache;
mod workspace;

#[cfg(test)]
mod processing_cache_bench;

use seis_contracts_interop::{
    AmplitudeSpectrumRequest, AmplitudeSpectrumResponse, CancelProcessingJobRequest,
    CancelProcessingJobResponse, DeletePipelinePresetRequest, DeletePipelinePresetResponse,
    GatherProcessingPipeline, GatherRequest, GatherView, GetProcessingJobRequest,
    GetProcessingJobResponse, IPC_SCHEMA_VERSION, ImportDatasetRequest, ImportDatasetResponse,
    ImportPrestackOffsetDatasetRequest, ImportPrestackOffsetDatasetResponse,
    ListPipelinePresetsResponse, LoadWorkspaceStateResponse, OpenDatasetRequest,
    OpenDatasetResponse, PreviewGatherProcessingRequest, PreviewGatherProcessingResponse,
    PreviewTraceLocalProcessingRequest, PreviewTraceLocalProcessingResponse,
    RemoveDatasetEntryRequest, RemoveDatasetEntryResponse, RunGatherProcessingRequest,
    RunGatherProcessingResponse, RunTraceLocalProcessingRequest, RunTraceLocalProcessingResponse,
    SavePipelinePresetRequest, SavePipelinePresetResponse, SaveWorkspaceSessionRequest,
    SaveWorkspaceSessionResponse, SetActiveDatasetEntryRequest, SetActiveDatasetEntryResponse,
    SurveyPreflightRequest, SurveyPreflightResponse, UpsertDatasetEntryRequest,
    UpsertDatasetEntryResponse, VelocityScanRequest, VelocityScanResponse,
};
use seis_runtime::{
    MaterializeOptions, ProcessingJobArtifact, ProcessingJobArtifactKind, ProcessingPipelineSpec,
    SectionAxis, SectionView, TbvolManifest, TraceLocalProcessingPipeline,
    materialize_gather_processing_store_with_progress, materialize_processing_volume_with_progress,
    open_store,
};
use std::{
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};
use tauri::{
    AppHandle, Emitter, Manager, State,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
};
use traceboost_app::{
    amplitude_spectrum, import_dataset, import_prestack_offset_dataset, load_gather,
    open_dataset_summary, preflight_dataset, preview_gather_processing, preview_processing,
    run_velocity_scan,
};

use crate::app_paths::AppPaths;
use crate::diagnostics::{DiagnosticsState, ExportBundleResponse, build_fields, json_value};
use crate::processing::{JobRecord, ProcessingState};
use crate::processing_cache::{ProcessingCachePolicy, ProcessingCacheState};
use crate::workspace::WorkspaceState;

const FILE_OPEN_VOLUME_MENU_ID: &str = "file.open_volume";
const FILE_OPEN_VOLUME_MENU_EVENT: &str = "menu:file-open-volume";
const TRACE_LOCAL_CACHE_FAMILY: &str = "trace_local";
const TBVOL_STORE_FORMAT_VERSION: &str = "tbvol-v1";
const PROCESSING_CACHE_RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

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

fn pipeline_output_slug(pipeline: &seis_runtime::TraceLocalProcessingPipeline) -> String {
    if let Some(name) = pipeline
        .name
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return sanitized_stem(name, "pipeline");
    }

    let mut parts = Vec::with_capacity(pipeline.operations.len());
    for operation in &pipeline.operations {
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
    visibility: TraceLocalStageVisibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TraceLocalStageVisibility {
    VisibleArtifact,
    HiddenPrefix,
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

fn display_store_stem(store_path: &str) -> String {
    Path::new(store_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("volume")
        .to_string()
}

fn clone_pipeline_with_operations(
    pipeline: &TraceLocalProcessingPipeline,
    operations: Vec<seis_runtime::ProcessingOperation>,
) -> TraceLocalProcessingPipeline {
    TraceLocalProcessingPipeline {
        schema_version: pipeline.schema_version,
        revision: pipeline.revision,
        preset_id: pipeline.preset_id.clone(),
        name: pipeline.name.clone(),
        description: pipeline.description.clone(),
        operations,
    }
}

fn pipeline_prefix(
    pipeline: &TraceLocalProcessingPipeline,
    end_operation_index: usize,
) -> TraceLocalProcessingPipeline {
    clone_pipeline_with_operations(
        pipeline,
        pipeline.operations[..=end_operation_index].to_vec(),
    )
}

fn pipeline_segment(
    pipeline: &TraceLocalProcessingPipeline,
    start_operation_index: usize,
    end_operation_index: usize,
) -> TraceLocalProcessingPipeline {
    clone_pipeline_with_operations(
        pipeline,
        pipeline.operations[start_operation_index..=end_operation_index].to_vec(),
    )
}

fn resolve_trace_local_checkpoint_indexes(
    pipeline: &TraceLocalProcessingPipeline,
    checkpoints: &[seis_runtime::TraceLocalProcessingCheckpoint],
) -> Result<Vec<usize>, String> {
    if pipeline.operations.is_empty() {
        return Ok(Vec::new());
    }

    let last_index = pipeline.operations.len() - 1;
    let mut indexes = checkpoints
        .iter()
        .map(|checkpoint| checkpoint.after_operation_index)
        .collect::<Vec<_>>();
    indexes.sort_unstable();
    indexes.dedup();

    for index in &indexes {
        if *index >= pipeline.operations.len() {
            return Err(format!(
                "Checkpoint after_operation_index {index} is out of range for a pipeline with {} operations.",
                pipeline.operations.len()
            ));
        }
        if *index == last_index {
            return Err(
                "Checkpoint markers cannot target the final operator because the final output is emitted automatically."
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
    let checkpoint_indexes =
        resolve_trace_local_checkpoint_indexes(&request.pipeline, &request.checkpoints)?;
    let mut stage_end_indexes = checkpoint_indexes;
    let final_step_index = request.pipeline.operations.len().saturating_sub(1);
    stage_end_indexes.push(final_step_index);
    stage_end_indexes.retain(|index| *index >= start_operation_index);

    let mut stages = Vec::with_capacity(stage_end_indexes.len());
    let mut segment_start = start_operation_index;
    for end_index in stage_end_indexes {
        let operation = request
            .pipeline
            .operations
            .get(end_index)
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
            visibility: TraceLocalStageVisibility::VisibleArtifact,
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
) -> Result<Option<ReusedTraceLocalCheckpoint>, String> {
    if processing_cache.settings().policy == ProcessingCachePolicy::Off {
        return Ok(None);
    }

    let checkpoint_indexes =
        resolve_trace_local_checkpoint_indexes(&request.pipeline, &request.checkpoints)?;
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
                .operations
                .get(checkpoint_index)
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

fn hidden_prefix_output_store_path(
    processing_cache: &ProcessingCacheState,
    source_fingerprint: &str,
    prefix_hash: &str,
    prefix_len: usize,
) -> String {
    processing_cache
        .volumes_dir()
        .join(format!(
            "trace-local-{source_fingerprint}-prefix-{prefix_len:02}-{prefix_hash}.tbvol"
        ))
        .display()
        .to_string()
}

fn maybe_add_hidden_trace_local_prefix_stage(
    processing_cache: &ProcessingCacheState,
    request: &RunTraceLocalProcessingRequest,
    stages: &mut Vec<TraceLocalProcessingStage>,
    start_operation_index: usize,
    source_fingerprint: Option<&str>,
) -> Result<(), String> {
    if processing_cache.settings().policy == ProcessingCachePolicy::Off {
        return Ok(());
    }
    if request.pipeline.operations.len() <= 1 {
        return Ok(());
    }
    if !request.checkpoints.is_empty() {
        return Ok(());
    }
    if stages.len() != 1 {
        return Ok(());
    }
    let Some(source_fingerprint) = source_fingerprint else {
        return Ok(());
    };
    let hidden_prefix_end_index = request.pipeline.operations.len() - 2;
    if hidden_prefix_end_index < start_operation_index {
        return Ok(());
    }

    let lineage_pipeline = pipeline_prefix(&request.pipeline, hidden_prefix_end_index);
    let prefix_hash = trace_local_pipeline_hash(&lineage_pipeline)?;
    if processing_cache
        .lookup_prefix_artifact(
            TRACE_LOCAL_CACHE_FAMILY,
            source_fingerprint,
            &prefix_hash,
            hidden_prefix_end_index + 1,
        )?
        .is_some()
    {
        return Ok(());
    }

    let operation = request
        .pipeline
        .operations
        .get(hidden_prefix_end_index)
        .ok_or_else(|| {
            format!("Missing operation at hidden prefix index {hidden_prefix_end_index}")
        })?;
    let stage = TraceLocalProcessingStage {
        segment_pipeline: pipeline_segment(
            &request.pipeline,
            start_operation_index,
            hidden_prefix_end_index,
        ),
        lineage_pipeline,
        stage_label: format!(
            "Cached prefix after step {}: {}",
            hidden_prefix_end_index + 1,
            processing_operation_display_label(operation)
        ),
        artifact: ProcessingJobArtifact {
            kind: ProcessingJobArtifactKind::Checkpoint,
            step_index: hidden_prefix_end_index,
            label: format!("Cached prefix after step {}", hidden_prefix_end_index + 1),
            store_path: hidden_prefix_output_store_path(
                processing_cache,
                source_fingerprint,
                &prefix_hash,
                hidden_prefix_end_index + 1,
            ),
        },
        visibility: TraceLocalStageVisibility::HiddenPrefix,
    };

    let final_stage = stages
        .pop()
        .ok_or_else(|| "Missing final processing stage.".to_string())?;
    stages.push(stage);
    stages.push(TraceLocalProcessingStage {
        segment_pipeline: pipeline_segment(
            &request.pipeline,
            hidden_prefix_end_index + 1,
            request.pipeline.operations.len() - 1,
        ),
        lineage_pipeline: final_stage.lineage_pipeline,
        stage_label: final_stage.stage_label,
        artifact: final_stage.artifact,
        visibility: TraceLocalStageVisibility::VisibleArtifact,
    });
    Ok(())
}

fn rewrite_trace_local_processing_lineage(
    store_path: &str,
    pipeline: &TraceLocalProcessingPipeline,
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
    ProcessingCacheState::fingerprint_json(&TraceLocalPipelineCacheIdentity {
        schema_version: pipeline.schema_version,
        revision: pipeline.revision,
        operations: &pipeline.operations,
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
    let separator = PredefinedMenuItem::separator(app)?;
    let close_window = PredefinedMenuItem::close_window(app, None)?;

    Menu::with_items(
        app,
        &[&Submenu::with_items(
            app,
            "&File",
            true,
            &[&open_volume, &separator, &close_window],
        )?],
    )
}

#[tauri::command]
fn preflight_import_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    input_path: String,
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
    overwrite_existing: bool,
) -> Result<ImportDatasetResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "import_dataset",
        "Starting SEG-Y import",
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
        "Reading SEG-Y survey and building runtime store",
        Some(build_fields([("stage", json_value("read_segy"))])),
    );

    let result = import_dataset(ImportDatasetRequest {
        schema_version: IPC_SCHEMA_VERSION,
        input_path,
        output_store_path,
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
                "SEG-Y import completed",
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
                "SEG-Y import failed",
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
fn load_section_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    store_path: String,
    axis: SectionAxis,
    index: usize,
) -> Result<SectionView, String> {
    let axis_name = format!("{axis:?}").to_ascii_lowercase();
    let operation = diagnostics.start_operation(
        &app,
        "load_section",
        "Loading section view",
        Some(build_fields([
            ("storePath", json_value(&store_path)),
            ("axis", json_value(&axis_name)),
            ("index", json_value(index)),
            ("stage", json_value("validate_input")),
        ])),
    );
    diagnostics.progress(
        &app,
        &operation,
        "Opening runtime store for section load",
        Some(build_fields([("stage", json_value("open_store"))])),
    );

    let result = open_store(store_path).and_then(|handle| handle.section_view(axis, index));
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
            diagnostics.fail(
                &app,
                &operation,
                "Section load failed",
                Some(build_fields([
                    ("stage", json_value("load_section")),
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
fn preview_processing_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    request: PreviewTraceLocalProcessingRequest,
) -> Result<PreviewTraceLocalProcessingResponse, String> {
    let operation = diagnostics.start_operation(
        &app,
        "preview_processing",
        "Generating processing preview",
        Some(build_fields([
            ("storePath", json_value(&request.store_path)),
            (
                "axis",
                json_value(format!("{:?}", request.section.axis).to_ascii_lowercase()),
            ),
            ("index", json_value(request.section.index)),
            (
                "operatorCount",
                json_value(request.pipeline.operations.len()),
            ),
            ("stage", json_value("preview_section")),
        ])),
    );

    let result = preview_processing(request);
    match result {
        Ok(response) => {
            diagnostics.complete(
                &app,
                &operation,
                "Processing preview ready",
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
            diagnostics.fail(
                &app,
                &operation,
                "Processing preview failed",
                Some(build_fields([("error", json_value(&message))])),
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
                        .map(|pipeline| pipeline.operations.len())
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
    let allow_exact_reuse = processing_cache.settings().policy != ProcessingCachePolicy::Off
        && request.output_store_path.is_none()
        && request.checkpoints.is_empty();

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
                step_index: request.pipeline.operations.len().saturating_sub(1),
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
                json_value(request.pipeline.operations.len()),
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
            match resolve_reused_trace_local_checkpoint(&processing_cache, &request) {
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
        Some(processing_cache)
            if processing_cache.settings().policy != ProcessingCachePolicy::Off =>
        {
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
    let mut stages = match build_trace_local_processing_stages_from(
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
    if let (Some(processing_cache), Some(source_fingerprint)) = (
        app.try_state::<ProcessingCacheState>(),
        source_fingerprint.as_deref(),
    ) {
        if let Err(error) = maybe_add_hidden_trace_local_prefix_stage(
            &processing_cache,
            &request,
            &mut stages,
            reused_checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.after_operation_index + 1)
                .unwrap_or(0),
            Some(source_fingerprint),
        ) {
            if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
                diagnostics.emit_session_event(
                    app,
                    "processing_hidden_prefix_plan_failed",
                    log::Level::Warn,
                    "Hidden prefix planning failed; processing will continue without an automatic cached prefix",
                    Some(build_fields([
                        ("jobId", json_value(&job_id)),
                        ("error", json_value(error)),
                    ])),
                );
            }
        }
    }
    let _ = record.mark_running(stages.first().map(|stage| stage.stage_label.clone()));
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
        if record.cancel_requested() {
            return Err(seis_runtime::SeisRefineError::Message(
                "processing cancelled".to_string(),
            ));
        }
        if !matches!(stage.artifact.kind, ProcessingJobArtifactKind::FinalOutput) {
            prepare_processing_output_store(
                &current_input_store_path,
                &stage.artifact.store_path,
                matches!(stage.visibility, TraceLocalStageVisibility::HiddenPrefix),
            )
            .map_err(seis_runtime::SeisRefineError::Message)?;
        }
        materialize_processing_volume_with_progress(
            &current_input_store_path,
            &stage.artifact.store_path,
            &stage.segment_pipeline,
            MaterializeOptions::default(),
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
        rewrite_trace_local_processing_lineage(&stage.artifact.store_path, &stage.lineage_pipeline)
            .map_err(seis_runtime::SeisRefineError::Message)?;
        if matches!(stage.visibility, TraceLocalStageVisibility::VisibleArtifact) {
            let _ = record.push_artifact(stage.artifact.clone());
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
            if let Err(error) =
                register_processing_store_artifact(app, &request.store_path, &stage.artifact)
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
        } else if let Some(diagnostics) = app.try_state::<DiagnosticsState>() {
            diagnostics.emit_session_event(
                app,
                "processing_hidden_prefix_emitted",
                log::Level::Info,
                "Processing hidden prefix emitted",
                Some(build_fields([
                    ("jobId", json_value(&job_id)),
                    ("storePath", json_value(&stage.artifact.store_path)),
                    ("label", json_value(&stage.stage_label)),
                ])),
            );
        }
        if matches!(stage.visibility, TraceLocalStageVisibility::VisibleArtifact)
            && matches!(stage.artifact.kind, ProcessingJobArtifactKind::Checkpoint)
        {
            if let (Some(processing_cache), Some(source_fingerprint)) =
                (app.try_state::<ProcessingCacheState>(), source_fingerprint.as_ref())
            {
                if processing_cache.settings().policy != ProcessingCachePolicy::Off {
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
        if matches!(stage.visibility, TraceLocalStageVisibility::HiddenPrefix) {
            if let (Some(processing_cache), Some(source_fingerprint)) =
                (app.try_state::<ProcessingCacheState>(), source_fingerprint.as_ref())
            {
                if processing_cache.settings().policy != ProcessingCachePolicy::Off {
                    match trace_local_pipeline_hash(&stage.lineage_pipeline) {
                        Ok(prefix_hash) => {
                            if let Err(error) = processing_cache.register_hidden_prefix(
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
                                        "processing_cache_hidden_prefix_register_failed",
                                        log::Level::Warn,
                                        "Hidden prefix emitted but cache registration failed",
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
                                    "processing_cache_hidden_prefix_hash_failed",
                                    log::Level::Warn,
                                    "Hidden prefix emitted but cache hashing failed",
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
        current_input_store_path = stage.artifact.store_path.clone();
        Ok(())
    });

    match result {
        Ok(_) => {
            if let Some(processing_cache) = app.try_state::<ProcessingCacheState>() {
                if processing_cache.settings().policy != ProcessingCachePolicy::Off {
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
                                request.pipeline.operations.len(),
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
    let _ = record.mark_running(None);
    let output_store_path = request.output_store_path.clone().unwrap_or_else(|| {
        default_gather_processing_store_path(&app_paths, &request.store_path, &request.pipeline)
            .unwrap_or_else(|_| "derived-output.tbgath".to_string())
    });
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
        .on_menu_event(|app, event| {
            if event.id().as_ref() != FILE_OPEN_VOLUME_MENU_ID {
                return;
            }

            if let Err(error) = app.emit(FILE_OPEN_VOLUME_MENU_EVENT, ()) {
                log::warn!("failed to emit native open-volume menu event: {error}");
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let app_paths = AppPaths::resolve(&app.handle().clone())?;
            let diagnostics =
                DiagnosticsState::initialize(app_paths.logs_dir(), session_basename.clone())?;
            let processing = ProcessingState::initialize(app_paths.pipeline_presets_dir())?;
            let processing_cache = ProcessingCacheState::initialize(
                app_paths.processing_cache_dir(),
                app_paths.processing_cache_volumes_dir(),
                app_paths.processing_cache_gathers_dir(),
                app_paths.processing_cache_tmp_dir(),
                app_paths.processing_cache_index_path(),
                app_paths.settings_path(),
            )?;
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
            app.manage(workspace);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            preflight_import_command,
            import_dataset_command,
            import_prestack_offset_dataset_command,
            open_dataset_command,
            load_section_command,
            load_gather_command,
            preview_processing_command,
            preview_gather_processing_command,
            amplitude_spectrum_command,
            velocity_scan_command,
            run_processing_command,
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
            default_import_store_path_command,
            default_import_prestack_store_path_command,
            default_processing_store_path_command,
            default_gather_processing_store_path_command,
            get_diagnostics_status_command,
            set_diagnostics_verbosity_command,
            export_diagnostics_bundle_command
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
