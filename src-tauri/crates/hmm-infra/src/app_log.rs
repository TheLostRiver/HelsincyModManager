use anyhow::{Context, Result};
use cap_fs_ext::{DirExt as _, FollowSymlinks, OpenOptionsFollowExt as _};
use cap_std::ambient_authority;
#[cfg(windows)]
use cap_std::fs::MetadataExt as _;
use cap_std::fs::{Dir, DirBuilder, File, Metadata, OpenOptions};
#[cfg(unix)]
use cap_std::fs::{DirBuilderExt as _, OpenOptionsExt as _, Permissions, PermissionsExt as _};
use serde::Serialize;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context as LayerContext;
use tracing_subscriber::prelude::*;
use tracing_subscriber::Layer;

pub const SAFE_APP_LOG_TARGET: &str = "hmm.safe_app_log";
const APP_LOG_RETENTION_DAYS: i64 = 14;
const MILLIS_PER_DAY: u128 = 86_400_000;
const MAX_CODE_LENGTH: usize = 96;
const MAX_ID_LENGTH: usize = 160;
const MAX_SAFE_PATH_LENGTH: usize = 512;
#[cfg(unix)]
const APP_LOG_DIRECTORY_MODE: u32 = 0o700;
#[cfg(unix)]
const APP_LOG_FILE_MODE: u32 = 0o600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct AppLogHealth {
    failure: Arc<AtomicU8>,
}

impl AppLogHealth {
    pub fn ready() -> Self {
        Self {
            failure: Arc::new(AtomicU8::new(0)),
        }
    }

    pub fn initialization_failed() -> Self {
        let health = Self::ready();
        health.degrade(AppLogFailure::InitializationFailed);
        health
    }

    pub fn status_code(&self) -> &'static str {
        match self.failure.load(Ordering::Acquire) {
            0 => "ok",
            1 => "app_log_event_rejected",
            2 => "app_log_retention_failed",
            3 => "app_log_write_failed",
            _ => "app_log_initialization_failed",
        }
    }

    fn degrade(&self, failure: AppLogFailure) {
        self.failure.fetch_max(failure as u8, Ordering::AcqRel);
    }
}

