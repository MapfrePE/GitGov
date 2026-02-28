use crate::models::{AuditAction, AuditStatus};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum OutboxError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEvent {
    pub event_uuid: String,
    pub event_type: String,
    pub user_login: String,
    pub user_name: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub files: Vec<String>,
    pub status: String,
    pub reason: Option<String>,
    pub repo_full_name: Option<String>,
    pub org_name: Option<String>,
    pub timestamp: i64,
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub sent: bool,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub last_attempt: Option<i64>,
}

impl OutboxEvent {
    pub fn new(
        event_type: String,
        user_login: String,
        branch: Option<String>,
        status: AuditStatus,
    ) -> Self {
        Self {
            event_uuid: Uuid::new_v4().to_string(),
            event_type,
            user_login,
            user_name: None,
            branch,
            commit_sha: None,
            files: vec![],
            status: status.as_str().to_string(),
            reason: None,
            repo_full_name: None,
            org_name: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
            metadata: None,
            sent: false,
            attempts: 0,
            last_attempt: None,
        }
    }

    pub fn from_audit_action(action: &AuditAction) -> String {
        match action {
            AuditAction::Push => "successful_push",
            AuditAction::BranchCreate => "create_branch",
            AuditAction::StageFile => "stage_files",
            AuditAction::Commit => "commit",
            AuditAction::BlockedPush => "blocked_push",
            AuditAction::BlockedBranch => "blocked_branch",
        }
        .to_string()
    }

    pub fn with_user_name(mut self, name: String) -> Self {
        self.user_name = Some(name);
        self
    }

