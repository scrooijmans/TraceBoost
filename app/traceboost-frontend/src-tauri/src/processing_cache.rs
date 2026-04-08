use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

const CACHE_SCHEMA_VERSION: i64 = 1;
const SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingCachePolicy {
    Off,
    Balanced,
    Aggressive,
}

impl Default for ProcessingCachePolicy {
    fn default() -> Self {
        Self::Balanced
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessingCacheSettings {
    pub schema_version: u32,
    #[serde(default)]
    pub policy: ProcessingCachePolicy,
    #[serde(default)]
    pub max_cache_bytes_override: Option<u64>,
}

impl Default for ProcessingCacheSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            policy: ProcessingCachePolicy::Balanced,
            max_cache_bytes_override: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessingCacheStatus {
    pub settings: ProcessingCacheSettings,
    pub hidden_artifact_count: usize,
    pub hidden_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactArtifactHit {
    pub artifact_key: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixArtifactHit {
    pub artifact_key: String,
    pub path: String,
    pub prefix_len: usize,
}

pub struct ProcessingCacheState {
    cache_dir: PathBuf,
    volumes_dir: PathBuf,
    gathers_dir: PathBuf,
    tmp_dir: PathBuf,
    settings_path: PathBuf,
    settings: Mutex<ProcessingCacheSettings>,
    connection: Mutex<Connection>,
}

impl ProcessingCacheState {
    pub fn initialize(
        cache_dir: &Path,
        volumes_dir: &Path,
        gathers_dir: &Path,
        tmp_dir: &Path,
        index_path: &Path,
        settings_path: &Path,
    ) -> Result<Self, String> {
        fs::create_dir_all(cache_dir).map_err(|error| error.to_string())?;
        fs::create_dir_all(volumes_dir).map_err(|error| error.to_string())?;
        fs::create_dir_all(gathers_dir).map_err(|error| error.to_string())?;
        fs::create_dir_all(tmp_dir).map_err(|error| error.to_string())?;
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }

        cleanup_tmp_dir(tmp_dir)?;
        let settings = load_settings(settings_path)?;
        let connection = Connection::open(index_path).map_err(|error| error.to_string())?;
        initialize_schema(&connection)?;
        reconcile_hidden_artifacts(&connection)?;

        Ok(Self {
            cache_dir: cache_dir.to_path_buf(),
            volumes_dir: volumes_dir.to_path_buf(),
            gathers_dir: gathers_dir.to_path_buf(),
            tmp_dir: tmp_dir.to_path_buf(),
            settings_path: settings_path.to_path_buf(),
            settings: Mutex::new(settings),
            connection: Mutex::new(connection),
        })
    }

    pub fn settings(&self) -> ProcessingCacheSettings {
        self.settings
            .lock()
            .expect("processing cache settings mutex poisoned")
            .clone()
    }

    pub fn update_settings(
        &self,
        policy: Option<ProcessingCachePolicy>,
        max_cache_bytes_override: Option<Option<u64>>,
    ) -> Result<ProcessingCacheSettings, String> {
        let mut settings = self
            .settings
            .lock()
            .expect("processing cache settings mutex poisoned");
        if let Some(policy) = policy {
            settings.policy = policy;
        }
        if let Some(max_cache_bytes_override) = max_cache_bytes_override {
            settings.max_cache_bytes_override = max_cache_bytes_override;
        }
        persist_settings(&self.settings_path, &settings)?;
        Ok(settings.clone())
    }

    pub fn status(&self) -> Result<ProcessingCacheStatus, String> {
        let connection = self
            .connection
            .lock()
            .expect("processing cache connection mutex poisoned");
        let mut statement = connection
            .prepare(
                "SELECT COUNT(*), COALESCE(SUM(bytes), 0)
                 FROM artifacts
                 WHERE valid = 1 AND kind = 'hidden_prefix'",
            )
            .map_err(|error| error.to_string())?;
        let (hidden_artifact_count, hidden_bytes) = statement
            .query_row([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
            .map_err(|error| error.to_string())?;
        Ok(ProcessingCacheStatus {
            settings: self.settings(),
            hidden_artifact_count: hidden_artifact_count.max(0) as usize,
            hidden_bytes: hidden_bytes.max(0) as u64,
        })
    }

    pub fn reconcile(&self) -> Result<(), String> {
        let connection = self
            .connection
            .lock()
            .expect("processing cache connection mutex poisoned");
        reconcile_hidden_artifacts(&connection)
    }

    pub fn lookup_exact_visible_output(
        &self,
        family: &str,
        source_fingerprint: &str,
        full_pipeline_hash: &str,
    ) -> Result<Option<ExactArtifactHit>, String> {
        let connection = self
            .connection
            .lock()
            .expect("processing cache connection mutex poisoned");
        let mut statement = connection
            .prepare(
                "SELECT artifact_key, path
                 FROM artifacts
                 WHERE valid = 1
                   AND kind = 'visible_final'
                   AND family = ?1
                   AND source_fingerprint = ?2
                   AND full_pipeline_hash = ?3
                 ORDER BY last_accessed_at_unix_s DESC, created_at_unix_s DESC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(
                params![family, source_fingerprint, full_pipeline_hash],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .map_err(|error| error.to_string())?;

        let mut stale_keys = Vec::new();
        for row in rows {
            let (artifact_key, path) = row.map_err(|error| error.to_string())?;
            if Path::new(&path).exists() {
                connection
                    .execute(
                        "UPDATE artifacts
                         SET last_accessed_at_unix_s = ?2
                         WHERE artifact_key = ?1",
                        params![artifact_key, unix_timestamp_s() as i64],
                    )
                    .map_err(|error| error.to_string())?;
                return Ok(Some(ExactArtifactHit { artifact_key, path }));
            }
            stale_keys.push(artifact_key);
        }

        for artifact_key in stale_keys {
            connection
                .execute(
                    "UPDATE artifacts SET valid = 0 WHERE artifact_key = ?1",
                    params![artifact_key],
                )
                .map_err(|error| error.to_string())?;
        }

        Ok(None)
    }

    pub fn register_visible_output(
        &self,
        family: &str,
        path: &str,
        source_fingerprint: &str,
        full_pipeline_hash: &str,
        prefix_hash: &str,
        prefix_len: usize,
        runtime_version: &str,
        store_format_version: &str,
    ) -> Result<(), String> {
        let bytes = file_size_bytes(Path::new(path))?;
        let now = unix_timestamp_s() as i64;
        let artifact_key = Self::fingerprint_json(&serde_json::json!({
            "kind": "visible_final",
            "family": family,
            "path": normalized_path_key(path),
            "source_fingerprint": source_fingerprint,
            "full_pipeline_hash": full_pipeline_hash,
        }))?;
        let connection = self
            .connection
            .lock()
            .expect("processing cache connection mutex poisoned");
        connection
            .execute(
                "INSERT INTO artifacts (
                    artifact_key, kind, family, path, source_fingerprint, full_pipeline_hash,
                    prefix_hash, prefix_len, bytes, created_at_unix_s, last_accessed_at_unix_s,
                    protection_class, runtime_version, store_format_version, valid
                ) VALUES (
                    ?1, 'visible_final', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9,
                    'protected_visible_output', ?10, ?11, 1
                )
                ON CONFLICT(artifact_key) DO UPDATE SET
                    path = excluded.path,
                    bytes = excluded.bytes,
                    last_accessed_at_unix_s = excluded.last_accessed_at_unix_s,
                    runtime_version = excluded.runtime_version,
                    store_format_version = excluded.store_format_version,
                    valid = 1",
                params![
                    artifact_key,
                    family,
                    path,
                    source_fingerprint,
                    full_pipeline_hash,
                    prefix_hash,
                    prefix_len as i64,
                    bytes as i64,
                    now,
                    runtime_version,
                    store_format_version,
                ],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn lookup_prefix_artifact(
        &self,
        family: &str,
        source_fingerprint: &str,
        prefix_hash: &str,
        prefix_len: usize,
    ) -> Result<Option<PrefixArtifactHit>, String> {
        let connection = self
            .connection
            .lock()
            .expect("processing cache connection mutex poisoned");
        let mut statement = connection
            .prepare(
                "SELECT artifact_key, path, prefix_len
                 FROM artifacts
                 WHERE valid = 1
                   AND family = ?1
                   AND source_fingerprint = ?2
                   AND prefix_hash = ?3
                   AND prefix_len = ?4
                   AND kind IN ('hidden_prefix', 'visible_checkpoint')
                 ORDER BY last_accessed_at_unix_s DESC, created_at_unix_s DESC",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(
                params![family, source_fingerprint, prefix_hash, prefix_len as i64],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .map_err(|error| error.to_string())?;

        let mut stale_keys = Vec::new();
        for row in rows {
            let (artifact_key, path, stored_prefix_len) = row.map_err(|error| error.to_string())?;
            if Path::new(&path).exists() {
                connection
                    .execute(
                        "UPDATE artifacts
                         SET last_accessed_at_unix_s = ?2
                         WHERE artifact_key = ?1",
                        params![artifact_key, unix_timestamp_s() as i64],
                    )
                    .map_err(|error| error.to_string())?;
                return Ok(Some(PrefixArtifactHit {
                    artifact_key,
                    path,
                    prefix_len: stored_prefix_len.max(0) as usize,
                }));
            }
            stale_keys.push(artifact_key);
        }

        for artifact_key in stale_keys {
            connection
                .execute(
                    "UPDATE artifacts SET valid = 0 WHERE artifact_key = ?1",
                    params![artifact_key],
                )
                .map_err(|error| error.to_string())?;
        }

        Ok(None)
    }

    pub fn register_visible_checkpoint(
        &self,
        family: &str,
        path: &str,
        source_fingerprint: &str,
        prefix_hash: &str,
        prefix_len: usize,
        runtime_version: &str,
        store_format_version: &str,
    ) -> Result<(), String> {
        let bytes = file_size_bytes(Path::new(path))?;
        let now = unix_timestamp_s() as i64;
        let artifact_key = Self::fingerprint_json(&serde_json::json!({
            "kind": "visible_checkpoint",
            "family": family,
            "path": normalized_path_key(path),
            "source_fingerprint": source_fingerprint,
            "prefix_hash": prefix_hash,
            "prefix_len": prefix_len,
        }))?;
        let connection = self
            .connection
            .lock()
            .expect("processing cache connection mutex poisoned");
        connection
            .execute(
                "INSERT INTO artifacts (
                    artifact_key, kind, family, path, source_fingerprint, full_pipeline_hash,
                    prefix_hash, prefix_len, bytes, created_at_unix_s, last_accessed_at_unix_s,
                    protection_class, runtime_version, store_format_version, valid
                ) VALUES (
                    ?1, 'visible_checkpoint', ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?8,
                    'protected_checkpoint', ?9, ?10, 1
                )
                ON CONFLICT(artifact_key) DO UPDATE SET
                    path = excluded.path,
                    bytes = excluded.bytes,
                    last_accessed_at_unix_s = excluded.last_accessed_at_unix_s,
                    runtime_version = excluded.runtime_version,
                    store_format_version = excluded.store_format_version,
                    valid = 1",
                params![
                    artifact_key,
                    family,
                    path,
                    source_fingerprint,
                    prefix_hash,
                    prefix_len as i64,
                    bytes as i64,
                    now,
                    runtime_version,
                    store_format_version,
                ],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn register_hidden_prefix(
        &self,
        family: &str,
        path: &str,
        source_fingerprint: &str,
        prefix_hash: &str,
        prefix_len: usize,
        runtime_version: &str,
        store_format_version: &str,
    ) -> Result<(), String> {
        let bytes = file_size_bytes(Path::new(path))?;
        let now = unix_timestamp_s() as i64;
        let artifact_key = Self::fingerprint_json(&serde_json::json!({
            "kind": "hidden_prefix",
            "family": family,
            "path": normalized_path_key(path),
            "source_fingerprint": source_fingerprint,
            "prefix_hash": prefix_hash,
            "prefix_len": prefix_len,
        }))?;
        let connection = self
            .connection
            .lock()
            .expect("processing cache connection mutex poisoned");
        connection
            .execute(
                "INSERT INTO artifacts (
                    artifact_key, kind, family, path, source_fingerprint, full_pipeline_hash,
                    prefix_hash, prefix_len, bytes, created_at_unix_s, last_accessed_at_unix_s,
                    protection_class, runtime_version, store_format_version, valid
                ) VALUES (
                    ?1, 'hidden_prefix', ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?8,
                    'hidden_prefix', ?9, ?10, 1
                )
                ON CONFLICT(artifact_key) DO UPDATE SET
                    path = excluded.path,
                    bytes = excluded.bytes,
                    last_accessed_at_unix_s = excluded.last_accessed_at_unix_s,
                    runtime_version = excluded.runtime_version,
                    store_format_version = excluded.store_format_version,
                    valid = 1",
                params![
                    artifact_key,
                    family,
                    path,
                    source_fingerprint,
                    prefix_hash,
                    prefix_len as i64,
                    bytes as i64,
                    now,
                    runtime_version,
                    store_format_version,
                ],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn volumes_dir(&self) -> &Path {
        &self.volumes_dir
    }

    pub fn gathers_dir(&self) -> &Path {
        &self.gathers_dir
    }

    pub fn tmp_dir(&self) -> &Path {
        &self.tmp_dir
    }

    pub fn fingerprint_bytes(bytes: &[u8]) -> String {
        blake3::hash(bytes).to_hex().to_string()
    }

    pub fn fingerprint_json<T: Serialize>(value: &T) -> Result<String, String> {
        let payload = serde_json::to_vec(value).map_err(|error| error.to_string())?;
        Ok(Self::fingerprint_bytes(&payload))
    }
}

fn cleanup_tmp_dir(tmp_dir: &Path) -> Result<(), String> {
    for entry in fs::read_dir(tmp_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_dir() {
            fs::remove_dir_all(&path).map_err(|error| error.to_string())?;
        } else {
            fs::remove_file(&path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn file_size_bytes(path: &Path) -> Result<u64, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0u64;
    for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        total = total
            .checked_add(file_size_bytes(&entry.path())?)
            .ok_or_else(|| {
                format!(
                    "Processing cache artifact size overflow: {}",
                    path.display()
                )
            })?;
    }
    Ok(total)
}

fn normalized_path_key(path: &str) -> String {
    path.trim().replace('/', "\\").to_ascii_lowercase()
}

fn unix_timestamp_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn load_settings(settings_path: &Path) -> Result<ProcessingCacheSettings, String> {
    if !settings_path.exists() {
        let settings = ProcessingCacheSettings::default();
        persist_settings(settings_path, &settings)?;
        return Ok(settings);
    }

    let bytes = fs::read(settings_path).map_err(|error| error.to_string())?;
    let settings = serde_json::from_slice::<ProcessingCacheSettings>(&bytes)
        .unwrap_or_else(|_| ProcessingCacheSettings::default());
    if settings.schema_version != SETTINGS_SCHEMA_VERSION {
        let normalized = ProcessingCacheSettings {
            schema_version: SETTINGS_SCHEMA_VERSION,
            ..settings
        };
        persist_settings(settings_path, &normalized)?;
        return Ok(normalized);
    }
    Ok(settings)
}

fn persist_settings(
    settings_path: &Path,
    settings: &ProcessingCacheSettings,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(settings_path, bytes).map_err(|error| error.to_string())
}

fn initialize_schema(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS artifacts (
                artifact_key TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                family TEXT NOT NULL,
                path TEXT NOT NULL,
                source_fingerprint TEXT NOT NULL,
                full_pipeline_hash TEXT,
                prefix_hash TEXT,
                prefix_len INTEGER,
                bytes INTEGER NOT NULL DEFAULT 0,
                created_at_unix_s INTEGER NOT NULL,
                last_accessed_at_unix_s INTEGER NOT NULL,
                protection_class TEXT NOT NULL,
                runtime_version TEXT NOT NULL,
                store_format_version TEXT NOT NULL,
                valid INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS artifact_refs (
                artifact_key TEXT NOT NULL,
                dataset_entry_id TEXT,
                session_pipeline_id TEXT,
                ref_kind TEXT NOT NULL,
                updated_at_unix_s INTEGER NOT NULL,
                PRIMARY KEY (artifact_key, dataset_entry_id, session_pipeline_id, ref_kind),
                FOREIGN KEY (artifact_key) REFERENCES artifacts(artifact_key) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_artifacts_lookup
                ON artifacts (family, source_fingerprint, full_pipeline_hash, prefix_hash, valid);

            CREATE INDEX IF NOT EXISTS idx_artifacts_priority
                ON artifacts (valid, protection_class, last_accessed_at_unix_s);

            CREATE INDEX IF NOT EXISTS idx_artifact_refs_dataset
                ON artifact_refs (dataset_entry_id);

            CREATE INDEX IF NOT EXISTS idx_artifact_refs_pipeline
                ON artifact_refs (session_pipeline_id);
            ",
        )
        .map_err(|error| error.to_string())?;

    let current_version = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'cache_schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    if current_version.as_deref() != Some(&CACHE_SCHEMA_VERSION.to_string()) {
        connection
            .execute(
                "INSERT INTO metadata (key, value)
                 VALUES ('cache_schema_version', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![CACHE_SCHEMA_VERSION.to_string()],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn reconcile_hidden_artifacts(connection: &Connection) -> Result<(), String> {
    let mut statement = connection
        .prepare(
            "SELECT artifact_key, path FROM artifacts WHERE kind = 'hidden_prefix' AND valid = 1",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?;

    let mut missing = Vec::new();
    for row in rows {
        let (artifact_key, path) = row.map_err(|error| error.to_string())?;
        if !Path::new(&path).exists() {
            missing.push(artifact_key);
        }
    }

    for artifact_key in missing {
        connection
            .execute(
                "UPDATE artifacts SET valid = 0 WHERE artifact_key = ?1",
                params![artifact_key],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp_dir(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "traceboost-processing-cache-{}-{}",
            name,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp processing cache dir");
        base
    }

    #[test]
    fn initialize_creates_settings_and_schema() {
        let root = temp_dir("init");
        let cache = ProcessingCacheState::initialize(
            &root,
            &root.join("volumes"),
            &root.join("gathers"),
            &root.join("tmp"),
            &root.join("index.sqlite"),
            &root.join("settings.json"),
        )
        .expect("initialize processing cache");

        assert_eq!(cache.settings().policy, ProcessingCachePolicy::Balanced);
        assert!(cache.cache_dir().exists());
        assert!(cache.volumes_dir().exists());
        assert!(cache.gathers_dir().exists());
        assert!(cache.tmp_dir().exists());

        let status = cache.status().expect("load status");
        assert_eq!(status.hidden_artifact_count, 0);
        assert_eq!(status.hidden_bytes, 0);
    }

    #[test]
    fn update_settings_persists_values() {
        let root = temp_dir("settings");
        let cache = ProcessingCacheState::initialize(
            &root,
            &root.join("volumes"),
            &root.join("gathers"),
            &root.join("tmp"),
            &root.join("index.sqlite"),
            &root.join("settings.json"),
        )
        .expect("initialize processing cache");

        let updated = cache
            .update_settings(Some(ProcessingCachePolicy::Off), Some(Some(1024)))
            .expect("update settings");
        assert_eq!(updated.policy, ProcessingCachePolicy::Off);
        assert_eq!(updated.max_cache_bytes_override, Some(1024));

        let restored = load_settings(&root.join("settings.json")).expect("reload settings");
        assert_eq!(restored, updated);
    }

    #[test]
    fn reconcile_marks_missing_hidden_entries_invalid() {
        let root = temp_dir("reconcile");
        let cache = ProcessingCacheState::initialize(
            &root,
            &root.join("volumes"),
            &root.join("gathers"),
            &root.join("tmp"),
            &root.join("index.sqlite"),
            &root.join("settings.json"),
        )
        .expect("initialize processing cache");

        let connection = cache
            .connection
            .lock()
            .expect("processing cache connection mutex poisoned");
        connection
            .execute(
                "INSERT INTO artifacts (
                    artifact_key, kind, family, path, source_fingerprint, full_pipeline_hash,
                    prefix_hash, prefix_len, bytes, created_at_unix_s, last_accessed_at_unix_s,
                    protection_class, runtime_version, store_format_version, valid
                ) VALUES (?1, 'hidden_prefix', 'trace_local', ?2, 'source', 'full', 'prefix', 3,
                          512, 1, 1, 'normal', 'dev', 'tbvol-v1', 1)",
                params![
                    "missing-artifact",
                    root.join("volumes")
                        .join("missing.tbvol")
                        .display()
                        .to_string()
                ],
            )
            .expect("insert artifact");
        drop(connection);

        cache.reconcile().expect("reconcile cache");

        let connection = cache
            .connection
            .lock()
            .expect("processing cache connection mutex poisoned");
        let valid = connection
            .query_row(
                "SELECT valid FROM artifacts WHERE artifact_key = 'missing-artifact'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("select artifact validity");
        assert_eq!(valid, 0);
    }

    #[test]
    fn fingerprint_json_is_stable() {
        #[derive(Serialize)]
        struct Payload<'a> {
            name: &'a str,
            values: &'a [u32],
        }

        let left = ProcessingCacheState::fingerprint_json(&Payload {
            name: "pipeline",
            values: &[1, 2, 3],
        })
        .expect("fingerprint left");
        let right = ProcessingCacheState::fingerprint_json(&Payload {
            name: "pipeline",
            values: &[1, 2, 3],
        })
        .expect("fingerprint right");
        assert_eq!(left, right);
    }

    #[test]
    fn lookup_exact_visible_output_returns_latest_existing_path() {
        let root = temp_dir("lookup");
        let output = root.join("derived.tbvol");
        fs::create_dir_all(&output).expect("create derived output");
        fs::write(output.join("manifest.json"), b"{}").expect("write manifest");

        let cache = ProcessingCacheState::initialize(
            &root.join("cache"),
            &root.join("cache").join("volumes"),
            &root.join("cache").join("gathers"),
            &root.join("cache").join("tmp"),
            &root.join("cache").join("index.sqlite"),
            &root.join("settings.json"),
        )
        .expect("initialize processing cache");

        cache
            .register_visible_output(
                "trace_local",
                &output.display().to_string(),
                "source-a",
                "pipeline-a",
                "pipeline-a",
                4,
                "dev",
                "tbvol-v1",
            )
            .expect("register visible output");

        let hit = cache
            .lookup_exact_visible_output("trace_local", "source-a", "pipeline-a")
            .expect("lookup visible output")
            .expect("expected exact hit");
        assert_eq!(hit.path, output.display().to_string());
    }

    #[test]
    fn lookup_prefix_artifact_returns_visible_checkpoint() {
        let root = temp_dir("prefix");
        let checkpoint = root.join("checkpoint.tbvol");
        fs::create_dir_all(&checkpoint).expect("create checkpoint output");
        fs::write(checkpoint.join("manifest.json"), b"{}").expect("write checkpoint manifest");

        let cache = ProcessingCacheState::initialize(
            &root.join("cache"),
            &root.join("cache").join("volumes"),
            &root.join("cache").join("gathers"),
            &root.join("cache").join("tmp"),
            &root.join("cache").join("index.sqlite"),
            &root.join("settings.json"),
        )
        .expect("initialize processing cache");

        cache
            .register_visible_checkpoint(
                "trace_local",
                &checkpoint.display().to_string(),
                "source-a",
                "prefix-a",
                3,
                "dev",
                "tbvol-v1",
            )
            .expect("register visible checkpoint");

        let hit = cache
            .lookup_prefix_artifact("trace_local", "source-a", "prefix-a", 3)
            .expect("lookup prefix artifact")
            .expect("expected prefix hit");
        assert_eq!(hit.path, checkpoint.display().to_string());
        assert_eq!(hit.prefix_len, 3);
    }
}