impl Default for AppLogHealth {
    fn default() -> Self {
        Self::ready()
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppLogFailure {
    EventRejected = 1,
    RetentionFailed = 2,
    WriteFailed = 3,
    InitializationFailed = 4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppLogEvent {
    level: AppLogLevel,
    event_name: String,
    task_id: Option<String>,
    game_id: Option<String>,
    profile_id: Option<String>,
    mod_id: Option<String>,
    task_kind: Option<String>,
    task_status: Option<String>,
    phase: Option<String>,
    operation: Option<String>,
    result: Option<String>,
    error_code: Option<String>,
    safe_path: Option<String>,
    item_count: Option<u64>,
    duration_ms: Option<u64>,
}

impl AppLogEvent {
    pub fn info(event_name: impl Into<String>) -> Self {
        Self::new(AppLogLevel::Info, event_name)
    }

    pub fn warning(event_name: impl Into<String>) -> Self {
        Self::new(AppLogLevel::Warn, event_name)
    }

    pub fn error(event_name: impl Into<String>) -> Self {
        Self::new(AppLogLevel::Error, event_name)
    }

    pub fn new(level: AppLogLevel, event_name: impl Into<String>) -> Self {
        Self {
            level,
            event_name: event_name.into(),
            task_id: None,
            game_id: None,
            profile_id: None,
            mod_id: None,
            task_kind: None,
            task_status: None,
            phase: None,
            operation: None,
            result: None,
            error_code: None,
            safe_path: None,
            item_count: None,
            duration_ms: None,
        }
    }

    pub fn with_task_id(mut self, value: impl Into<String>) -> Self {
        self.task_id = Some(value.into());
        self
    }

    pub fn with_game_id(mut self, value: impl Into<String>) -> Self {
        self.game_id = Some(value.into());
        self
    }

    pub fn with_profile_id(mut self, value: impl Into<String>) -> Self {
        self.profile_id = Some(value.into());
        self
    }

    pub fn with_mod_id(mut self, value: impl Into<String>) -> Self {
        self.mod_id = Some(value.into());
        self
    }

    pub fn with_task_kind(mut self, value: impl Into<String>) -> Self {
        self.task_kind = Some(value.into());
        self
    }

    pub fn with_task_status(mut self, value: impl Into<String>) -> Self {
        self.task_status = Some(value.into());
        self
    }

    pub fn with_phase(mut self, value: impl Into<String>) -> Self {
        self.phase = Some(value.into());
        self
    }

    pub fn with_operation(mut self, value: impl Into<String>) -> Self {
        self.operation = Some(value.into());
        self
    }

    pub fn with_result(mut self, value: impl Into<String>) -> Self {
        self.result = Some(value.into());
        self
    }

    pub fn with_error_code(mut self, value: impl Into<String>) -> Self {
        self.error_code = Some(value.into());
        self
    }

    pub fn with_safe_path(mut self, value: impl Into<String>) -> Self {
        self.safe_path = Some(value.into());
        self
    }

    pub fn with_item_count(mut self, value: u64) -> Self {
        self.item_count = Some(value);
        self
    }

    pub fn with_duration_ms(mut self, value: u64) -> Self {
        self.duration_ms = Some(value);
        self
    }
}

pub fn redact_sensitive_text(value: &str) -> String {
    if contains_sensitive_text(value) || looks_like_path_text(value) {
        "[redacted]".to_owned()
    } else {
        value.to_owned()
    }
}

pub fn emit_safe_app_log(event: AppLogEvent) {
    macro_rules! emit_at_level {
        ($level:expr) => {
            tracing::event!(
                target: SAFE_APP_LOG_TARGET,
                $level,
                event_name = event.event_name.as_str(),
                task_id = event.task_id.as_deref(),
                game_id = event.game_id.as_deref(),
                profile_id = event.profile_id.as_deref(),
                mod_id = event.mod_id.as_deref(),
                task_kind = event.task_kind.as_deref(),
                task_status = event.task_status.as_deref(),
                phase = event.phase.as_deref(),
                operation = event.operation.as_deref(),
                result = event.result.as_deref(),
                error_code = event.error_code.as_deref(),
                safe_path = event.safe_path.as_deref(),
                item_count = event.item_count,
                duration_ms = event.duration_ms,
            );
        };
    }

    match event.level {
        AppLogLevel::Info => {
            emit_at_level!(Level::INFO);
        }
        AppLogLevel::Warn => {
            emit_at_level!(Level::WARN);
        }
        AppLogLevel::Error => {
            emit_at_level!(Level::ERROR);
        }
    }
}

pub fn initialize_app_logging(app_data_root: &Path) -> AppLogHealth {
    let health = AppLogHealth::ready();
    let clock: Arc<dyn AppLogClock> = Arc::new(SystemAppLogClock);
    let Some(layer) = prepare_app_log_layer(app_data_root, Arc::clone(&clock), health.clone())
    else {
        return health;
    };
    let subscriber = tracing_subscriber::registry().with(layer);
    if tracing::subscriber::set_global_default(subscriber).is_err() {
        health.degrade(AppLogFailure::InitializationFailed);
    }
    health
}

fn prepare_app_log_layer(
    app_data_root: &Path,
    clock: Arc<dyn AppLogClock>,
    health: AppLogHealth,
) -> Option<SafeAppLogLayer> {
    match SafeAppLogLayer::new(app_data_root.to_path_buf(), clock, health.clone()) {
        Ok(layer) => Some(layer),
        Err(_) => {
            health.degrade(AppLogFailure::InitializationFailed);
            None
        }
    }
}

trait AppLogClock: Send + Sync {
    fn now_unix_millis(&self) -> Result<u128>;
}

struct SystemAppLogClock;

impl AppLogClock for SystemAppLogClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
    }
}

struct SafeAppLogLayer {
    writer: FileSystemAppLogWriter,
    clock: Arc<dyn AppLogClock>,
    health: AppLogHealth,
}

impl SafeAppLogLayer {
    fn new(
        app_data_root: PathBuf,
        clock: Arc<dyn AppLogClock>,
        health: AppLogHealth,
    ) -> Result<Self> {
        let writer = FileSystemAppLogWriter::new(app_data_root, APP_LOG_RETENTION_DAYS)?;
        writer.prepare(clock.now_unix_millis()?)?;
        Ok(Self {
            writer,
            clock,
            health,
        })
    }
}

impl<S> Layer<S> for SafeAppLogLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _context: LayerContext<'_, S>) {
        if event.metadata().target() != SAFE_APP_LOG_TARGET {
            return;
        }

        let mut visitor = AppLogEventVisitor::default();
        event.record(&mut visitor);
        let timestamp = match self.clock.now_unix_millis() {
            Ok(timestamp) => timestamp,
            Err(_) => {
                self.health.degrade(AppLogFailure::WriteFailed);
                return;
            }
        };
        let record = match visitor.into_validated_record(event.metadata().level(), timestamp) {
            Ok(record) => record,
            Err(_) => {
                self.health.degrade(AppLogFailure::EventRejected);
                return;
            }
        };
        if let Err(error) = self.writer.write(&record) {
            self.health.degrade(error.failure());
        }
    }
}

#[derive(Default)]
struct AppLogEventVisitor {
    invalid: bool,
    event_name: Option<String>,
    task_id: Option<String>,
    game_id: Option<String>,
    profile_id: Option<String>,
    mod_id: Option<String>,
    task_kind: Option<String>,
    task_status: Option<String>,
    phase: Option<String>,
    operation: Option<String>,
    result: Option<String>,
    error_code: Option<String>,
    safe_path: Option<String>,
    item_count: Option<u64>,
    duration_ms: Option<u64>,
}

impl AppLogEventVisitor {
    fn set_string(&mut self, field: &Field, value: &str) {
        let slot = match field.name() {
            "event_name" => &mut self.event_name,
            "task_id" => &mut self.task_id,
            "game_id" => &mut self.game_id,
            "profile_id" => &mut self.profile_id,
            "mod_id" => &mut self.mod_id,
            "task_kind" => &mut self.task_kind,
            "task_status" => &mut self.task_status,
            "phase" => &mut self.phase,
            "operation" => &mut self.operation,
            "result" => &mut self.result,
            "error_code" => &mut self.error_code,
            "safe_path" => &mut self.safe_path,
            _ => {
                self.invalid = true;
                return;
            }
        };
        if slot.replace(value.to_owned()).is_some() {
            self.invalid = true;
        }
    }

