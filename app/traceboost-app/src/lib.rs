use std::{
    fs,
    path::{Path, PathBuf},
};

use seis_contracts_interop::{
    DatasetSummary, IPC_SCHEMA_VERSION, ImportDatasetRequest, ImportDatasetResponse,
    OpenDatasetRequest, OpenDatasetResponse, PreviewProcessingRequest, PreviewProcessingResponse,
    RunProcessingRequest, SuggestedImportAction, SurveyPreflightRequest, SurveyPreflightResponse,
};
use seis_runtime::{
    IngestOptions, MaterializeOptions, PreviewView, ProcessingPipeline, SparseSurveyPolicy,
    describe_store, ingest_segy, materialize_processing_volume, open_store, preflight_segy,
    preview_processing_section_view,
};

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
            sparse_survey_policy: SparseSurveyPolicy::Reject,
            ..IngestOptions::default()
        },
    )?;
    Ok(ImportDatasetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: DatasetSummary {
            store_path: handle.root.to_string_lossy().into_owned(),
            descriptor: describe_store(&output)?,
        },
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
    let descriptor = {
        let store = Path::new(&store_path);
        let _handle = open_store(store)?;
        describe_store(store)?
    };
    Ok(OpenDatasetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: DatasetSummary {
            store_path,
            descriptor,
        },
    })
}

pub fn preview_processing(
    request: PreviewProcessingRequest,
) -> Result<PreviewProcessingResponse, Box<dyn std::error::Error>> {
    let handle = open_store(&request.store_path)?;
    ensure_dataset_matches(&handle, &request.section.dataset_id.0)?;
    let section = preview_processing_section_view(
        &request.store_path,
        request.section.axis,
        request.section.index,
        &request.pipeline,
    )?;
    Ok(PreviewProcessingResponse {
        schema_version: IPC_SCHEMA_VERSION,
        preview: PreviewView {
            section,
            processing_label: processing_label(&request.pipeline),
            preview_ready: true,
        },
        pipeline: request.pipeline,
    })
}

pub fn apply_processing(
    request: RunProcessingRequest,
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

pub fn default_output_store_path(
    input_store_path: impl AsRef<Path>,
    pipeline: &ProcessingPipeline,
) -> PathBuf {
    let input_store_path = input_store_path.as_ref();
    let parent = input_store_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let stem = input_store_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("dataset");
    let suffix = pipeline_slug(pipeline);
    parent.join(format!("{stem}.{suffix}.tbvol"))
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

fn processing_label(pipeline: &ProcessingPipeline) -> String {
    pipeline
        .name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| pipeline_slug(pipeline))
}

fn pipeline_slug(pipeline: &ProcessingPipeline) -> String {
    let mut parts = Vec::with_capacity(pipeline.operations.len());
    for operation in &pipeline.operations {
        let label = match operation {
            seis_runtime::ProcessingOperation::AmplitudeScalar { factor } => {
                format!("amplitude-scalar-{}", format_factor(*factor))
            }
            seis_runtime::ProcessingOperation::TraceRmsNormalize => {
                "trace-rms-normalize".to_string()
            }
        };
        parts.push(label);
    }
    if parts.is_empty() {
        "pipeline".to_string()
    } else {
        parts.join("__")
    }
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
