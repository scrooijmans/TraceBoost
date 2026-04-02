use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

#[derive(Debug, Clone)]
pub struct AppPaths {
    logs_dir: PathBuf,
    pipeline_presets_dir: PathBuf,
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
        Ok(Self {
            logs_dir,
            pipeline_presets_dir,
        })
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }

    pub fn pipeline_presets_dir(&self) -> &Path {
        &self.pipeline_presets_dir
    }
}
