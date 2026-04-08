use std::{
    fs,
    path::{Path, PathBuf},
};

use seis_contracts_interop::{
    AmplitudeSpectrumRequest, AmplitudeSpectrumResponse, DatasetSummary, GatherProcessingPipeline,
    GatherRequest, GatherView, IPC_SCHEMA_VERSION, ImportDatasetRequest, ImportDatasetResponse,
    ImportPrestackOffsetDatasetRequest, ImportPrestackOffsetDatasetResponse, OpenDatasetRequest,
    OpenDatasetResponse, PrestackThirdAxisField, PreviewGatherProcessingRequest,
    PreviewGatherProcessingResponse, PreviewTraceLocalProcessingRequest,
    PreviewTraceLocalProcessingResponse, RunGatherProcessingRequest,
    RunTraceLocalProcessingRequest, SuggestedImportAction, SurveyPreflightRequest,
    SurveyPreflightResponse, VelocityFunctionSource, VelocityScanRequest, VelocityScanResponse,
};
use seis_io::HeaderField;
use seis_runtime::{
    GatherInterpolationMode, IngestOptions, MaterializeOptions, PreviewView, SeisGeometryOptions,
    SparseSurveyPolicy, TraceLocalProcessingPipeline, amplitude_spectrum_from_store,
    describe_prestack_store, describe_store, ingest_prestack_offset_segy, ingest_segy,
    materialize_gather_processing_store, materialize_processing_volume, open_prestack_store,
    open_store, preflight_segy, prestack_gather_view, preview_gather_processing_view,
    preview_processing_section_view, velocity_scan,
};

const DEFAULT_SPARSE_FILL_VALUE: f32 = 0.0;

pub fn preflight_dataset(
    request: SurveyPreflightRequest,
) -> Result<SurveyPreflightResponse, Box<dyn std::error::Error>> {
    let preflight = preflight_segy(&request.input_path, &IngestOptions::default())?;
    Ok(SurveyPreflightResponse {
        schema_version: IPC_SCHEMA_VERSION,
        input_path: request.input_path,
        trace_count: preflight.inspection.trace_count,
        samples_per_trace: preflight.inspection.samples_per_trace as usize,
        classification: preflight.geometry.classification,
        stacking_state: preflight.geometry.stacking_state,
        organization: preflight.geometry.organization,
        layout: preflight.geometry.layout,
        gather_axis_kind: preflight.geometry.gather_axis_kind,
        suggested_action: suggested_action(preflight.recommended_action),
        observed_trace_count: preflight.geometry.observed_trace_count,
        expected_trace_count: preflight.geometry.expected_trace_count,
        completeness_ratio: preflight.geometry.completeness_ratio,
        notes: preflight.notes,
    })
}

pub fn import_dataset(
    request: ImportDatasetRequest,
) -> Result<ImportDatasetResponse, Box<dyn std::error::Error>> {
    let input = PathBuf::from(&request.input_path);
    let output = PathBuf::from(&request.output_store_path);
    prepare_output_store(&input, &output, request.overwrite_existing)?;
    let handle = ingest_segy(
        &input,
        &output,
        IngestOptions {
            sparse_survey_policy: SparseSurveyPolicy::RegularizeToDense {
                fill_value: DEFAULT_SPARSE_FILL_VALUE,
            },
            ..IngestOptions::default()
        },
    )?;
    Ok(ImportDatasetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: dataset_summary_for_path(&handle.root)?,
    })
}

pub fn import_prestack_offset_dataset(
    request: ImportPrestackOffsetDatasetRequest,
) -> Result<ImportPrestackOffsetDatasetResponse, Box<dyn std::error::Error>> {
    let input = PathBuf::from(&request.input_path);
    let output = PathBuf::from(&request.output_store_path);
    prepare_output_store(&input, &output, request.overwrite_existing)?;
    let handle = ingest_prestack_offset_segy(
        &input,
        &output,
        IngestOptions {
            geometry: SeisGeometryOptions {
                third_axis_field: Some(prestack_third_axis_field(request.third_axis_field)),
                ..SeisGeometryOptions::default()
            },
            ..IngestOptions::default()
        },
    )?;
    Ok(ImportPrestackOffsetDatasetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: dataset_summary_for_path(&handle.root)?,
    })
}