    fn into_validated_record(
        self,
        level: &Level,
        timestamp_unix_millis: u128,
    ) -> Result<ValidatedAppLogRecord> {
        if self.invalid {
            anyhow::bail!("app log event contains unknown or duplicate fields");
        }
        let level = match *level {
            Level::INFO => "info",
            Level::WARN => "warn",
            Level::ERROR => "error",
            _ => anyhow::bail!("app log event contains unsupported level"),
        };
        let event_name = self
            .event_name
            .context("app log event is missing event name")?;
        validate_stable_code("event", &event_name)?;
        validate_optional_id("task_id", self.task_id.as_deref())?;
        validate_optional_id("game_id", self.game_id.as_deref())?;
        validate_optional_id("profile_id", self.profile_id.as_deref())?;
        validate_optional_id("mod_id", self.mod_id.as_deref())?;
        validate_optional_code("task_kind", self.task_kind.as_deref())?;
        validate_optional_code("task_status", self.task_status.as_deref())?;
        validate_optional_code("phase", self.phase.as_deref())?;
        validate_optional_code("operation", self.operation.as_deref())?;
        validate_optional_code("result", self.result.as_deref())?;
        validate_optional_code("error_code", self.error_code.as_deref())?;
        if let Some(safe_path) = self.safe_path.as_deref() {
            validate_safe_path(safe_path)?;
        }

        Ok(ValidatedAppLogRecord(AppLogRecord {
            schema_version: 1,
            timestamp_unix_millis,
            level,
            event: event_name,
            task_id: self.task_id,
            game_id: self.game_id,
            profile_id: self.profile_id,
            mod_id: self.mod_id,
            task_kind: self.task_kind,
            task_status: self.task_status,
            phase: self.phase,
            operation: self.operation,
            result: self.result,
            error_code: self.error_code,
            safe_path: self.safe_path,
            item_count: self.item_count,
            duration_ms: self.duration_ms,
        }))
    }
}

impl Visit for AppLogEventVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.set_string(field, value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        let slot = match field.name() {
            "item_count" => &mut self.item_count,
            "duration_ms" => &mut self.duration_ms,
            _ => {
                self.invalid = true;
                return;
            }
        };
        if slot.replace(value).is_some() {
            self.invalid = true;
        }
    }

    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {
        self.invalid = true;
    }
}

#[derive(Debug, Serialize)]
struct AppLogRecord {
    schema_version: u8,
    timestamp_unix_millis: u128,
    level: &'static str,
    event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    game_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mod_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safe_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
}

struct ValidatedAppLogRecord(AppLogRecord);

struct FileSystemAppLogWriter {
    log_dir: Dir,
    retention_days: i64,
    state: Mutex<AppLogWriterState>,
}

#[derive(Default)]
struct AppLogWriterState {
    last_retention_day: Option<i64>,
}

enum AppLogSinkError {
    Write(anyhow::Error),
    Retention(anyhow::Error),
}

impl AppLogSinkError {
    fn failure(&self) -> AppLogFailure {
        match self {
            Self::Write(_) => AppLogFailure::WriteFailed,
            Self::Retention(_) => AppLogFailure::RetentionFailed,
        }
    }
}

impl std::fmt::Debug for AppLogSinkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Write(error) => formatter.debug_tuple("Write").field(error).finish(),
            Self::Retention(error) => formatter.debug_tuple("Retention").field(error).finish(),
        }
    }
}

impl FileSystemAppLogWriter {
    fn new(app_data_root: PathBuf, retention_days: i64) -> Result<Self> {
        if retention_days <= 0 {
            anyhow::bail!("app log retention must be positive");
        }
        let app_data_dir = open_app_data_directory(&app_data_root)?;
        let logs_dir = open_managed_directory(&app_data_dir, "logs", "logs directory")?;
        let log_dir = open_managed_directory(&logs_dir, "app", "app log directory")?;
        Ok(Self {
            log_dir,
            retention_days,
            state: Mutex::new(AppLogWriterState::default()),
        })
    }

