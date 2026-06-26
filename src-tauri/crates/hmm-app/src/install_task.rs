use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use hmm_core::{FileLayer, GameId, InstallPlan, ModId, ProfileId};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter};

use crate::{
    BuildImportedModInstallPlanRequest, CommitInstallPlanRequest, InstallCommitError,
    InstallCommitResult, InstallCommitService, InstallPlanningError, InstallPlanningService,
    TaskKind, TaskManager, TaskManagerError, TaskProgressEvent, TaskStarted, TaskStatus,
    UninstallModError, UninstallModRequest, UninstallModResult, UninstallModService,
};

const INSTALL_PLAN_BUILDING_PHASE: &str = "install.plan.building";
const INSTALL_COMMIT_PROCESSING_PHASE: &str = "install.commit.processing";
const INSTALL_COMPLETED_PHASE: &str = "install.completed";
const INSTALL_FAILED_PHASE: &str = "install.failed";
const INSTALL_FAILED_ERROR: &str = "install_failed";
const INSTALL_UNINSTALL_PROCESSING_PHASE: &str = "install.uninstall.processing";
const INSTALL_UNINSTALL_COMPLETED_PHASE: &str = "install.uninstall.completed";
const INSTALL_UNINSTALL_FAILED_PHASE: &str = "install.uninstall.failed";
const INSTALL_UNINSTALL_FAILED_ERROR: &str = "install_uninstall_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartInstallTaskRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub profile_id: ProfileId,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartUninstallTaskRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub profile_id: ProfileId,
}

pub struct InstallTaskService {
    task_manager: Arc<TaskManager>,
}

pub struct UninstallTaskService {
    task_manager: Arc<TaskManager>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallTaskRunError {
    pub events: Vec<TaskProgressEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UninstallTaskRunError {
    pub events: Vec<TaskProgressEvent>,
}

pub struct ImportedModInstallCommitRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub profile_id: ProfileId,
    pub plan: InstallPlan,
}

pub trait ImportedModInstallPlanner: Send + Sync {
    fn build_imported_mod_install_plan(
        &self,
        request: BuildImportedModInstallPlanRequest,
    ) -> Result<InstallPlan, InstallPlanningError>;
}

impl ImportedModInstallPlanner for InstallPlanningService {
    fn build_imported_mod_install_plan(
        &self,
        request: BuildImportedModInstallPlanRequest,
    ) -> Result<InstallPlan, InstallPlanningError> {
        self.build_plan_from_imported_mod(request)
    }
}

pub trait InstallPlanCommitter: Send + Sync {
    fn commit_install_plan(
        &self,
        request: ImportedModInstallCommitRequest,
    ) -> Result<InstallCommitResult, InstallCommitError>;
}

impl InstallPlanCommitter for InstallCommitService {
    fn commit_install_plan(
        &self,
        request: ImportedModInstallCommitRequest,
    ) -> Result<InstallCommitResult, InstallCommitError> {
        self.commit_plan(CommitInstallPlanRequest {
            profile_id: request.profile_id,
            plan: request.plan,
        })
    }
}

pub trait ModUninstaller: Send + Sync {
    fn uninstall_mod(
        &self,
        request: StartUninstallTaskRequest,
    ) -> Result<UninstallModResult, UninstallModError>;
}

impl ModUninstaller for UninstallModService {
    fn uninstall_mod(
        &self,
        request: StartUninstallTaskRequest,
    ) -> Result<UninstallModResult, UninstallModError> {
        UninstallModService::uninstall_mod(
            self,
            UninstallModRequest {
                profile_id: request.profile_id,
                mod_id: request.mod_id,
            },
        )
    }
}

pub struct InstallTaskRunner {
    task_manager: Arc<TaskManager>,
    planner: Arc<dyn ImportedModInstallPlanner>,
    committer: Arc<dyn InstallPlanCommitter>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    write_locks: Arc<GameProfileWriteLockRegistry>,
}

pub struct UninstallTaskRunner {
    task_manager: Arc<TaskManager>,
    uninstaller: Arc<dyn ModUninstaller>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    write_locks: Arc<GameProfileWriteLockRegistry>,
}

impl InstallTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self { task_manager }
    }

    pub fn start_install_task(
        &self,
        _request: StartInstallTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.task_manager.create_task(TaskKind::Install)?;

        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }
}