fn prepare_output_store(
    input_path: &Path,
    output_path: &Path,
    overwrite_existing: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !overwrite_existing || !output_path.exists() {
        return Ok(());
    }

    let input_path = input_path
        .canonicalize()
        .unwrap_or_else(|_| input_path.to_path_buf());
    let output_path = output_path
        .canonicalize()
        .unwrap_or_else(|_| output_path.to_path_buf());

    if input_path == output_path {
        return Err("Output store path cannot overwrite the input SEG-Y file.".into());
    }

    let metadata = fs::symlink_metadata(&output_path)?;
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(&output_path)?;
    } else {
        fs::remove_file(&output_path)?;
    }

    Ok(())
}

pub fn open_dataset_summary(
    request: OpenDatasetRequest,
) -> Result<OpenDatasetResponse, Box<dyn std::error::Error>> {
    let store_path = request.store_path;
    Ok(OpenDatasetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: dataset_summary_for_path(&store_path)?,
    })
}

pub fn load_gather(
    store_path: String,
    request: GatherRequest,
) -> Result<GatherView, Box<dyn std::error::Error>> {
    let handle = open_prestack_store(&store_path)?;
    ensure_prestack_dataset_matches(&handle, &request.dataset_id.0)?;
    Ok(prestack_gather_view(&store_path, &request)?)
}

pub fn preview_processing(
    request: PreviewTraceLocalProcessingRequest,
) -> Result<PreviewTraceLocalProcessingResponse, Box<dyn std::error::Error>> {
    let handle = open_store(&request.store_path)?;
    ensure_dataset_matches(&handle, &request.section.dataset_id.0)?;
    let section = preview_processing_section_view(
        &request.store_path,
        request.section.axis,
        request.section.index,
        &request.pipeline,
    )?;
    Ok(PreviewTraceLocalProcessingResponse {
        schema_version: IPC_SCHEMA_VERSION,
        preview: PreviewView {
            section,
            processing_label: processing_label(&request.pipeline),
            preview_ready: true,
        },
        pipeline: request.pipeline,
    })
}

pub fn preview_gather_processing(
    request: PreviewGatherProcessingRequest,
) -> Result<PreviewGatherProcessingResponse, Box<dyn std::error::Error>> {
    let handle = open_prestack_store(&request.store_path)?;
    ensure_prestack_dataset_matches(&handle, &request.gather.dataset_id.0)?;
    let preview =
        preview_gather_processing_view(&request.store_path, &request.gather, &request.pipeline)?;
    Ok(PreviewGatherProcessingResponse {
        schema_version: IPC_SCHEMA_VERSION,
        preview,
        pipeline: request.pipeline,
    })
}

pub fn apply_processing(
    request: RunTraceLocalProcessingRequest,
) -> Result<DatasetSummary, Box<dyn std::error::Error>> {
    let pipeline = request.pipeline;
    let output_store = request
        .output_store_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_store_path(&request.store_path, &pipeline));
    prepare_processing_output_store(&output_store, request.overwrite_existing)?;
    let derived = materialize_processing_volume(
        &request.store_path,
        &output_store,
        &pipeline,
        MaterializeOptions::default(),
    )?;
    Ok(DatasetSummary {
        store_path: derived.root.to_string_lossy().into_owned(),
        descriptor: handle_for_summary(&derived)?,
    })
}

pub fn apply_gather_processing(
    request: RunGatherProcessingRequest,
) -> Result<DatasetSummary, Box<dyn std::error::Error>> {
    let pipeline = request.pipeline;
    let output_store = request
        .output_store_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_gather_output_store_path(&request.store_path, &pipeline));
    prepare_processing_output_store(&output_store, request.overwrite_existing)?;
    let derived =
        materialize_gather_processing_store(&request.store_path, &output_store, &pipeline)?;
    dataset_summary_for_path(&derived.root)
}

