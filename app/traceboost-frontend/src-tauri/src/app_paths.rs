use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

#[derive(Debug, Clone)]
pub struct AppPaths {
    logs_dir: PathBuf,
    pipeline_presets_dir: PathBuf,
    imported_volumes_dir: PathBuf,
    derived_volumes_dir: PathBuf,
    dataset_registry_path: PathBuf,
    workspace_session_path: PathBuf,
}

impl AppPaths {
    pub fn resolve(app: &AppHandle) -> Result<Self, String> {
        let logs_dir = app
            .path()
            .app_log_dir()
            .map_err(|error| error.to_string())?;
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        let pipeline_presets_dir = app_data_dir.join("processing-pipelines");
        let imported_volumes_dir = app_data_dir.join("volumes");
        let derived_volumes_dir = app_data_dir.join("derived-volumes");
        let dataset_registry_path = app_data_dir.join("workspace").join("dataset-registry.json");
        let workspace_session_path = app_data_dir.join("workspace").join("session.json");
        Ok(Self {
            logs_dir,
            pipeline_presets_dir,
            imported_volumes_dir,
            derived_volumes_dir,
            dataset_registry_path,
            workspace_session_path,
        })
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    pub fn pipeline_presets_dir(&self) -> &Path {
        &self.pipeline_presets_dir
    }

    pub fn imported_volumes_dir(&self) -> &Path {
        &self.imported_volumes_dir
    }

    pub fn derived_volumes_dir(&self) -> &Path {
        &self.derived_volumes_dir
    }

    pub fn dataset_registry_path(&self) -> &Path {
        &self.dataset_registry_path
    }

    pub fn workspace_session_path(&self) -> &Path {
        &self.workspace_session_path
    }
}
