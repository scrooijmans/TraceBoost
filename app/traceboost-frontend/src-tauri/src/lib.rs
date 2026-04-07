mod app_paths;
mod diagnostics;
mod processing;
mod workspace;

use seis_contracts_interop::{
    AmplitudeSpectrumRequest, AmplitudeSpectrumResponse, CancelProcessingJobRequest,
    CancelProcessingJobResponse, DeletePipelinePresetRequest, DeletePipelinePresetResponse,
    GetProcessingJobRequest, GetProcessingJobResponse, IPC_SCHEMA_VERSION, ImportDatasetRequest,
    ImportDatasetResponse, ListPipelinePresetsResponse, LoadWorkspaceStateResponse,
    OpenDatasetRequest, OpenDatasetResponse, PreviewProcessingRequest, PreviewProcessingResponse,
    RemoveDatasetEntryRequest, RemoveDatasetEntryResponse, RunProcessingRequest,
    RunProcessingResponse, SavePipelinePresetRequest, SavePipelinePresetResponse,
    SaveWorkspaceSessionRequest, SaveWorkspaceSessionResponse, SetActiveDatasetEntryRequest,
    SetActiveDatasetEntryResponse, SurveyPreflightRequest, SurveyPreflightResponse,
    UpsertDatasetEntryRequest, UpsertDatasetEntryResponse,
};
use seis_runtime::{
    MaterializeOptions, SectionAxis, SectionView, materialize_processing_volume_with_progress,
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
    amplitude_spectrum, import_dataset, open_dataset_summary, preflight_dataset,
    preview_processing,
};

use crate::app_paths::AppPaths;
use crate::diagnostics::{DiagnosticsState, ExportBundleResponse, build_fields, json_value};
use crate::processing::{JobRecord, ProcessingState};
use crate::workspace::WorkspaceState;

const FILE_OPEN_VOLUME_MENU_ID: &str = "file.open_volume";
const FILE_OPEN_VOLUME_MENU_EVENT: &str = "menu:file-open-volume";

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

fn pipeline_output_slug(pipeline: &seis_runtime::ProcessingPipeline) -> String {
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
        };
        parts.push(part);
    }

    if parts.is_empty() {
        "pipeline".to_string()
    } else {
        sanitized_stem(&parts.join("-"), "pipeline")
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

fn source_store_stem(store_path: &str) -> String {
    let path = Path::new(store_path);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("dataset");
    sanitized_stem(stem, "dataset")
}

fn unique_store_candidate(dir: &Path, base_name: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{base_name}.tbvol"));
    let mut index = 2usize;
    while candidate.exists() {
        candidate = dir.join(format!("{base_name}-{index}.tbvol"));
        index += 1;
    }
    candidate
}

fn default_processing_store_path(
    app_paths: &AppPaths,
    input_store_path: &str,
    pipeline: &seis_runtime::ProcessingPipeline,
) -> Result<String, String> {
    fs::create_dir_all(app_paths.derived_volumes_dir()).map_err(|error| error.to_string())?;
    let source_stem = source_store_stem(input_store_path);
    let pipeline_stem = pipeline_output_slug(pipeline);
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let base_name = format!("{source_stem}.{pipeline_stem}.{timestamp}");
    Ok(
        unique_store_candidate(app_paths.derived_volumes_dir(), &base_name)
            .display()
            .to_string(),
    )
}

fn import_store_path_for_input(app_paths: &AppPaths, input_path: &str) -> Result<String, String> {
    let input_path = input_path.trim();
    if input_path.is_empty() {
        return Err("Input path is required.".to_string());
    }

    fs::create_dir_all(app_paths.imported_volumes_dir()).map_err(|error| error.to_string())?;

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
    let store_name = format!("{sanitized_stem}-{fingerprint:016x}.tbvol");
    Ok(app_paths
        .imported_volumes_dir()
        .join(store_name)
        .display()
        .to_string())
}

#[tauri::command]
fn default_import_store_path_command(app: AppHandle, input_path: String) -> Result<String, String> {
    let app_paths = AppPaths::resolve(&app)?;
    import_store_path_for_input(&app_paths, &input_path)
}

#[tauri::command]
fn default_processing_store_path_command(
    app: AppHandle,
    store_path: String,
    pipeline: seis_runtime::ProcessingPipeline,
) -> Result<String, String> {
    let app_paths = AppPaths::resolve(&app)?;
    default_processing_store_path(&app_paths, &store_path, &pipeline)
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
fn preview_processing_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    request: PreviewProcessingRequest,
) -> Result<PreviewProcessingResponse, String> {
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
fn run_processing_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    processing: State<ProcessingState>,
    request: RunProcessingRequest,
) -> Result<RunProcessingResponse, String> {
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
        request.pipeline.clone(),
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
    let worker_request = RunProcessingRequest {
        output_store_path: Some(output_store_path.clone()),
        ..request
    };
    std::thread::spawn(move || {
        run_processing_job(&worker_app, &record, worker_request);
    });

    Ok(RunProcessingResponse {
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

fn run_processing_job(app: &AppHandle, record: &JobRecord, request: RunProcessingRequest) {
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
    let _ = record.mark_running();
    let output_store_path = request.output_store_path.clone().unwrap_or_else(|| {
        default_processing_store_path(&app_paths, &request.store_path, &request.pipeline)
            .unwrap_or_else(|_| "derived-output.tbvol".to_string())
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
    let result = materialize_processing_volume_with_progress(
        &request.store_path,
        &output_store_path,
        &request.pipeline,
        MaterializeOptions::default(),
        |completed, total| {
            if record.cancel_requested() {
                return Err(seis_runtime::SeisRefineError::Message(
                    "processing cancelled".to_string(),
                ));
            }
            let _ = record.mark_progress(completed, total);
            Ok(())
        },
    );

    match result {
        Ok(_) => {
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
            app.manage(workspace);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            preflight_import_command,
            import_dataset_command,
            open_dataset_command,
            load_section_command,
            preview_processing_command,
            amplitude_spectrum_command,
            run_processing_command,
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
            default_processing_store_path_command,
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