    fn prepare(&self, timestamp_unix_millis: u128) -> Result<()> {
        let current_day = days_since_epoch(timestamp_unix_millis)?;
        self.prune_before(current_day)?;
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("app log writer state is unavailable"))?
            .last_retention_day = Some(current_day);
        Ok(())
    }

    fn write(&self, record: &ValidatedAppLogRecord) -> Result<(), AppLogSinkError> {
        let mut state = self.state.lock().map_err(|_| {
            AppLogSinkError::Write(anyhow::anyhow!("app log writer state is unavailable"))
        })?;
        let current_day =
            days_since_epoch(record.0.timestamp_unix_millis).map_err(AppLogSinkError::Write)?;
        let file_name = app_log_file_name(current_day).map_err(AppLogSinkError::Write)?;
        let mut file = self
            .open_log_file(&file_name)
            .map_err(AppLogSinkError::Write)?;
        let serialized = serde_json::to_vec(&record.0)
            .context("failed to serialize app log event")
            .map_err(AppLogSinkError::Write)?;
        file.write_all(&serialized)
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_data())
            .context("failed to write app log event")
            .map_err(AppLogSinkError::Write)?;

        if state.last_retention_day != Some(current_day) {
            self.prune_before(current_day)
                .map_err(AppLogSinkError::Retention)?;
            state.last_retention_day = Some(current_day);
        }
        Ok(())
    }

    fn prune_before(&self, current_day: i64) -> Result<()> {
        let cutoff_day = current_day
            .checked_sub(self.retention_days - 1)
            .context("app log retention cutoff is out of range")?;
        let cutoff_name = app_log_file_name(cutoff_day)?;
        for entry in self
            .log_dir
            .entries()
            .context("failed to read app log directory")?
        {
            let entry = entry.context("failed to read app log directory entry")?;
            if !entry
                .file_type()
                .context("failed to inspect app log entry type")?
                .is_file()
            {
                continue;
            }
            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if !is_app_log_file_name(file_name) {
                continue;
            }
            if file_name < cutoff_name.as_str() {
                entry
                    .remove_file()
                    .context("failed to prune expired app log")?;
            } else {
                tighten_retained_file_permissions(&entry)?;
            }
        }
        Ok(())
    }

    fn open_log_file(&self, file_name: &str) -> Result<File> {
        ensure_regular_file_or_missing(&self.log_dir, file_name)?;

        let mut options = OpenOptions::new();
        options.append(true).create(true);
        options.follow(FollowSymlinks::No);
        configure_secure_file_mode(&mut options);
        let file = self
            .log_dir
            .open_with(file_name, &options)
            .context("failed to open app log")?;
        let metadata = file
            .metadata()
            .context("failed to inspect opened app log")?;
        if !is_regular_file(&metadata) {
            anyhow::bail!("app log target is not a regular file");
        }
        tighten_file_permissions(&file)?;
        Ok(file)
    }
}

fn open_app_data_directory(app_data_root: &Path) -> Result<Dir> {
    let parent = app_data_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .context("app data directory must have a parent")?;
    let name = app_data_root
        .file_name()
        .context("app data directory must have a final component")?;
    let parent = Dir::open_ambient_dir(parent, ambient_authority())
        .context("failed to open app data parent directory")?;
    match parent.create_dir(name) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("failed to create app data directory for app log"),
    }
    let directory = parent
        .open_dir_nofollow(name)
        .context("failed to open app data directory")?;
    ensure_real_directory(&directory, "app data directory")?;
    Ok(directory)
}

fn open_managed_directory(parent: &Dir, name: &str, label: &str) -> Result<Dir> {
    let mut builder = DirBuilder::new();
    configure_secure_directory_mode(&mut builder);
    match parent.create_dir_with(name, &builder) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).with_context(|| format!("failed to create {label}")),
    }
    let directory = parent
        .open_dir_nofollow(name)
        .with_context(|| format!("failed to open {label}"))?;
    ensure_real_directory(&directory, label)?;
    tighten_directory_permissions(&directory, label)?;
    Ok(directory)
}

fn ensure_real_directory(directory: &Dir, label: &str) -> Result<()> {
    let metadata = directory
        .dir_metadata()
        .with_context(|| format!("failed to inspect {label}"))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        anyhow::bail!("{label} is not a real directory");
    }
    Ok(())
}

fn ensure_regular_file_or_missing(directory: &Dir, file_name: &str) -> Result<()> {
    match directory.symlink_metadata(file_name) {
        Ok(metadata) if is_regular_file(&metadata) => Ok(()),
        Ok(_) => anyhow::bail!("app log target is not a regular file"),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("failed to inspect app log target"),
    }
}

fn is_regular_file(metadata: &Metadata) -> bool {
    metadata.is_file() && !is_link_or_reparse(metadata)
}

fn is_link_or_reparse(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

#[cfg(unix)]
fn configure_secure_directory_mode(builder: &mut DirBuilder) {
    builder.mode(APP_LOG_DIRECTORY_MODE);
}

#[cfg(not(unix))]
fn configure_secure_directory_mode(_builder: &mut DirBuilder) {}

#[cfg(unix)]
fn tighten_directory_permissions(directory: &Dir, label: &str) -> Result<()> {
    directory
        .set_permissions(".", Permissions::from_mode(APP_LOG_DIRECTORY_MODE))
        .with_context(|| format!("failed to secure {label}"))
}

#[cfg(not(unix))]
fn tighten_directory_permissions(_directory: &Dir, _label: &str) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn configure_secure_file_mode(options: &mut OpenOptions) {
    options.mode(APP_LOG_FILE_MODE);
}

#[cfg(not(unix))]
fn configure_secure_file_mode(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn tighten_file_permissions(file: &File) -> Result<()> {
    file.set_permissions(Permissions::from_mode(APP_LOG_FILE_MODE))
        .context("failed to secure app log file")
}

#[cfg(not(unix))]
fn tighten_file_permissions(_file: &File) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn tighten_retained_file_permissions(entry: &cap_std::fs::DirEntry) -> Result<()> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = entry
        .open_with(&options)
        .context("failed to open retained app log")?;
    if !is_regular_file(
        &file
            .metadata()
            .context("failed to inspect retained app log")?,
    ) {
        anyhow::bail!("retained app log target is not a regular file");
    }
    tighten_file_permissions(&file)
}

#[cfg(not(unix))]
fn tighten_retained_file_permissions(_entry: &cap_std::fs::DirEntry) -> Result<()> {
    Ok(())
}

fn validate_optional_id(label: &str, value: Option<&str>) -> Result<()> {
    if let Some(value) = value {
        validate_id(label, value)?;
    }
    Ok(())
}

fn validate_optional_code(label: &str, value: Option<&str>) -> Result<()> {
    if let Some(value) = value {
        validate_stable_code(label, value)?;
    }
    Ok(())
}

fn validate_stable_code(label: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_CODE_LENGTH
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-".contains(&byte)
        })
        || contains_sensitive_text(value)
    {
        anyhow::bail!("app log event contains invalid {label}");
    }
    Ok(())
}

