mod app_paths;
mod diagnostics;
mod processing;

use seis_contracts_interop::{
    CancelProcessingJobRequest, CancelProcessingJobResponse, GetProcessingJobRequest,
    GetProcessingJobResponse, IPC_SCHEMA_VERSION, ImportDatasetRequest, ImportDatasetResponse,
    ListPipelinePresetsResponse, OpenDatasetRequest, OpenDatasetResponse,
    PreviewProcessingRequest, PreviewProcessingResponse, RunProcessingRequest,
    RunProcessingResponse, SavePipelinePresetRequest, SavePipelinePresetResponse,
    SurveyPreflightRequest, SurveyPreflightResponse, DeletePipelinePresetRequest,
    DeletePipelinePresetResponse,
};
use seis_runtime::{
    MaterializeOptions, SectionAxis, SectionView, materialize_processing_volume_with_progress,
    open_store,
};
use tauri::{AppHandle, Manager, State};
use traceboost_app::{
    default_output_store_path, import_dataset, open_dataset_summary, preflight_dataset,
    preview_processing,
};

use crate::app_paths::AppPaths;
use crate::diagnostics::{DiagnosticsState, ExportBundleResponse, build_fields, json_value};
use crate::processing::{JobRecord, ProcessingState};

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
            ("axis", json_value(format!("{:?}", request.section.axis).to_ascii_lowercase())),
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
fn run_processing_command(
    app: AppHandle,
    diagnostics: State<DiagnosticsState>,
    processing: State<ProcessingState>,
    request: RunProcessingRequest,
) -> Result<RunProcessingResponse, String> {
    let output_store_path = request
        .output_store_path
        .clone()
        .unwrap_or_else(|| {
            default_output_store_path(&request.store_path, &request.pipeline)
                .display()
                .to_string()
        });
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

fn run_processing_job(app: &AppHandle, record: &JobRecord, request: RunProcessingRequest) {
    let _ = record.mark_running();
    let output_store_path = request
        .output_store_path
        .clone()
        .unwrap_or_else(|| default_output_store_path(&request.store_path, &request.pipeline).display().to_string());
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
                    if matches!(final_status.state, seis_runtime::ProcessingJobState::Cancelled) {
                        log::Level::Warn
                    } else {
                        log::Level::Error
                    },
                    if matches!(final_status.state, seis_runtime::ProcessingJobState::Cancelled) {
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
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let app_paths = AppPaths::resolve(&app.handle().clone())?;
            let diagnostics =
                DiagnosticsState::initialize(app_paths.logs_dir(), session_basename.clone())?;
            let processing = ProcessingState::initialize(app_paths.pipeline_presets_dir())?;
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            preflight_import_command,
            import_dataset_command,
            open_dataset_command,
            load_section_command,
            preview_processing_command,
            run_processing_command,
            get_processing_job_command,
            cancel_processing_job_command,
            list_pipeline_presets_command,
            save_pipeline_preset_command,
            delete_pipeline_preset_command,
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
