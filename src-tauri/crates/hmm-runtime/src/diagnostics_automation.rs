use crate::game_automation::is_canonically_within;
use crate::{production_app_data_dir, RuntimeEnvironment};
use hmm_app::DiagnosticsPageSnapshotService;
use hmm_infra::{
    DiagnosticsEvidenceHealthState, FileSystemAuditLogReader, FileSystemTextLogReader,
    SystemDiagnosticsEnvironmentProvider,
};
use serde::Serialize;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSnapshot {
    pub platform_status: String,
    pub app_log_status: String,
    pub task_log_status: String,
    pub audit_log_status: String,
    pub app_log_line_count: usize,
    pub task_log_line_count: usize,
    pub audit_event_count: usize,
    pub platform: Option<DiagnosticsPlatformSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsPlatformSnapshot {
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub game_adapter_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyDiagnosticsAutomationError {
    AppDataUnavailable,
    SandboxStoragePathRejected,
}

impl ReadOnlyDiagnosticsAutomationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::AppDataUnavailable => "app_data_unavailable",
            Self::SandboxStoragePathRejected => "sandbox_storage_path_rejected",
        }
    }
}

impl fmt::Display for ReadOnlyDiagnosticsAutomationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ReadOnlyDiagnosticsAutomationError {}

pub struct ReadOnlyDiagnosticsAutomation {
    snapshot_service: DiagnosticsPageSnapshotService,
}

impl ReadOnlyDiagnosticsAutomation {
    pub fn from_environment(
        environment: &RuntimeEnvironment,
    ) -> Result<Self, ReadOnlyDiagnosticsAutomationError> {
        let app_data_dir = match environment {
            RuntimeEnvironment::Production => production_app_data_dir()
                .ok_or(ReadOnlyDiagnosticsAutomationError::AppDataUnavailable)?,
            RuntimeEnvironment::Sandbox { data_dir } => {
                validate_sandbox_log_paths(data_dir)?;
                data_dir.clone()
            }
        };
        let text_log_reader = Arc::new(FileSystemTextLogReader::new(app_data_dir.clone()));
        let audit_log_reader = Arc::new(FileSystemAuditLogReader::new(app_data_dir));
        let environment_provider = Arc::new(SystemDiagnosticsEnvironmentProvider::new(
            env!("CARGO_PKG_VERSION").to_owned(),
            vec!["mhw".to_owned()],
        ));
        let evidence_health = Arc::new(DiagnosticsEvidenceHealthState::default());

        Ok(Self {
            snapshot_service: DiagnosticsPageSnapshotService::new(
                text_log_reader,
                audit_log_reader,
                environment_provider,
                evidence_health,
            ),
        })
    }

    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        let snapshot = self.snapshot_service.read();
        DiagnosticsSnapshot {
            platform_status: snapshot.platform_status,
            app_log_status: snapshot.app_log_status,
            task_log_status: snapshot.task_log_status,
            audit_log_status: snapshot.audit_log_status,
            app_log_line_count: snapshot.app_log_lines.len(),
            task_log_line_count: snapshot.task_log_lines.len(),
            audit_event_count: snapshot.audit_events.len(),
            platform: snapshot
                .platform_summary
                .map(|summary| DiagnosticsPlatformSnapshot {
                    app_version: summary.app_version,
                    os: summary.os,
                    arch: summary.arch,
                    game_adapter_ids: summary.game_adapter_ids,
                }),
        }
    }
}