    pub fn with_commit_sha(mut self, sha: String) -> Self {
        self.commit_sha = Some(sha);
        self
    }

    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files = files;
        self
    }

    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    pub fn with_repo(mut self, full_name: String) -> Self {
        self.repo_full_name = Some(full_name);
        self
    }

    pub fn with_org(mut self, org: String) -> Self {
        self.org_name = Some(org);
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Worker control for clean shutdown
struct WorkerControl {
    shutdown: AtomicBool,
    trigger: Condvar,
}

/// Outbox implementation with CRASH-SAFE persistence and NO nested locks.
///
/// CRASH-SAFETY: Uses atomic rename pattern:
///   1. Write complete file to .tmp
///   2. Sync to disk
///   3. Atomic rename to final location
///
/// On crash: Either old file exists (complete) or new file exists (complete)
///
/// LOCK DISCIPLINE: Never hold two locks simultaneously.
/// Pattern: snapshot → release → work → update → release → persist
pub struct Outbox {
    path: PathBuf,
    events: Arc<Mutex<Vec<OutboxEvent>>>,
    file_lock: Arc<Mutex<()>>,
    worker_control: Arc<WorkerControl>,
    server_url: Arc<Mutex<Option<String>>>,
    api_key: Arc<Mutex<Option<String>>>,
    max_retries: u32,
}

impl Outbox {
    pub fn new(app_data_dir: &std::path::Path) -> Result<Self, OutboxError> {
        let path = app_data_dir.join("outbox.jsonl");
        let events = Self::load_events(&path)?;

        Ok(Self {
            path,
            events: Arc::new(Mutex::new(events)),
            file_lock: Arc::new(Mutex::new(())),
            worker_control: Arc::new(WorkerControl {
                shutdown: AtomicBool::new(false),
                trigger: Condvar::new(),
            }),
            server_url: Arc::new(Mutex::new(None)),
            api_key: Arc::new(Mutex::new(None)),
            max_retries: 5,
        })
    }

    pub fn with_server(self, url: String, api_key: Option<String>) -> Self {
        if let Ok(mut server_url) = self.server_url.lock() {
            *server_url = Some(url);
        }
        if let Ok(mut key) = self.api_key.lock() {
            *key = api_key;
        }
        self
    }

    pub fn set_server_config(&self, server_url: Option<String>, api_key: Option<String>) {
        if let Ok(mut url_guard) = self.server_url.lock() {
            *url_guard = server_url;
        }
        if let Ok(mut key_guard) = self.api_key.lock() {
            *key_guard = api_key;
        }
        // Wake worker so it can flush promptly with the new config.
        self.worker_control.trigger.notify_all();
    }

    fn load_events(path: &PathBuf) -> Result<Vec<OutboxEvent>, OutboxError> {
        if !path.exists() {
            return Ok(vec![]);
        }

        let file = File::open(path).map_err(|e| OutboxError::IoError(e.to_string()))?;

        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| OutboxError::IoError(e.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<OutboxEvent>(&line) {
                Ok(event) if !event.sent => events.push(event),
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Failed to parse outbox event: {}", e);
                }
            }
        }

        Ok(events)
    }

    /// CRASH-SAFE persist using atomic rename pattern.
    ///
    /// Pattern:
    ///   1. Take events snapshot (events lock only)
    ///   2. Write to .tmp file
    ///   3. Sync to disk (ensures data on storage)
    ///   4. Atomic rename .tmp → final (or safe Windows fallback)
    ///   5. Cleanup backup if exists
    fn persist(&self) -> Result<(), OutboxError> {
        // Step 1: Snapshot events (only events lock, NO file_lock)
        let snapshot: Vec<String> = {
            let events = self
                .events
                .lock()
                .map_err(|_| OutboxError::IoError("Events lock poisoned".to_string()))?;

            events
                .iter()
                .map(serde_json::to_string)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| OutboxError::SerializationError(e.to_string()))?
        };
        // events lock released here

        // Step 2-5: Write atomically (only file_lock, NO events lock)
        {
            let _file_guard = self
                .file_lock
                .lock()
                .map_err(|_| OutboxError::IoError("File lock poisoned".to_string()))?;

            let tmp_path = self.path.with_extension("jsonl.tmp");

            // Write to temp file
            {
                let mut tmp_file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&tmp_path)
                    .map_err(|e| {
                        OutboxError::IoError(format!("Failed to create temp file: {}", e))
                    })?;

                for json in snapshot.iter() {
                    writeln!(tmp_file, "{}", json).map_err(|e| {
                        OutboxError::IoError(format!("Failed to write temp file: {}", e))
                    })?;
                }

                // CRITICAL: Sync to disk before rename
                tmp_file.sync_all().map_err(|e| {
                    OutboxError::IoError(format!("Failed to sync temp file: {}", e))
                })?;
            }

            // Atomic replace strategy (cross-platform safe)
            // On Unix: rename is atomic
            // On Windows: need to backup original first
            #[cfg(unix)]
            {
                std::fs::rename(&tmp_path, &self.path)
                    .map_err(|e| OutboxError::IoError(format!("Failed to rename: {}", e)))?;
            }

            #[cfg(windows)]
            {
                let bak_path = self.path.with_extension("jsonl.bak");
                // Windows: backup original, rename tmp, delete backup
                if self.path.exists() {
                    let _ = std::fs::rename(&self.path, &bak_path);
                }

                if let Err(e) = std::fs::rename(&tmp_path, &self.path) {
                    // Rollback: restore backup
                    if bak_path.exists() {
                        let _ = std::fs::rename(&bak_path, &self.path);
                    }
                    return Err(OutboxError::IoError(format!(
                        "Failed to rename on Windows: {}",
                        e
                    )));
                }

                // Cleanup backup
                if bak_path.exists() {
                    let _ = std::fs::remove_file(&bak_path);
                }
            }

            tracing::trace!("Persisted {} events to outbox", snapshot.len());
        }
        // file_lock released here

        Ok(())
    }

    /// Add event to outbox. NEVER holds both locks simultaneously.
    pub fn add(&self, event: OutboxEvent) -> Result<String, OutboxError> {
        let event_uuid = event.event_uuid.clone();

        // Step 1: Add to memory (only events lock)
        {
            let mut events = self
                .events
                .lock()
                .map_err(|_| OutboxError::IoError("Events lock poisoned".to_string()))?;
            events.push(event);
        }

        // Step 2: Persist (separate lock discipline)
        if let Err(e) = self.persist() {
            tracing::error!(
                "Failed to persist outbox event: {}. Event will be lost if app crashes.",
                e
            );
        }

        // Step 3: Wake up worker if waiting
        self.worker_control.trigger.notify_all();

        Ok(event_uuid)
    }

    pub fn get_pending_count(&self) -> usize {
        self.events
            .lock()
            .map(|e| e.iter().filter(|ev| !ev.sent).count())
            .unwrap_or(0)
    }

    /// Flush pending events. NEVER holds both locks simultaneously.
    pub fn flush(&self) -> Result<FlushResult, OutboxError> {
        let server_url = match self.server_url.lock() {
            Ok(guard) => match &*guard {
                Some(url) => url.clone(),
                None => {
                    return Err(OutboxError::NetworkError(
                        "Server URL not configured".to_string(),
                    ))
                }
            },
            Err(_) => {
                return Err(OutboxError::IoError(
                    "Server URL lock poisoned".to_string(),
                ))
            }
        };

        let api_key = match self.api_key.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => {
                return Err(OutboxError::IoError("API key lock poisoned".to_string()))
            }
        };

        // Step 1: Snapshot pending events
        let events_to_send: Vec<OutboxEvent> = {
            let events = self
                .events
                .lock()
                .map_err(|_| OutboxError::IoError("Events lock poisoned".to_string()))?;
            events.iter().filter(|e| !e.sent).cloned().collect()
        };

        if events_to_send.is_empty() {
            return Ok(FlushResult {
                sent: 0,
                duplicates: 0,
                failed: 0,
            });
        }

        // Step 2: Build batch
        let batch = ClientEventBatch {
            events: events_to_send
                .iter()
                .map(|e| ClientEventInput {
                    event_uuid: e.event_uuid.clone(),
                    event_type: e.event_type.clone(),
                    org_name: e.org_name.clone(),
                    repo_full_name: e.repo_full_name.clone(),
                    user_login: e.user_login.clone(),
                    user_name: e.user_name.clone(),
                    branch: e.branch.clone(),
                    commit_sha: e.commit_sha.clone(),
                    files: e.files.clone(),
                    status: e.status.clone(),
                    reason: e.reason.clone(),
                    metadata: e.metadata.clone(),
                    timestamp: Some(e.timestamp),
                })
                .collect(),
            client_id: None,
            client_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        };

        // Step 3: Send to server
        let response = self.send_batch(&server_url, api_key.as_deref(), &batch)?;

        let sent_count = response.accepted.len();
        let dup_count = response.duplicates.len();
        let failed_count = response.errors.len();

        // Step 4: Update state
        {
            let mut events = self
                .events
                .lock()
                .map_err(|_| OutboxError::IoError("Events lock poisoned".to_string()))?;

            for event in events.iter_mut() {
                if response.accepted.contains(&event.event_uuid)
                    || response.duplicates.contains(&event.event_uuid)
                {
                    event.sent = true;
                } else if response
                    .errors
                    .iter()
                    .any(|e| e.event_uuid == event.event_uuid)
                {
                    event.attempts += 1;
                    event.last_attempt = Some(chrono::Utc::now().timestamp_millis());
                }
            }

            events.retain(|e| !e.sent || e.attempts < self.max_retries);
        }

        // Step 5: Persist
        self.persist()?;

        Ok(FlushResult {
            sent: sent_count,
            duplicates: dup_count,
            failed: failed_count,
        })
    }

    fn send_batch(
        &self,
        server_url: &str,
        api_key: Option<&str>,
        batch: &ClientEventBatch,
    ) -> Result<ClientEventResponse, OutboxError> {
        let url = format!("{}/events", server_url);

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| OutboxError::NetworkError(e.to_string()))?;

        let mut request = client.post(&url).json(batch);

        if let Some(api_key) = api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .map_err(|e| OutboxError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(OutboxError::NetworkError(format!(
                "Server returned status: {}",
                response.status()
            )));
        }

        response
            .json()
            .map_err(|e| OutboxError::SerializationError(e.to_string()))
    }

    /// Start background flush worker with CLEAN SHUTDOWN support.
    ///
    /// Returns JoinHandle for clean termination.
    /// Uses Condvar for fast wake-up on shutdown (no 60s wait).
    pub fn start_background_flush(&self, interval_secs: u64) -> std::thread::JoinHandle<()> {
        let events = Arc::clone(&self.events);
        let file_lock = Arc::clone(&self.file_lock);
        let worker_control = Arc::clone(&self.worker_control);
        let path = self.path.clone();
        let server_url = Arc::clone(&self.server_url);
        let api_key = Arc::clone(&self.api_key);
        let max_retries = self.max_retries;

        std::thread::spawn(move || {
            let mut last_flush = std::time::Instant::now();
            let interval = Duration::from_secs(interval_secs);

            loop {
                // Wait with timeout, wakeable by shutdown signal
                let events_lock = events.lock().unwrap();
                let (lock, _result) = worker_control
                    .trigger
                    .wait_timeout(events_lock, Duration::from_secs(1))
                    .unwrap();
                drop(lock);

                // Check shutdown
                if worker_control.shutdown.load(Ordering::Relaxed) {
                    tracing::info!("Outbox worker shutting down gracefully");
                    return;
                }

                // Check if it's time to flush
                let now = std::time::Instant::now();
                if now.duration_since(last_flush) < interval {
                    continue;
                }

                last_flush = now;

                // Check pending count
                let pending_count = events
                    .lock()
                    .map(|e| e.iter().filter(|ev| !ev.sent).count())
                    .unwrap_or(0);

                if pending_count == 0 {
                    continue;
                }

                // Snapshot pending events
                let events_to_send: Vec<OutboxEvent> = {
                    let events_lock = events.lock().unwrap();
                    events_lock.iter().filter(|e| !e.sent).cloned().collect()
                };

                if events_to_send.is_empty() {
                    continue;
                }

                // Build and send batch
                let url_opt = server_url.lock().ok().and_then(|g| (*g).clone());
                let api_key_opt = api_key.lock().ok().and_then(|g| (*g).clone());

                if let (Some(ref url), Ok(client)) = (
                    &url_opt,
                    reqwest::blocking::Client::builder()
                        .timeout(Duration::from_secs(30))
                        .build(),
                ) {
                    let batch = ClientEventBatch {
                        events: events_to_send
                            .iter()
                            .map(|e| ClientEventInput {
                                event_uuid: e.event_uuid.clone(),
                                event_type: e.event_type.clone(),
                                org_name: e.org_name.clone(),
                                repo_full_name: e.repo_full_name.clone(),
                                user_login: e.user_login.clone(),
                                user_name: e.user_name.clone(),
                                branch: e.branch.clone(),
                                commit_sha: e.commit_sha.clone(),
                                files: e.files.clone(),
                                status: e.status.clone(),
                                reason: e.reason.clone(),
                                metadata: e.metadata.clone(),
                                timestamp: Some(e.timestamp),
                            })
                            .collect(),
                        client_id: None,
                        client_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                    };

                    let mut request = client.post(format!("{}/events", url)).json(&batch);
                    if let Some(ref key) = api_key_opt {
                        request = request.header("Authorization", format!("Bearer {}", key));
                    }

                    match request.send() {
                        Ok(response) if response.status().is_success() => {
                            if let Ok(batch_response) = response.json::<ClientEventResponse>() {
                                // Update state and persist atomically
                                let snapshot_for_persist: Vec<String> = {
                                    let mut events_lock = events.lock().unwrap();
                                    for event in events_lock.iter_mut() {
                                        if batch_response.accepted.contains(&event.event_uuid)
                                            || batch_response.duplicates.contains(&event.event_uuid)
                                        {
                                            event.sent = true;
                                        } else if batch_response
                                            .errors
                                            .iter()
                                            .any(|e| e.event_uuid == event.event_uuid)
                                        {
                                            event.attempts += 1;
                                            event.last_attempt =
                                                Some(chrono::Utc::now().timestamp_millis());
                                        }
                                    }
                                    events_lock.retain(|e| !e.sent || e.attempts < max_retries);

                                    events_lock
                                        .iter()
                                        .filter_map(|e| serde_json::to_string(e).ok())
                                        .collect()
                                };

                                // Atomic persist
                                if let Err(e) = Self::atomic_persist_static(
                                    &path,
                                    &file_lock,
                                    &snapshot_for_persist,
                                ) {
                                    tracing::error!("Failed to persist outbox: {}", e);
                                }
                            }
                        }
                        Ok(response) => {
                            tracing::warn!(
                                url = %format!("{}/events", url),
                                status = %response.status(),
                                "Outbox flush failed"
                            );
                        }
                        Err(e) => {
                            tracing::warn!("Outbox flush network error: {}", e);
                        }
                    }
                }
            }
        })
    }

    /// Signal shutdown to worker
    pub fn signal_shutdown(&self) {
        self.worker_control.shutdown.store(true, Ordering::Relaxed);
        self.worker_control.trigger.notify_all();
    }

    /// Static atomic persist for use in worker
    fn atomic_persist_static(
        path: &PathBuf,
        file_lock: &Arc<Mutex<()>>,
        snapshot: &[String],
    ) -> Result<(), OutboxError> {
        let _file_guard = file_lock
            .lock()
            .map_err(|_| OutboxError::IoError("File lock poisoned".to_string()))?;

        let tmp_path = path.with_extension("jsonl.tmp");

        // Write to temp
        {
            let mut tmp_file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|e| OutboxError::IoError(format!("Temp file: {}", e)))?;

            for json in snapshot.iter() {
                writeln!(tmp_file, "{}", json)
                    .map_err(|e| OutboxError::IoError(format!("Write: {}", e)))?;
            }

            tmp_file
                .sync_all()
                .map_err(|e| OutboxError::IoError(format!("Sync: {}", e)))?;
        }

        // Atomic replace
        #[cfg(unix)]
        {
            std::fs::rename(&tmp_path, path)
                .map_err(|e| OutboxError::IoError(format!("Rename: {}", e)))?;
        }

        #[cfg(windows)]
        {
            let bak_path = path.with_extension("jsonl.bak");
            if path.exists() {
                let _ = std::fs::rename(path, &bak_path);
            }

            if let Err(e) = std::fs::rename(&tmp_path, path) {
                if bak_path.exists() {
                    let _ = std::fs::rename(&bak_path, path);
                }
                return Err(OutboxError::IoError(format!("Windows rename: {}", e)));
            }

            if bak_path.exists() {
                let _ = std::fs::remove_file(&bak_path);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEventBatch {
    pub events: Vec<ClientEventInput>,
    pub client_id: Option<String>,
    pub client_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEventInput {
    pub event_uuid: String,
    pub event_type: String,
    pub org_name: Option<String>,
    pub repo_full_name: Option<String>,
    pub user_login: String,
    pub user_name: Option<String>,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub files: Vec<String>,
    pub status: String,
    pub reason: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEventResponse {
    pub accepted: Vec<String>,
    pub duplicates: Vec<String>,
    pub errors: Vec<EventError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventError {
    pub event_uuid: String,
    pub error: String,
}

#[derive(Debug)]
pub struct FlushResult {
    pub sent: usize,
    pub duplicates: usize,
    pub failed: usize,
}