fn validate_id(label: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_ID_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
        || contains_sensitive_text(value)
    {
        anyhow::bail!("app log event contains invalid {label}");
    }
    Ok(())
}

fn validate_safe_path(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_SAFE_PATH_LENGTH
        || value.starts_with('/')
        || value.contains('\0')
        || value.contains('\\')
        || value.contains(':')
        || contains_sensitive_text(value)
        || value.split('/').any(|segment| {
            segment.is_empty()
                || matches!(segment, "." | "..")
                || !segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        })
    {
        anyhow::bail!("app log event contains invalid safe_path");
    }
    Ok(())
}

fn contains_sensitive_text(value: &str) -> bool {
    if value.chars().any(char::is_control) {
        return true;
    }
    let lower = value.to_ascii_lowercase();
    const FORBIDDEN_SNIPPETS: &[&str] = &[
        "token",
        "cookie",
        "api_key",
        "api-key",
        "apikey",
        "api key",
        "authorization",
        "bearer ",
        "password",
        "passwd",
        "secret",
        "session=",
        "ghp_",
        "github_pat_",
        "steamid",
        "steam_id",
        "username=",
        "user_name=",
        "user-name=",
        "c:/",
        "c:\\",
        "\\users\\",
        "/users/",
        "/home/",
        "/root/",
        "appdata\\",
        "%appdata%",
    ];
    FORBIDDEN_SNIPPETS
        .iter()
        .any(|snippet| lower.contains(snippet))
        || contains_long_digit_run(value, 17)
}

fn looks_like_path_text(value: &str) -> bool {
    value.contains('\\')
        || value.starts_with('/')
        || value
            .as_bytes()
            .windows(3)
            .any(|window| window[0].is_ascii_alphabetic() && window[1] == b':' && window[2] == b'/')
        || value
            .split_ascii_whitespace()
            .any(|part| part.starts_with('/'))
}