fn validate_sandbox_log_paths(data_dir: &Path) -> Result<(), ReadOnlyDiagnosticsAutomationError> {
    if !data_dir.is_dir() || !is_canonically_within(data_dir, data_dir) {
        return Err(ReadOnlyDiagnosticsAutomationError::SandboxStoragePathRejected);
    }
    let managed_paths = [
        data_dir.join("logs"),
        data_dir.join("logs").join("app"),
        data_dir.join("logs").join("tasks"),
        data_dir.join("logs").join("audit"),
    ];
    if managed_paths
        .iter()
        .any(|path| !is_canonically_within(path, data_dir))
    {
        return Err(ReadOnlyDiagnosticsAutomationError::SandboxStoragePathRejected);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::AuditLogEvent;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    fn sandbox_environment(root: &Path) -> RuntimeEnvironment {
        RuntimeEnvironment::sandbox(root.to_path_buf()).expect("sandbox environment")
    }

    #[test]
    fn snapshot_returns_only_bounded_platform_status_and_counts_without_writing() {
        let temp = tempfile::tempdir().expect("temp dir");
        let app_logs = temp.path().join("logs").join("app");
        let task_logs = temp.path().join("logs").join("tasks");
        let audit_logs = temp.path().join("logs").join("audit");
        fs::create_dir_all(&app_logs).expect("create app logs");
        fs::create_dir_all(&task_logs).expect("create task logs");
        fs::create_dir_all(&audit_logs).expect("create audit logs");
        fs::write(
            app_logs.join("app-1970-01-01.log"),
            "safe app line\nC:/Users/Player/raw_path\n",
        )
        .expect("write app log fixture");
        fs::write(
            task_logs.join("task-fixture-1.log"),
            "safe task line\ntoken=private\n",
        )
        .expect("write task log fixture");
        let event = AuditLogEvent {
            timestamp_unix_millis: 42,
            category: "save_backup".to_owned(),
            operation: "create_backup".to_owned(),
            result: "success".to_owned(),
            fields: BTreeMap::from([("file_count".to_owned(), "2".to_owned())]),
        };
        fs::write(
            audit_logs.join("audit-1970-01-01.log"),
            format!(
                "{}\n",
                serde_json::to_string(&event).expect("serialize audit fixture")
            ),
        )
        .expect("write audit fixture");
        let before = directory_snapshot(temp.path());

        let automation =
            ReadOnlyDiagnosticsAutomation::from_environment(&sandbox_environment(temp.path()))
                .expect("diagnostics automation");
        let snapshot = automation.snapshot();

        assert_eq!(snapshot.app_log_line_count, 1);
        assert_eq!(snapshot.task_log_line_count, 1);
        assert_eq!(snapshot.audit_event_count, 1);
        let serialized = serde_json::to_string(&snapshot).expect("serialize snapshot");
        for forbidden in [
            "safe app line",
            "safe task line",
            "C:/Users/Player",
            "raw_path",
            "token=private",
            "app-1970-01-01.log",
            "task-fixture-1.log",
            "create_backup",
            "file_count",
        ] {
            assert!(!serialized.contains(forbidden));
        }
        assert_eq!(directory_snapshot(temp.path()), before);
    }

    #[test]
    #[cfg(unix)]
    fn sandbox_rejects_log_directory_symlink_escape() {
        let temp = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("outside dir");
        fs::create_dir_all(temp.path().join("logs")).expect("create logs root");

        std::os::unix::fs::symlink(outside.path(), temp.path().join("logs").join("app"))
            .expect("create symlink");

        assert_eq!(
            ReadOnlyDiagnosticsAutomation::from_environment(&sandbox_environment(temp.path()))
                .map(|_| ()),
            Err(ReadOnlyDiagnosticsAutomationError::SandboxStoragePathRejected)
        );
    }

    fn directory_snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn visit(root: &Path, directory: &Path, snapshot: &mut Vec<(PathBuf, Vec<u8>)>) {
            let mut entries = fs::read_dir(directory)
                .expect("read snapshot directory")
                .collect::<Result<Vec<_>, _>>()
                .expect("read snapshot entries");
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if path.is_dir() {
                    visit(root, &path, snapshot);
                } else {
                    snapshot.push((
                        path.strip_prefix(root)
                            .expect("relative snapshot path")
                            .to_path_buf(),
                        fs::read(path).expect("read snapshot file"),
                    ));
                }
            }
        }

        let mut snapshot = Vec::new();
        visit(root, root, &mut snapshot);
        snapshot
    }
}
