use seis_contracts_interop::{
    ImportDatasetRequest, ImportDatasetResponse, OpenDatasetRequest, OpenDatasetResponse,
    SurveyPreflightRequest, SurveyPreflightResponse, IPC_SCHEMA_VERSION,
};
use seis_runtime::{SectionAxis, SectionView, open_store};
use traceboost_app::{import_dataset, open_dataset_summary, preflight_dataset};

#[tauri::command]
fn preflight_import_command(input_path: String) -> Result<SurveyPreflightResponse, String> {
    preflight_dataset(SurveyPreflightRequest {
        schema_version: IPC_SCHEMA_VERSION,
        input_path,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn import_dataset_command(
    input_path: String,
    output_store_path: String,
) -> Result<ImportDatasetResponse, String> {
    import_dataset(ImportDatasetRequest {
        schema_version: IPC_SCHEMA_VERSION,
        input_path,
        output_store_path,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_dataset_command(store_path: String) -> Result<OpenDatasetResponse, String> {
    open_dataset_summary(OpenDatasetRequest {
        schema_version: IPC_SCHEMA_VERSION,
        store_path,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn load_section_command(
    store_path: String,
    axis: SectionAxis,
    index: usize,
) -> Result<SectionView, String> {
    open_store(store_path)
        .and_then(|handle| handle.section_view(axis, index))
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            preflight_import_command,
            import_dataset_command,
            open_dataset_command,
            load_section_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running traceboost desktop shell");
}