fn contains_long_digit_run(value: &str, minimum_length: usize) -> bool {
    let mut run = 0;
    for byte in value.bytes() {
        if byte.is_ascii_digit() {
            run += 1;
            if run >= minimum_length {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

fn days_since_epoch(timestamp_unix_millis: u128) -> Result<i64> {
    i64::try_from(timestamp_unix_millis / MILLIS_PER_DAY)
        .context("app log timestamp is out of supported range")
}

fn app_log_file_name(days_since_epoch: i64) -> Result<String> {
    let (year, month, day) = civil_from_days(days_since_epoch);
    if !(0..=9999).contains(&year) {
        anyhow::bail!("app log date is out of supported range");
    }
    Ok(format!("app-{year:04}-{month:02}-{day:02}.log"))
}

fn is_app_log_file_name(file_name: &str) -> bool {
    let bytes = file_name.as_bytes();
    bytes.len() == "app-1970-01-01.log".len()
        && file_name.starts_with("app-")
        && file_name.ends_with(".log")
        && bytes[4..8].iter().all(u8::is_ascii_digit)
        && bytes[8] == b'-'
        && bytes[9..11].iter().all(u8::is_ascii_digit)
        && bytes[11] == b'-'
        && bytes[12..14].iter().all(u8::is_ascii_digit)
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let days = days_since_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::{TextLogKind, TextLogReadRequest, TextLogReader};
    use std::fs;

    struct FixedClock(u128);

    impl AppLogClock for FixedClock {
        fn now_unix_millis(&self) -> Result<u128> {
            Ok(self.0)
        }
    }

    fn scoped_layer(root: &Path, timestamp: u128, health: AppLogHealth) -> SafeAppLogLayer {
        SafeAppLogLayer::new(root.to_path_buf(), Arc::new(FixedClock(timestamp)), health)
            .expect("prepare app log layer")
    }

    #[cfg(unix)]
    fn create_directory_link(link: &Path, target: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create directory symlink");
    }

    #[cfg(windows)]
    fn create_directory_link(link: &Path, target: &Path) {
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_str().expect("junction path"),
                target.to_str().expect("junction target"),
            ])
            .output()
            .expect("create directory junction");
        assert!(
            output.status.success(),
            "mklink failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).expect("remove directory symlink");
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).expect("remove directory junction");
    }

    #[test]
    fn redaction_helper_rejects_paths_accounts_and_credentials() {
        for sensitive in [
            r"C:\Users\Player\AppData\Roaming\HMM\logs",
            r"D:\Games\MonsterHunterWorld\nativePC",
            "D:/Games/MonsterHunterWorld/nativePC",
            "/home/player/.local/state/hmm",
            "/games/MonsterHunterWorld/nativePC",
            "username=Player",
            "steamId=76561198012345678",
            "token=top-secret",
            "Cookie: session=abc",
            "api_key=abc123",
            "api key: abc123",
        ] {
            assert_eq!(redact_sensitive_text(sensitive), "[redacted]");
        }
        assert_eq!(
            redact_sensitive_text("install.commit.completed"),
            "install.commit.completed"
        );
    }

    #[test]
    fn health_exposes_stable_codes_and_keeps_the_most_severe_failure() {
        let health = AppLogHealth::ready();
        health.degrade(AppLogFailure::EventRejected);
        assert_eq!(health.status_code(), "app_log_event_rejected");
        health.degrade(AppLogFailure::RetentionFailed);
        assert_eq!(health.status_code(), "app_log_retention_failed");
        health.degrade(AppLogFailure::WriteFailed);
        assert_eq!(health.status_code(), "app_log_write_failed");
        health.degrade(AppLogFailure::EventRejected);
        assert_eq!(health.status_code(), "app_log_write_failed");
        health.degrade(AppLogFailure::InitializationFailed);
        assert_eq!(health.status_code(), "app_log_initialization_failed");
    }

    #[test]
    fn safe_layer_writes_reader_compatible_jsonl() {
        let temp = tempfile::tempdir().expect("temp dir");
        let timestamp = 1_704_067_200_000;
        let health = AppLogHealth::ready();
        let subscriber = tracing_subscriber::registry().with(scoped_layer(
            temp.path(),
            timestamp,
            health.clone(),
        ));

        tracing::subscriber::with_default(subscriber, || {
            emit_safe_app_log(
                AppLogEvent::info("game.discovery.completed")
                    .with_game_id("mhw")
                    .with_operation("scan_candidates")
                    .with_result("success")
                    .with_item_count(2),
            );
        });

        assert_eq!(health.status_code(), "ok");
        let log_path = temp
            .path()
            .join("logs")
            .join("app")
            .join("app-2024-01-01.log");
        let contents = fs::read_to_string(&log_path).expect("read app log");
        let value: serde_json::Value = serde_json::from_str(contents.trim()).expect("json line");
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["event"], "game.discovery.completed");
        assert_eq!(value["game_id"], "mhw");
        assert_eq!(value["item_count"], 2);

        let reader = crate::FileSystemTextLogReader::new(temp.path().to_path_buf());
        let lines = reader
            .read_recent_sanitized(TextLogReadRequest {
                kind: TextLogKind::App,
                max_lines: 10,
            })
            .expect("reader consumes writer output");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].source, "app-2024-01-01.log");
        assert_eq!(lines[0].line.trim(), contents.trim());
    }

    #[test]
    fn file_sink_rejects_sensitive_or_unapproved_tracing_fields() {
        let temp = tempfile::tempdir().expect("temp dir");
        let timestamp = 1_704_067_200_000;
        let health = AppLogHealth::ready();
        let subscriber = tracing_subscriber::registry().with(scoped_layer(
            temp.path(),
            timestamp,
            health.clone(),
        ));
        let raw = r"C:\Users\Player\game\nativePC token=top-secret cookie=session";

        tracing::subscriber::with_default(subscriber, || {
            emit_safe_app_log(
                AppLogEvent::error("application.error")
                    .with_operation(raw)
                    .with_error_code("internal_error"),
            );
            tracing::warn!(target: SAFE_APP_LOG_TARGET, raw_error = %raw, "unsafe direct event");
            tracing::warn!(raw_error = %raw, "ordinary tracing stays outside the file layer");
            emit_safe_app_log(AppLogEvent::info("application.recovered"));
        });

        assert_eq!(health.status_code(), "app_log_event_rejected");
        let contents = fs::read_to_string(
            temp.path()
                .join("logs")
                .join("app")
                .join("app-2024-01-01.log"),
        )
        .expect("read app log");
        assert!(contents.contains("application.recovered"));
        assert!(!contents.contains("C:\\Users"));
        assert!(!contents.contains("Player"));
        assert!(!contents.contains("top-secret"));
        assert!(!contents.contains("cookie"));
        assert_eq!(contents.lines().count(), 1);
    }

    #[test]
    fn writer_rotates_by_utc_day_and_prunes_only_expired_app_logs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let writer =
            FileSystemAppLogWriter::new(temp.path().to_path_buf(), 14).expect("create writer");
        fs::create_dir_all(temp.path().join("logs").join("app")).expect("create log dir");
        let log_dir = temp.path().join("logs").join("app");
        fs::write(log_dir.join(app_log_file_name(6).unwrap()), "old\n").unwrap();
        fs::write(log_dir.join(app_log_file_name(7).unwrap()), "cutoff\n").unwrap();
        fs::write(log_dir.join("notes.txt"), "unmanaged\n").unwrap();
        writer.prepare(20 * MILLIS_PER_DAY).expect("prepare writer");

        assert!(!log_dir.join(app_log_file_name(6).unwrap()).exists());
        assert!(log_dir.join(app_log_file_name(7).unwrap()).exists());
        assert!(log_dir.join("notes.txt").exists());

        for day in [20, 21] {
            let record = ValidatedAppLogRecord(AppLogRecord {
                schema_version: 1,
                timestamp_unix_millis: day * MILLIS_PER_DAY,
                level: "info",
                event: "application.started".to_owned(),
                task_id: None,
                game_id: None,
                profile_id: None,
                mod_id: None,
                task_kind: None,
                task_status: None,
                phase: None,
                operation: None,
                result: None,
                error_code: None,
                safe_path: None,
                item_count: None,
                duration_ms: None,
            });
            writer.write(&record).expect("write rotated record");
        }
        assert!(log_dir.join(app_log_file_name(20).unwrap()).exists());
        assert!(log_dir.join(app_log_file_name(21).unwrap()).exists());
    }

    #[test]
    fn retention_does_not_delete_an_outside_sentinel_through_a_link() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside temp dir");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, "outside\n").expect("write outside sentinel");
        let writer =
            FileSystemAppLogWriter::new(temp.path().to_path_buf(), 14).expect("create writer");
        let linked_old_log = temp
            .path()
            .join("logs")
            .join("app")
            .join(app_log_file_name(6).unwrap());
        create_directory_link(&linked_old_log, outside.path());

        writer
            .prepare(20 * MILLIS_PER_DAY)
            .expect("prune through directory handle");

        assert_eq!(fs::read_to_string(&sentinel).unwrap(), "outside\n");
        assert!(linked_old_log.exists());
        remove_directory_link(&linked_old_log);
    }

    #[cfg(unix)]
    #[test]
    fn runtime_ancestor_replacement_cannot_redirect_writes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let timestamp = 1_704_067_200_000;
        let health = AppLogHealth::ready();
        let layer = scoped_layer(temp.path(), timestamp, health.clone());
        let moved_logs = temp.path().join("logs-before-replacement");
        fs::rename(temp.path().join("logs"), &moved_logs).expect("rename managed logs dir");
        let outside = tempfile::tempdir().expect("outside temp dir");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, "outside\n").expect("write outside sentinel");
        create_directory_link(&temp.path().join("logs"), outside.path());
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            emit_safe_app_log(AppLogEvent::info("application.started"));
        });

        assert_eq!(health.status_code(), "ok");
        assert_eq!(fs::read_to_string(&sentinel).unwrap(), "outside\n");
        assert!(!outside.path().join("app").exists());
        assert!(moved_logs.join("app").join("app-2024-01-01.log").is_file());
        remove_directory_link(&temp.path().join("logs"));
    }

    #[cfg(unix)]
    #[test]
    fn writer_tightens_managed_directory_and_file_permissions() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().expect("temp dir");
        let log_dir = temp.path().join("logs").join("app");
        fs::create_dir_all(&log_dir).expect("create permissive log dirs");
        fs::set_permissions(temp.path().join("logs"), fs::Permissions::from_mode(0o777))
            .expect("set permissive logs mode");
        fs::set_permissions(&log_dir, fs::Permissions::from_mode(0o777))
            .expect("set permissive app mode");
        let log_path = log_dir.join(app_log_file_name(20).unwrap());
        fs::write(&log_path, "existing\n").expect("write existing log");
        fs::set_permissions(&log_path, fs::Permissions::from_mode(0o666))
            .expect("set permissive log mode");
        let retained_path = log_dir.join(app_log_file_name(7).unwrap());
        fs::write(&retained_path, "retained\n").expect("write retained log");
        fs::set_permissions(&retained_path, fs::Permissions::from_mode(0o666))
            .expect("set permissive retained log mode");
        let writer =
            FileSystemAppLogWriter::new(temp.path().to_path_buf(), 14).expect("create writer");
        writer
            .prepare(20 * MILLIS_PER_DAY)
            .expect("prepare app log retention");
        let record = ValidatedAppLogRecord(AppLogRecord {
            schema_version: 1,
            timestamp_unix_millis: 20 * MILLIS_PER_DAY,
            level: "info",
            event: "application.started".to_owned(),
            task_id: None,
            game_id: None,
            profile_id: None,
            mod_id: None,
            task_kind: None,
            task_status: None,
            phase: None,
            operation: None,
            result: None,
            error_code: None,
            safe_path: None,
            item_count: None,
            duration_ms: None,
        });
        writer.write(&record).expect("write app log");

        assert_eq!(
            fs::metadata(temp.path().join("logs"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            APP_LOG_DIRECTORY_MODE
        );
        assert_eq!(
            fs::metadata(&log_dir).unwrap().permissions().mode() & 0o777,
            APP_LOG_DIRECTORY_MODE
        );
        assert_eq!(
            fs::metadata(&log_path).unwrap().permissions().mode() & 0o777,
            APP_LOG_FILE_MODE
        );
        assert_eq!(
            fs::metadata(&retained_path).unwrap().permissions().mode() & 0o777,
            APP_LOG_FILE_MODE
        );
    }

    #[test]
    fn initialization_rejects_links_in_managed_log_directories() {
        for relative_link in [PathBuf::from("logs"), PathBuf::from("logs").join("app")] {
            let temp = tempfile::tempdir().expect("temp dir");
            let outside = tempfile::tempdir().expect("outside temp dir");
            let link = temp.path().join(relative_link);
            fs::create_dir_all(link.parent().expect("link parent")).expect("create link parent");
            create_directory_link(&link, outside.path());

            let health = AppLogHealth::ready();
            let layer = prepare_app_log_layer(
                temp.path(),
                Arc::new(FixedClock(1_704_067_200_000)),
                health.clone(),
            );

            assert!(layer.is_none());
            assert_eq!(health.status_code(), "app_log_initialization_failed");
            assert!(fs::read_dir(outside.path())
                .expect("read outside dir")
                .next()
                .is_none());
            remove_directory_link(&link);
        }
    }

    #[test]
    fn initialization_rejects_a_linked_app_data_root() {
        let parent = tempfile::tempdir().expect("app data parent temp dir");
        let outside = tempfile::tempdir().expect("outside temp dir");
        let app_data_root = parent.path().join("linked-app-data");
        create_directory_link(&app_data_root, outside.path());

        let health = AppLogHealth::ready();
        let layer = prepare_app_log_layer(
            &app_data_root,
            Arc::new(FixedClock(1_704_067_200_000)),
            health.clone(),
        );

        assert!(layer.is_none());
        assert_eq!(health.status_code(), "app_log_initialization_failed");
        assert!(fs::read_dir(outside.path())
            .expect("read outside dir")
            .next()
            .is_none());
        remove_directory_link(&app_data_root);
    }

    #[cfg(unix)]
    #[test]
    fn runtime_write_rejects_symlinked_daily_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let timestamp = 1_704_067_200_000;
        let health = AppLogHealth::ready();
        let layer = scoped_layer(temp.path(), timestamp, health.clone());
        let outside_root = tempfile::tempdir().expect("outside temp dir");
        let outside = outside_root.path().join("outside.log");
        fs::write(&outside, "outside\n").expect("write outside file");
        let log_path = temp
            .path()
            .join("logs")
            .join("app")
            .join("app-2024-01-01.log");
        std::os::unix::fs::symlink(&outside, &log_path).expect("create daily log symlink");
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            emit_safe_app_log(AppLogEvent::info("application.started"));
        });

        assert_eq!(health.status_code(), "app_log_write_failed");
        assert_eq!(fs::read_to_string(&outside).unwrap(), "outside\n");
    }

    #[cfg(windows)]
    #[test]
    fn runtime_write_rejects_reparse_point_at_daily_file_path() {
        let temp = tempfile::tempdir().expect("temp dir");
        let timestamp = 1_704_067_200_000;
        let health = AppLogHealth::ready();
        let layer = scoped_layer(temp.path(), timestamp, health.clone());
        let outside = tempfile::tempdir().expect("outside temp dir");
        fs::write(outside.path().join("sentinel.txt"), "outside\n").expect("write outside file");
        let log_path = temp
            .path()
            .join("logs")
            .join("app")
            .join("app-2024-01-01.log");
        create_directory_link(&log_path, outside.path());
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            emit_safe_app_log(AppLogEvent::info("application.started"));
        });

        assert_eq!(health.status_code(), "app_log_write_failed");
        assert_eq!(
            fs::read_to_string(outside.path().join("sentinel.txt")).unwrap(),
            "outside\n"
        );
        remove_directory_link(&log_path);
    }

    #[test]
    fn initialization_failure_returns_stable_degraded_status_without_panic() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::write(temp.path().join("logs"), "path conflict").expect("create path conflict");
        let health = AppLogHealth::ready();
        let layer = prepare_app_log_layer(
            temp.path(),
            Arc::new(FixedClock(1_704_067_200_000)),
            health.clone(),
        );

        assert!(layer.is_none());
        assert_eq!(health.status_code(), "app_log_initialization_failed");
        assert_eq!(
            fs::read_to_string(temp.path().join("logs")).unwrap(),
            "path conflict"
        );
    }

    #[test]
    fn runtime_write_failure_updates_health_without_touching_other_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let timestamp = 1_704_067_200_000;
        let health = AppLogHealth::ready();
        let layer = scoped_layer(temp.path(), timestamp, health.clone());
        let app_log_dir = temp.path().join("logs").join("app");
        let log_path = app_log_dir.join("app-2024-01-01.log");
        fs::create_dir(&log_path).expect("create daily log path conflict");
        let sentinel = temp.path().join("sentinel.txt");
        fs::write(&sentinel, "unchanged\n").expect("write sentinel");
        let subscriber = tracing_subscriber::registry().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            emit_safe_app_log(AppLogEvent::info("application.started"));
        });

        assert_eq!(health.status_code(), "app_log_write_failed");
        assert!(log_path.is_dir());
        assert_eq!(fs::read_to_string(sentinel).unwrap(), "unchanged\n");
    }
}