impl UninstallTaskService {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self { task_manager }
    }

    pub fn start_uninstall_task(
        &self,
        _request: StartUninstallTaskRequest,
    ) -> Result<TaskStarted, TaskManagerError> {
        let task = self.task_manager.create_task(TaskKind::Install)?;

        Ok(TaskStarted {
            task_id: task.task_id,
            kind: task.kind,
            status: task.status,
        })
    }
}

impl InstallTaskRunner {
    pub fn new(
        task_manager: Arc<TaskManager>,
        planner: Arc<dyn ImportedModInstallPlanner>,
        committer: Arc<dyn InstallPlanCommitter>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self::with_write_locks(
            task_manager,
            planner,
            committer,
            audit_log,
            clock,
            Arc::new(GameProfileWriteLockRegistry::default()),
        )
    }

    pub fn with_write_locks(
        task_manager: Arc<TaskManager>,
        planner: Arc<dyn ImportedModInstallPlanner>,
        committer: Arc<dyn InstallPlanCommitter>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
    ) -> Self {
        Self {
            task_manager,
            planner,
            committer,
            audit_log,
            clock,
            write_locks,
        }
    }

    pub fn run_install_task(
        &self,
        task_id: &str,
        request: StartInstallTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, InstallTaskRunError> {
        if self.task_manager.start_task(task_id).is_err() {
            return Err(InstallTaskRunError { events: Vec::new() });
        }

        let mut events = vec![running_event(task_id, INSTALL_PLAN_BUILDING_PHASE)];
        let plan =
            match self
                .planner
                .build_imported_mod_install_plan(BuildImportedModInstallPlanRequest {
                    game_id: request.game_id.clone(),
                    mod_id: request.mod_id.clone(),
                    layer: request.layer.clone(),
                }) {
                Ok(plan) => plan,
                Err(_) => {
                    return Err(self.fail_with_audit(task_id, &request, events, "planning", 0))
                }
            };
        let action_count = plan.actions.len();

        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            return Ok(events);
        }

        let write_lock = self
            .write_locks
            .lock_for(&request.game_id, &request.profile_id);
        let commit_result = {
            let _guard = write_lock.lock().map_err(|_| {
                self.fail_with_audit(task_id, &request, events.clone(), "lock", action_count)
            })?;
            if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                return Ok(events);
            }
            events.push(running_event(task_id, INSTALL_COMMIT_PROCESSING_PHASE));
            self.committer
                .commit_install_plan(ImportedModInstallCommitRequest {
                    game_id: request.game_id.clone(),
                    mod_id: request.mod_id.clone(),
                    profile_id: request.profile_id.clone(),
                    plan,
                })
        };

        match commit_result {
            Ok(_) => self.record_audit(task_id, &request, "success", action_count),
            Err(_) => {
                return Err(self.fail_with_audit(task_id, &request, events, "commit", action_count))
            }
        }

        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                events.push(TaskProgressEvent::new(
                    task.task_id,
                    task.kind,
                    task.status,
                    INSTALL_COMPLETED_PHASE,
                ));
                Ok(events)
            }
            Err(_) if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) => {
                Ok(events)
            }
            Err(_) => {
                Err(self.fail_with_audit(task_id, &request, events, "complete", action_count))
            }
        }
    }

    fn fail_with_audit(
        &self,
        task_id: &str,
        request: &StartInstallTaskRequest,
        mut events: Vec<TaskProgressEvent>,
        phase: &str,
        action_count: usize,
    ) -> InstallTaskRunError {
        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            return InstallTaskRunError { events };
        }

        let _ = self.task_manager.fail_task(task_id);
        events.push(failed_event(task_id, phase));
        self.record_audit(task_id, request, "failure", action_count);
        InstallTaskRunError { events }
    }

    fn record_audit(
        &self,
        task_id: &str,
        request: &StartInstallTaskRequest,
        result: &str,
        action_count: usize,
    ) {
        let timestamp_unix_millis = self.clock.now_unix_millis().unwrap_or_default();
        let mut fields = BTreeMap::new();
        fields.insert("task_id".to_owned(), task_id.to_owned());
        fields.insert("game_id".to_owned(), request.game_id.as_str().to_owned());
        fields.insert("mod_id".to_owned(), request.mod_id.as_str().to_owned());
        fields.insert(
            "profile_id".to_owned(),
            request.profile_id.as_str().to_owned(),
        );
        fields.insert("action_count".to_owned(), action_count.to_string());

        let _ = self.audit_log.record(AuditLogEvent {
            timestamp_unix_millis,
            category: "install".to_owned(),
            operation: "commit_imported_mod".to_owned(),
            result: result.to_owned(),
            fields,
        });
    }
}