pub fn amplitude_spectrum(
    request: AmplitudeSpectrumRequest,
) -> Result<AmplitudeSpectrumResponse, Box<dyn std::error::Error>> {
    let handle = open_store(&request.store_path)?;
    ensure_dataset_matches(&handle, &request.section.dataset_id.0)?;
    let curve = amplitude_spectrum_from_store(
        &request.store_path,
        request.section.axis,
        request.section.index,
        request
            .pipeline
            .as_ref()
            .map(|pipeline| pipeline.operations.as_slice()),
        &request.selection,
    )?;

    Ok(AmplitudeSpectrumResponse {
        schema_version: IPC_SCHEMA_VERSION,
        section: request.section,
        selection: request.selection,
        sample_interval_ms: handle.volume_descriptor().sample_interval_ms,
        curve,
        processing_label: request.pipeline.as_ref().map(processing_label),
    })
}

pub fn run_velocity_scan(
    request: VelocityScanRequest,
) -> Result<VelocityScanResponse, Box<dyn std::error::Error>> {
    let handle = open_prestack_store(&request.store_path)?;
    ensure_prestack_dataset_matches(&handle, &request.gather.dataset_id.0)?;
    Ok(velocity_scan(request)?)
}

pub fn default_output_store_path(
    input_store_path: impl AsRef<Path>,
    pipeline: &TraceLocalProcessingPipeline,
) -> PathBuf {
    let input_store_path = input_store_path.as_ref();
    let parent = input_store_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = input_store_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("dataset");
    let suffix = pipeline_slug(pipeline);
    parent.join(format!("{stem}.{suffix}.tbvol"))
}

pub fn default_gather_output_store_path(
    input_store_path: impl AsRef<Path>,
    pipeline: &GatherProcessingPipeline,
) -> PathBuf {
    let input_store_path = input_store_path.as_ref();
    let parent = input_store_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = input_store_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("dataset");
    let suffix = gather_pipeline_slug(pipeline);
    parent.join(format!("{stem}.{suffix}.tbgath"))
}

fn dataset_summary_for_path(
    store_path: impl AsRef<Path>,
) -> Result<DatasetSummary, Box<dyn std::error::Error>> {
    let store_path = store_path.as_ref();
    let descriptor = match open_store(store_path) {
        Ok(_) => describe_store(store_path)?,
        Err(poststack_error) => match open_prestack_store(store_path) {
            Ok(_) => describe_prestack_store(store_path)?,
            Err(prestack_error) => {
                return Err(format!(
                    "failed to open dataset store as tbvol ({poststack_error}) or tbgath ({prestack_error})"
                )
                .into())
            }
        },
    };
    Ok(DatasetSummary {
        store_path: store_path.to_string_lossy().into_owned(),
        descriptor,
    })
}

fn suggested_action(action: seis_runtime::PreflightAction) -> SuggestedImportAction {
    match action {
        seis_runtime::PreflightAction::DirectDenseIngest => {
            SuggestedImportAction::DirectDenseIngest
        }
        seis_runtime::PreflightAction::RegularizeSparseSurvey => {
            SuggestedImportAction::RegularizeSparseSurvey
        }
        seis_runtime::PreflightAction::ReviewGeometryMapping => {
            SuggestedImportAction::ReviewGeometryMapping
        }
        seis_runtime::PreflightAction::UnsupportedInV1 => SuggestedImportAction::UnsupportedInV1,
    }
}

fn handle_for_summary(
    handle: &seis_runtime::StoreHandle,
) -> Result<seis_runtime::VolumeDescriptor, Box<dyn std::error::Error>> {
    Ok(describe_store(&handle.root)?)
}

fn ensure_dataset_matches(
    handle: &seis_runtime::StoreHandle,
    expected_dataset_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = handle.dataset_id().0;
    if actual != expected_dataset_id {
        return Err(format!(
            "Section request dataset mismatch: expected {expected_dataset_id}, found {actual}"
        )
        .into());
    }
    Ok(())
}

