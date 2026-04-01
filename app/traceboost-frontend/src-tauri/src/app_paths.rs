use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

#[derive(Debug, Clone)]
pub struct AppPaths {
    logs_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve(app: &AppHandle) -> Result<Self, String> {
        let logs_dir = app
            .path()
            .app_log_dir()
            .map_err(|error| error.to_string())?;
        Ok(Self { logs_dir })
    }

    pub fn logs_dir(&self) -> &Path {
        &self.logs_dir
    }
}