impl UninstallTaskRunner {
    pub fn new(
        task_manager: Arc<TaskManager>,
        uninstaller: Arc<dyn ModUninstaller>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self::with_write_locks(
            task_manager,
            uninstaller,
            audit_log,
            clock,
            Arc::new(GameProfileWriteLockRegistry::default()),
        )
    }

    pub fn with_write_locks(
        task_manager: Arc<TaskManager>,
        uninstaller: Arc<dyn ModUninstaller>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
        write_locks: Arc<GameProfileWriteLockRegistry>,
    ) -> Self {
        Self {
            task_manager,
            uninstaller,
            audit_log,
            clock,
            write_locks,
        }
    }

    pub fn run_uninstall_task(
        &self,
        task_id: &str,
        request: StartUninstallTaskRequest,
    ) -> Result<Vec<TaskProgressEvent>, UninstallTaskRunError> {
        if self.task_manager.start_task(task_id).is_err() {
            return Err(UninstallTaskRunError { events: Vec::new() });
        }

        let mut events = vec![running_event(task_id, INSTALL_UNINSTALL_PROCESSING_PHASE)];
        let write_lock = self
            .write_locks
            .lock_for(&request.game_id, &request.profile_id);
        let uninstall_result = {
            let _guard = write_lock.lock().map_err(|_| {
                self.fail_uninstall_with_audit(task_id, &request, events.clone(), "lock", None)
            })?;
            if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
                return Ok(events);
            }
            self.uninstaller.uninstall_mod(request.clone())
        };

        let result = match uninstall_result {
            Ok(result) => result,
            Err(_) => {
                return Err(self.fail_uninstall_with_audit(
                    task_id,
                    &request,
                    events,
                    "uninstall",
                    None,
                ))
            }
        };

        self.record_uninstall_audit(task_id, &request, "success", Some(&result));
        match self.task_manager.complete_task(task_id) {
            Ok(task) => {
                events.push(TaskProgressEvent::new(
                    task.task_id,
                    task.kind,
                    task.status,
                    INSTALL_UNINSTALL_COMPLETED_PHASE,
                ));
                Ok(events)
            }
            Err(_) if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) => {
                Ok(events)
            }
            Err(_) => Err(self.fail_uninstall_with_audit(
                task_id,
                &request,
                events,
                "complete",
                Some(&result),
            )),
        }
    }

    fn fail_uninstall_with_audit(
        &self,
        task_id: &str,
        request: &StartUninstallTaskRequest,
        mut events: Vec<TaskProgressEvent>,
        phase: &str,
        result: Option<&UninstallModResult>,
    ) -> UninstallTaskRunError {
        if self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled) {
            return UninstallTaskRunError { events };
        }

        let _ = self.task_manager.fail_task(task_id);
        events.push(failed_uninstall_event(task_id, phase));
        self.record_uninstall_audit(task_id, request, "failure", result);
        UninstallTaskRunError { events }
    }

    fn record_uninstall_audit(
        &self,
        task_id: &str,
        request: &StartUninstallTaskRequest,
        result: &str,
        uninstall_result: Option<&UninstallModResult>,
    ) {
        let timestamp_unix_millis = self.clock.now_unix_millis().unwrap_or_default();
        let mut fields = BTreeMap::new();
        fields.insert("task_id".to_owned(), task_id.to_owned());
        fields.insert("game_id".to_owned(), request.game_id.as_str().to_owned());
        fields.insert("mod_id".to_owned(), request.mod_id.as_str().to_owned());
        fields.insert(
            "profile_id".to_owned(),
            request.profile_id.as_str().to_owned(),
        );
        fields.insert(
            "removed_file_count".to_owned(),
            uninstall_result
                .map(|result| result.removed_file_count)
                .unwrap_or_default()
                .to_string(),
        );
        fields.insert(
            "restored_file_count".to_owned(),
            uninstall_result
                .map(|result| result.restored_file_count)
                .unwrap_or_default()
                .to_string(),
        );

        let _ = self.audit_log.record(AuditLogEvent {
            timestamp_unix_millis,
            category: "install".to_owned(),
            operation: "uninstall_mod".to_owned(),
            result: result.to_owned(),
            fields,
        });
    }
}

