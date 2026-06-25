use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use hmm_core::{FileLayer, GameId, InstallPlan, ModId, ProfileId};
use hmm_ports::{AppClock, AuditLogEvent, AuditLogWriter};

use crate::{
    BuildImportedModInstallPlanRequest, CommitInstallPlanRequest, InstallCommitError,
    InstallCommitResult, InstallCommitService, InstallPlanningError, InstallPlanningService,
    TaskKind, TaskManager, TaskManagerError, TaskProgressEvent, TaskStarted, TaskStatus,
};

const INSTALL_PLAN_BUILDING_PHASE: &str = "install.plan.building";
const INSTALL_COMMIT_PROCESSING_PHASE: &str = "install.commit.processing";
const INSTALL_COMPLETED_PHASE: &str = "install.completed";
const INSTALL_FAILED_PHASE: &str = "install.failed";
const INSTALL_FAILED_ERROR: &str = "install_failed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartInstallTaskRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub profile_id: ProfileId,
    pub layer: FileLayer,
}

pub struct InstallTaskService {
    task_manager: Arc<TaskManager>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallTaskRunError {
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

pub struct InstallTaskRunner {
    task_manager: Arc<TaskManager>,
    planner: Arc<dyn ImportedModInstallPlanner>,
    committer: Arc<dyn InstallPlanCommitter>,
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
    write_locks: Arc<GameProfileWriteLocks>,
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

impl InstallTaskRunner {
    pub fn new(
        task_manager: Arc<TaskManager>,
        planner: Arc<dyn ImportedModInstallPlanner>,
        committer: Arc<dyn InstallPlanCommitter>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            task_manager,
            planner,
            committer,
            audit_log,
            clock,
            write_locks: Arc::new(GameProfileWriteLocks::default()),
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
                Err(_) => return Err(self.fail_with_audit(task_id, &request, events, "planning")),
            };
        let action_count = plan.actions.len();

        events.push(running_event(task_id, INSTALL_COMMIT_PROCESSING_PHASE));
        let write_lock = self
            .write_locks
            .lock_for(&request.game_id, &request.profile_id);
        let commit_result = {
            let _guard = write_lock
                .lock()
                .map_err(|_| self.fail_with_audit(task_id, &request, events.clone(), "lock"))?;
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
            Err(_) => return Err(self.fail_with_audit(task_id, &request, events, "commit")),
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
            Err(_) => Err(self.fail_with_audit(task_id, &request, events, "complete")),
        }
    }

    fn fail_with_audit(
        &self,
        task_id: &str,
        request: &StartInstallTaskRequest,
        mut events: Vec<TaskProgressEvent>,
        phase: &str,
    ) -> InstallTaskRunError {
        let _ = self.task_manager.fail_task(task_id);
        events.push(failed_event(task_id, phase));
        self.record_audit(task_id, request, "failure", 0);
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

#[derive(Default)]
struct GameProfileWriteLocks {
    locks: Mutex<HashMap<(String, String), Arc<Mutex<()>>>>,
}

impl GameProfileWriteLocks {
    fn lock_for(&self, game_id: &GameId, profile_id: &ProfileId) -> Arc<Mutex<()>> {
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
    use std::sync::Arc;
    use std::sync::Mutex;

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