fn ensure_prestack_dataset_matches(
    handle: &seis_runtime::PrestackStoreHandle,
    expected_dataset_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = handle.dataset_id().0;
    if actual != expected_dataset_id {
        return Err(format!(
            "Gather request dataset mismatch: expected {expected_dataset_id}, found {actual}"
        )
        .into());
    }
    Ok(())
}

fn processing_label(pipeline: &TraceLocalProcessingPipeline) -> String {
    pipeline
        .name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| pipeline_slug(pipeline))
}

fn pipeline_slug(pipeline: &TraceLocalProcessingPipeline) -> String {
    let mut parts = Vec::with_capacity(pipeline.operations.len());
    for operation in &pipeline.operations {
        let label = match operation {
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
                store_path_slug(secondary_store_path)
            ),
        };
        parts.push(label);
    }
    if parts.is_empty() {
        "pipeline".to_string()
    } else {
        parts.join("__")
    }
}

fn gather_pipeline_slug(pipeline: &GatherProcessingPipeline) -> String {
    if let Some(name) = pipeline
        .name
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return name.replace(' ', "-").to_ascii_lowercase();
    }

    let mut parts = Vec::new();
    if let Some(trace_local_pipeline) = pipeline.trace_local_pipeline.as_ref() {
        parts.push(pipeline_slug(trace_local_pipeline));
    }
    for operation in &pipeline.operations {
        let label = match operation {
            seis_runtime::GatherProcessingOperation::NmoCorrection {
                velocity_model,
                interpolation,
            } => format!(
                "nmo-{}-{}",
                velocity_model_slug(velocity_model),
                interpolation_slug(*interpolation)
            ),
            seis_runtime::GatherProcessingOperation::StretchMute {
                velocity_model,
                max_stretch_ratio,
            } => format!(
                "stretch-mute-{}-{}",
                velocity_model_slug(velocity_model),
                format_factor(*max_stretch_ratio)
            ),
            seis_runtime::GatherProcessingOperation::OffsetMute {
                min_offset,
                max_offset,
            } => format!(
                "offset-mute-{}-{}",
                optional_factor_slug(*min_offset),
                optional_factor_slug(*max_offset)
            ),
        };
        parts.push(label);
    }
    if parts.is_empty() {
        "gather-processing".to_string()
    } else {
        parts.join("__")
    }
}

fn interpolation_slug(mode: GatherInterpolationMode) -> &'static str {
    match mode {
        GatherInterpolationMode::Linear => "linear",
    }
}

fn velocity_model_slug(model: &VelocityFunctionSource) -> String {
    match model {
        VelocityFunctionSource::ConstantVelocity { velocity_m_per_s } => {
            format!("constant-{}", format_factor(*velocity_m_per_s))
        }
        VelocityFunctionSource::TimeVelocityPairs { .. } => "time-velocity-pairs".to_string(),
        VelocityFunctionSource::VelocityAssetReference { asset_id } => {
            format!(
                "velocity-asset-{}",
                asset_id.replace(' ', "-").to_ascii_lowercase()
            )
        }
    }
}

fn optional_factor_slug(value: Option<f32>) -> String {
    value
        .map(format_factor)
        .unwrap_or_else(|| "none".to_string())
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

fn store_path_slug(store_path: &str) -> String {
    Path::new(store_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() {
                        ch.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
        })
        .map(|value| {
            value
                .split('-')
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
                .join("-")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "volume".to_string())
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

fn prestack_third_axis_field(field: PrestackThirdAxisField) -> HeaderField {
    match field {
        PrestackThirdAxisField::Offset => HeaderField::OFFSET,
    }
}

fn prepare_processing_output_store(
    output_path: &Path,
    overwrite_existing: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !output_path.exists() {
        return Ok(());
    }
    if !overwrite_existing {
        return Err(format!(
            "Output processing store already exists: {}",
            output_path.display()
        )
        .into());
    }
    let metadata = fs::symlink_metadata(output_path)?;
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(output_path)?;
    } else {
        fs::remove_file(output_path)?;
    }
    Ok(())
}