#[derive(Default)]
pub struct GameProfileWriteLockRegistry {
    locks: Mutex<HashMap<GameProfileLockKey, GameProfileLock>>,
}

type GameProfileLockKey = (String, String);
type GameProfileLock = Arc<Mutex<()>>;

impl GameProfileWriteLockRegistry {
    fn lock_for(&self, game_id: &GameId, profile_id: &ProfileId) -> GameProfileLock {
        let mut locks = self.locks.lock().expect("write lock registry");
        locks
            .entry((game_id.as_str().to_owned(), profile_id.as_str().to_owned()))
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

fn running_event(task_id: &str, phase: &str) -> TaskProgressEvent {
    TaskProgressEvent::new(
        task_id.to_owned(),
        TaskKind::Install,
        TaskStatus::Running,
        phase,
    )
}

fn failed_event(task_id: &str, failed_phase: &str) -> TaskProgressEvent {
    let mut event = TaskProgressEvent::new(
        task_id.to_owned(),
        TaskKind::Install,
        TaskStatus::Failed,
        INSTALL_FAILED_PHASE,
    );
    event.error = Some(format!("{INSTALL_FAILED_ERROR}:{failed_phase}"));
    event
}

fn failed_uninstall_event(task_id: &str, failed_phase: &str) -> TaskProgressEvent {
    let mut event = TaskProgressEvent::new(
        task_id.to_owned(),
        TaskKind::Install,
        TaskStatus::Failed,
        INSTALL_UNINSTALL_FAILED_PHASE,
    );
    event.error = Some(format!("{INSTALL_UNINSTALL_FAILED_ERROR}:{failed_phase}"));
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BuildImportedModInstallPlanRequest, ImportedModInstallCommitRequest, InstallCommitError,
        InstallCommitResult, InstallPlanningError,
    };
    use hmm_core::{
        InstallFileProvider, InstallManifest, InstallManifestEntry, InstallPlan, InstallTargetPath,
        PackageFileId,
    };
    use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter};
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn start_install_task_returns_queued_install_task_without_leaking_inputs() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let service = InstallTaskService::new(Arc::clone(&task_manager));

        let task = service
            .start_install_task(StartInstallTaskRequest {
                game_id: GameId::mhw(),
                mod_id: ModId::new("visible-mod-id"),
                profile_id: ProfileId::new("default-profile"),
                layer: FileLayer::new("base", 0),
            })
            .expect("install task starts");

        assert!(task.task_id.starts_with("install-"));
        assert!(!task.task_id.contains("visible-mod-id"));
        assert!(!task.task_id.contains("default-profile"));
        assert_eq!(task.kind, crate::TaskKind::Install);
        assert_eq!(task.status, crate::TaskStatus::Queued);
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Queued)
        );
    }

    #[test]
    fn start_uninstall_task_returns_queued_install_task_without_leaking_inputs() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let service = UninstallTaskService::new(Arc::clone(&task_manager));

        let task = service
            .start_uninstall_task(StartUninstallTaskRequest {
                game_id: GameId::mhw(),
                mod_id: ModId::new("visible-mod-id"),
                profile_id: ProfileId::new("default-profile"),
            })
            .expect("uninstall task starts");

        assert!(task.task_id.starts_with("install-"));
        assert!(!task.task_id.contains("visible-mod-id"));
        assert!(!task.task_id.contains("default-profile"));
        assert_eq!(task.kind, crate::TaskKind::Install);
        assert_eq!(task.status, crate::TaskStatus::Queued);
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Queued)
        );
    }

    #[test]
    fn run_install_task_commits_imported_mod_plan_and_records_sanitized_success_audit() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let planner = Arc::new(RecordingInstallPlanner::new(sample_plan()));
        let committer = Arc::new(RecordingInstallCommitter::new(sample_manifest()));
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            planner.clone(),
            committer.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
        );
        let request = sample_request();

        let events = runner
            .run_install_task(&task.task_id, request.clone())
            .expect("install task succeeds");

        assert_eq!(
            events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.plan.building",
                "install.commit.processing",
                "install.completed"
            ]
        );
        assert!(events.iter().all(|event| event.task_id == task.task_id));
        assert!(events
            .iter()
            .all(|event| event.kind == crate::TaskKind::Install));
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Completed)
        );
        assert_eq!(
            planner.take_requests(),
            vec![BuildImportedModInstallPlanRequest {
                game_id: request.game_id.clone(),
                mod_id: request.mod_id.clone(),
                layer: request.layer.clone(),
            }]
        );
        assert_eq!(committer.take_profiles(), vec!["default".to_owned()]);

        let event = audit_log.take_event().expect("audit event recorded");
        assert_eq!(event.timestamp_unix_millis, 42);
        assert_eq!(event.category, "install");
        assert_eq!(event.operation, "commit_imported_mod");
        assert_eq!(event.result, "success");
        assert_eq!(event.fields["task_id"], task.task_id);
        assert_eq!(event.fields["game_id"], "mhw");
        assert_eq!(event.fields["mod_id"], "mod-a");
        assert_eq!(event.fields["profile_id"], "default");
        assert_eq!(event.fields["action_count"], "1");
        let serialized = serde_json::to_string(&event).expect("serialize audit event");
        assert!(!serialized.contains("nativePC/models/player.mod3"));
        assert!(!serialized.contains("C:/"));
        assert!(!serialized.contains('\\'));
    }

    #[test]
    fn run_uninstall_task_executes_uninstall_and_records_sanitized_success_audit() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let uninstaller = Arc::new(RecordingUninstaller::new(UninstallModResult {
            manifest: sample_manifest(),
            removed_file_count: 1,
            restored_file_count: 0,
        }));
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = UninstallTaskRunner::new(
            Arc::clone(&task_manager),
            uninstaller.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
        );
        let request = StartUninstallTaskRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new("mod-a"),
            profile_id: ProfileId::new("default"),
        };

        let events = runner
            .run_uninstall_task(&task.task_id, request.clone())
            .expect("uninstall task succeeds");

        assert_eq!(
            events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.uninstall.processing",
                "install.uninstall.completed"
            ]
        );
        assert!(events.iter().all(|event| event.task_id == task.task_id));
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Completed)
        );
        assert_eq!(uninstaller.take_requests(), vec![request.clone()]);

        let event = audit_log.take_event().expect("audit event recorded");
        assert_eq!(event.timestamp_unix_millis, 42);
        assert_eq!(event.category, "install");
        assert_eq!(event.operation, "uninstall_mod");
        assert_eq!(event.result, "success");
        assert_eq!(event.fields["task_id"], task.task_id);
        assert_eq!(event.fields["game_id"], "mhw");
        assert_eq!(event.fields["mod_id"], "mod-a");
        assert_eq!(event.fields["profile_id"], "default");
        assert_eq!(event.fields["removed_file_count"], "1");
        assert_eq!(event.fields["restored_file_count"], "0");
        let serialized = serde_json::to_string(&event).expect("serialize audit event");
        assert!(!serialized.contains("nativePC/models/player.mod3"));
        assert!(!serialized.contains("C:/"));
        assert!(!serialized.contains('\\'));
    }

    #[test]
    fn install_and_uninstall_share_game_profile_write_lock_when_configured() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let install_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("install task can be created");
        let uninstall_task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("uninstall task can be created");
        let write_locks = Arc::new(GameProfileWriteLockRegistry::default());
        let (commit_started_tx, commit_started_rx) = mpsc::channel();
        let (release_commit_tx, release_commit_rx) = mpsc::channel();
        let (uninstall_started_tx, uninstall_started_rx) = mpsc::channel();

        let install_runner = InstallTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            Arc::new(RecordingInstallPlanner::new(sample_plan())),
            Arc::new(BlockingInstallCommitter {
                manifest: sample_manifest(),
                started: Mutex::new(Some(commit_started_tx)),
                release: Mutex::new(release_commit_rx),
            }),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            Arc::clone(&write_locks),
        );
        let uninstall_runner = UninstallTaskRunner::with_write_locks(
            Arc::clone(&task_manager),
            Arc::new(NotifyingUninstaller {
                result: UninstallModResult {
                    manifest: sample_manifest(),
                    removed_file_count: 1,
                    restored_file_count: 0,
                },
                started: Mutex::new(Some(uninstall_started_tx)),
            }),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock),
            write_locks,
        );

        let install_task_id = install_task.task_id.clone();
        let install_handle = thread::spawn(move || {
            install_runner.run_install_task(&install_task_id, sample_request())
        });
        commit_started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("install commit should enter write section");

        let uninstall_task_id = uninstall_task.task_id.clone();
        let uninstall_handle = thread::spawn(move || {
            uninstall_runner.run_uninstall_task(
                &uninstall_task_id,
                StartUninstallTaskRequest {
                    game_id: GameId::mhw(),
                    mod_id: ModId::new("mod-a"),
                    profile_id: ProfileId::new("default"),
                },
            )
        });

        wait_for_status(
            &task_manager,
            &uninstall_task.task_id,
            crate::TaskStatus::Running,
        );
        assert!(
            uninstall_started_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err(),
            "uninstall must wait while install commit holds the same game/profile write lock"
        );

        release_commit_tx
            .send(())
            .expect("install commit can be released");
        uninstall_started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("uninstall should enter after install releases write lock");
        assert!(install_handle
            .join()
            .expect("install thread should not panic")
            .is_ok());
        assert!(uninstall_handle
            .join()
            .expect("uninstall thread should not panic")
            .is_ok());
    }

    #[test]
    fn run_install_task_does_not_emit_failed_when_task_is_cancelled_during_commit() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let planner = Arc::new(RecordingInstallPlanner::new(sample_plan()));
        let committer = Arc::new(CancellingInstallCommitter {
            task_manager: Arc::clone(&task_manager),
            task_id: task.task_id.clone(),
            manifest: sample_manifest(),
        });
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            planner,
            committer,
            audit_log.clone(),
            Arc::new(FixedClock),
        );

        let events = runner
            .run_install_task(&task.task_id, sample_request())
            .expect("commit succeeds even if task was cancelled during commit");

        assert_eq!(
            events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec!["install.plan.building", "install.commit.processing"]
        );
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Cancelled)
        );
        let event = audit_log.take_event().expect("audit event recorded");
        assert_eq!(event.result, "success");
        assert_eq!(event.fields["task_id"], task.task_id);
    }

    #[test]
    fn run_install_task_stops_before_commit_when_cancelled_after_planning() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let planner = Arc::new(CancellingInstallPlanner {
            task_manager: Arc::clone(&task_manager),
            task_id: task.task_id.clone(),
            plan: sample_plan(),
        });
        let committer = Arc::new(RecordingInstallCommitter::new(sample_manifest()));
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            planner,
            committer.clone(),
            audit_log.clone(),
            Arc::new(FixedClock),
        );

        let events = runner
            .run_install_task(&task.task_id, sample_request())
            .expect("cancelled task should stop without failing");

        assert_eq!(
            events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec!["install.plan.building"]
        );
        assert_eq!(
            task_manager.task_status(&task.task_id),
            Some(crate::TaskStatus::Cancelled)
        );
        assert!(committer.take_profiles().is_empty());
        assert!(audit_log.take_event().is_none());
    }

    #[test]
    fn run_install_task_preserves_action_count_in_failure_audit_after_planning() {
        let task_manager = Arc::new(crate::TaskManager::new());
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("task can be created");
        let planner = Arc::new(RecordingInstallPlanner::new(sample_plan()));
        let committer = Arc::new(FailingInstallCommitter);
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let runner = InstallTaskRunner::new(
            Arc::clone(&task_manager),
            planner,
            committer,
            audit_log.clone(),
            Arc::new(FixedClock),
        );

        let error = runner
            .run_install_task(&task.task_id, sample_request())
            .expect_err("commit failure should fail the task");

        assert_eq!(
            error
                .events
                .iter()
                .map(|event| event.phase.as_str())
                .collect::<Vec<_>>(),
            vec![
                "install.plan.building",
                "install.commit.processing",
                "install.failed"
            ]
        );
        let event = audit_log.take_event().expect("failure audit recorded");
        assert_eq!(event.result, "failure");
        assert_eq!(event.fields["action_count"], "1");
    }

    fn sample_request() -> StartInstallTaskRequest {
        StartInstallTaskRequest {
            game_id: GameId::mhw(),
            mod_id: ModId::new("mod-a"),
            profile_id: ProfileId::new("default"),
            layer: FileLayer::new("base", 0),
        }
    }

    fn sample_plan() -> InstallPlan {
        InstallPlan::from_providers(vec![sample_provider()])
    }

    fn sample_manifest() -> InstallManifest {
        InstallManifest {
            profile_id: ProfileId::new("default"),
            entries: vec![InstallManifestEntry {
                target_path: sample_target(),
                mod_id: ModId::new("mod-a"),
                package_file_id: PackageFileId::new("nativePC/models/player.mod3"),
                layer: FileLayer::new("base", 0),
                backup_ref: None,
                installed_file: None,
            }],
        }
    }

    fn sample_provider() -> InstallFileProvider {
        InstallFileProvider::new(
            ModId::new("mod-a"),
            PackageFileId::new("nativePC/models/player.mod3"),
            sample_target(),
            FileLayer::new("base", 0),
        )
    }

    fn sample_target() -> InstallTargetPath {
        InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"])
            .expect("sample target is valid")
    }

    fn wait_for_status(
        task_manager: &crate::TaskManager,
        task_id: &str,
        expected: crate::TaskStatus,
    ) {
        for _ in 0..100 {
            if task_manager.task_status(task_id) == Some(expected) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }

        panic!("task {task_id} did not reach expected status {expected:?}");
    }

    struct RecordingInstallPlanner {
        plan: InstallPlan,
        requests: Mutex<Vec<BuildImportedModInstallPlanRequest>>,
    }

    impl RecordingInstallPlanner {
        fn new(plan: InstallPlan) -> Self {
            Self {
                plan,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn take_requests(&self) -> Vec<BuildImportedModInstallPlanRequest> {
            std::mem::take(&mut *self.requests.lock().expect("requests"))
        }
    }

    impl ImportedModInstallPlanner for RecordingInstallPlanner {
        fn build_imported_mod_install_plan(
            &self,
            request: BuildImportedModInstallPlanRequest,
        ) -> Result<InstallPlan, InstallPlanningError> {
            self.requests.lock().expect("requests").push(request);
            Ok(self.plan.clone())
        }
    }

    struct CancellingInstallPlanner {
        task_manager: Arc<crate::TaskManager>,
        task_id: String,
        plan: InstallPlan,
    }

    impl ImportedModInstallPlanner for CancellingInstallPlanner {
        fn build_imported_mod_install_plan(
            &self,
            _request: BuildImportedModInstallPlanRequest,
        ) -> Result<InstallPlan, InstallPlanningError> {
            self.task_manager
                .cancel_task(&self.task_id)
                .expect("task can be cancelled after planning");
            Ok(self.plan.clone())
        }
    }

    struct RecordingInstallCommitter {
        manifest: InstallManifest,
        profiles: Mutex<Vec<String>>,
    }

    impl RecordingInstallCommitter {
        fn new(manifest: InstallManifest) -> Self {
            Self {
                manifest,
                profiles: Mutex::new(Vec::new()),
            }
        }

        fn take_profiles(&self) -> Vec<String> {
            std::mem::take(&mut *self.profiles.lock().expect("profiles"))
        }
    }

    impl InstallPlanCommitter for RecordingInstallCommitter {
        fn commit_install_plan(
            &self,
            request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            self.profiles
                .lock()
                .expect("profiles")
                .push(request.profile_id.as_str().to_owned());
            Ok(InstallCommitResult {
                manifest: self.manifest.clone(),
            })
        }
    }

    struct BlockingInstallCommitter {
        manifest: InstallManifest,
        started: Mutex<Option<mpsc::Sender<()>>>,
        release: Mutex<mpsc::Receiver<()>>,
    }

    impl InstallPlanCommitter for BlockingInstallCommitter {
        fn commit_install_plan(
            &self,
            _request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            if let Some(started) = self.started.lock().expect("started").take() {
                started.send(()).expect("commit start can be signalled");
            }
            self.release
                .lock()
                .expect("release")
                .recv_timeout(Duration::from_secs(2))
                .expect("commit release should arrive");

            Ok(InstallCommitResult {
                manifest: self.manifest.clone(),
            })
        }
    }

    struct RecordingUninstaller {
        result: UninstallModResult,
        requests: Mutex<Vec<StartUninstallTaskRequest>>,
    }

    impl RecordingUninstaller {
        fn new(result: UninstallModResult) -> Self {
            Self {
                result,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn take_requests(&self) -> Vec<StartUninstallTaskRequest> {
            std::mem::take(&mut *self.requests.lock().expect("requests"))
        }
    }

    impl ModUninstaller for RecordingUninstaller {
        fn uninstall_mod(
            &self,
            request: StartUninstallTaskRequest,
        ) -> Result<UninstallModResult, UninstallModError> {
            self.requests.lock().expect("requests").push(request);
            Ok(self.result.clone())
        }
    }

    struct NotifyingUninstaller {
        result: UninstallModResult,
        started: Mutex<Option<mpsc::Sender<()>>>,
    }

    impl ModUninstaller for NotifyingUninstaller {
        fn uninstall_mod(
            &self,
            _request: StartUninstallTaskRequest,
        ) -> Result<UninstallModResult, UninstallModError> {
            if let Some(started) = self.started.lock().expect("started").take() {
                started.send(()).expect("uninstall start can be signalled");
            }
            Ok(self.result.clone())
        }
    }

    struct FailingInstallCommitter;

    impl InstallPlanCommitter for FailingInstallCommitter {
        fn commit_install_plan(
            &self,
            _request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            Err(InstallCommitError::Failed {
                phase: crate::InstallCommitPhase::Write,
            })
        }
    }

    struct CancellingInstallCommitter {
        task_manager: Arc<crate::TaskManager>,
        task_id: String,
        manifest: InstallManifest,
    }

    impl InstallPlanCommitter for CancellingInstallCommitter {
        fn commit_install_plan(
            &self,
            _request: ImportedModInstallCommitRequest,
        ) -> Result<InstallCommitResult, InstallCommitError> {
            self.task_manager
                .cancel_task(&self.task_id)
                .expect("task can be cancelled during commit");
            Ok(InstallCommitResult {
                manifest: self.manifest.clone(),
            })
        }
    }

    #[derive(Default)]
    struct RecordingAuditLogWriter {
        event: Mutex<Option<AuditLogEvent>>,
    }

    impl RecordingAuditLogWriter {
        fn take_event(&self) -> Option<AuditLogEvent> {
            self.event.lock().expect("event").take()
        }
    }

    impl AuditLogWriter for RecordingAuditLogWriter {
        fn record(&self, event: AuditLogEvent) -> anyhow::Result<()> {
            *self.event.lock().expect("event") = Some(event);
            Ok(())
        }
    }

    struct FixedClock;

    impl AppClock for FixedClock {
        fn now_unix_millis(&self) -> anyhow::Result<u128> {
            Ok(42)
        }
    }
}
