//! Storage-root migration task (#275, slice 2).
//!
//! Moves every package below `<source>/sandboxes` into a new root without ever leaving the
//! library half-migrated: packages are copied and verified one by one, the setting is switched
//! only after the last package passed, and the source copies are deleted at the *next* start,
//! once the new root is in effect. Until then the running process keeps reading the source, so
//! nothing it does in the meantime breaks. A journal is persisted before every irreversible
//! step so a crash can be finished or rolled back by [`settle_pending_mod_storage_migration`].
//!
//! Sandbox writers are refused for the whole session through [`ModStorageWriteGate`]: while
//! copying, a write to the source would be missed; after the switch, it would land in a root
//! nobody reads after the restart.

use crate::task_manager::observe_task_progress;
use crate::{
    AppSettingsService, ModStorageWriteGate, ModStorageWriteGateError, TaskKind, TaskManager,
    TaskProgressEvent, TaskProgressObserver, TaskStarted, TaskStatus,
};
use hmm_core::GameId;
use hmm_ports::{
    AppClock, CancellationToken, GameConfigRepository, ModStorageDirectoryError,
    ModStorageDirectoryInspectionRequest, ModStorageDirectoryInspector, ModStorageMigrationError,
    ModStorageMigrationJournal, ModStorageMigrationJournalRepository, ModStorageMigrationState,
    ModStorageMigrator, MOD_STORAGE_MIGRATION_JOURNAL_VERSION,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

pub const MOD_STORAGE_MIGRATION_QUEUED_PHASE: &str = "mod_storage.migration.queued";
pub const MOD_STORAGE_MIGRATION_COPYING_PHASE: &str = "mod_storage.migration.copying";
pub const MOD_STORAGE_MIGRATION_VERIFYING_PHASE: &str = "mod_storage.migration.verifying";
pub const MOD_STORAGE_MIGRATION_SWITCHING_PHASE: &str = "mod_storage.migration.switching";
pub const MOD_STORAGE_MIGRATION_COMPLETED_PHASE: &str = "mod_storage.migration.completed";
pub const MOD_STORAGE_MIGRATION_FAILED_PHASE: &str = "mod_storage.migration.failed";
/// Emitted by `cancel_task` the moment the request is accepted; the runner is still removing
/// the copies it made. The terminal `cancelled` follows once the rollback finished.
pub const MOD_STORAGE_MIGRATION_CANCELLING_PHASE: &str = "mod_storage.migration.cancelling";
pub const MOD_STORAGE_MIGRATION_CANCELLED_PHASE: &str = "mod_storage.migration.cancelled";

/// A registered, not yet running migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModStorageMigrationLaunch {
    pub task: TaskStarted,
    pub source_root: PathBuf,
    pub target_root: PathBuf,
    /// Value written to `settings.json` on success (`None` = back to the default root).
    pub configured_target: Option<PathBuf>,
}

/// Refusals before a task exists. Failures of the migration itself are terminal task events.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModStorageMigrationTaskError {
    #[error("{0}")]
    Gate(ModStorageWriteGateError),
    #[error("{0}")]
    Directory(ModStorageDirectoryError),
    #[error("an import task is still queued or running")]
    ImportsActive,
    #[error("game configuration unavailable")]
    GameConfigUnavailable,
    #[error("task registry unavailable")]
    TaskUnavailable,
}

impl ModStorageMigrationTaskError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Gate(error) => error.code(),
            Self::Directory(error) => error.code(),
            Self::ImportsActive => "mod_storage_migration_imports_active",
            Self::GameConfigUnavailable => "game_config_unavailable",
            Self::TaskUnavailable => "mod_storage_migration_task_unavailable",
        }
    }
}

impl From<ModStorageWriteGateError> for ModStorageMigrationTaskError {
    fn from(error: ModStorageWriteGateError) -> Self {
        Self::Gate(error)
    }
}

impl From<ModStorageDirectoryError> for ModStorageMigrationTaskError {
    fn from(error: ModStorageDirectoryError) -> Self {
        Self::Directory(error)
    }
}

/// Stable code carried by the `failed` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModStorageMigrationFailure {
    Migration(ModStorageMigrationError),
    /// Every package was copied and verified, but `settings.json` could not be written; the
    /// copies were rolled back and the source stays in effect.
    SettingsUnavailable,
}

impl ModStorageMigrationFailure {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Migration(error) => error.code(),
            Self::SettingsUnavailable => "mod_storage_migration_settings_unavailable",
        }
    }
}

pub struct ModStorageMigrationTaskServiceDependencies {
    pub task_manager: Arc<TaskManager>,
    pub write_gate: Arc<ModStorageWriteGate>,
    /// Single writer for `settings.json`.
    pub app_settings: Arc<AppSettingsService>,
    pub inspector: Arc<dyn ModStorageDirectoryInspector>,
    pub migrator: Arc<dyn ModStorageMigrator>,
    pub journal: Arc<dyn ModStorageMigrationJournalRepository>,
    pub game_config: Arc<dyn GameConfigRepository>,
    pub game_ids: Vec<GameId>,
    pub clock: Arc<dyn AppClock>,
    /// Root the running process resolved at startup — the migration source.
    pub effective_root: PathBuf,
    pub default_root: PathBuf,
}

pub struct ModStorageMigrationTaskService {
    task_manager: Arc<TaskManager>,
    write_gate: Arc<ModStorageWriteGate>,
    app_settings: Arc<AppSettingsService>,
    inspector: Arc<dyn ModStorageDirectoryInspector>,
    migrator: Arc<dyn ModStorageMigrator>,
    journal: Arc<dyn ModStorageMigrationJournalRepository>,
    game_config: Arc<dyn GameConfigRepository>,
    game_ids: Vec<GameId>,
    clock: Arc<dyn AppClock>,
    effective_root: PathBuf,
    default_root: PathBuf,
}

enum MigrationStop {
    Cancelled,
    Failed(ModStorageMigrationFailure),
    /// The task registry itself broke; there is no honest terminal event to emit.
    TaskUnavailable,
}

impl From<ModStorageMigrationError> for MigrationStop {
    fn from(error: ModStorageMigrationError) -> Self {
        match error {
            ModStorageMigrationError::Cancelled => Self::Cancelled,
            other => Self::Failed(ModStorageMigrationFailure::Migration(other)),
        }
    }
}

impl ModStorageMigrationTaskService {
    pub fn new(dependencies: ModStorageMigrationTaskServiceDependencies) -> Self {
        Self {
            task_manager: dependencies.task_manager,
            write_gate: dependencies.write_gate,
            app_settings: dependencies.app_settings,
            inspector: dependencies.inspector,
            migrator: dependencies.migrator,
            journal: dependencies.journal,
            game_config: dependencies.game_config,
            game_ids: dependencies.game_ids,
            clock: dependencies.clock,
            effective_root: dependencies.effective_root,
            default_root: dependencies.default_root,
        }
    }

    /// Validates and claims the target, then registers a queued task and freezes sandbox
    /// writes — both under the gate, so an import admitted a moment earlier is seen and the
    /// migration is refused with `ImportsActive` instead of racing it.
    pub fn start(
        &self,
        directory: Option<PathBuf>,
    ) -> Result<ModStorageMigrationLaunch, ModStorageMigrationTaskError> {
        self.write_gate.ensure_open()?;
        let target_root = directory
            .clone()
            .unwrap_or_else(|| self.default_root.clone());
        if target_root == self.effective_root {
            return Err(ModStorageDirectoryError::OverlapsCurrentRoot.into());
        }
        match &directory {
            Some(directory) => {
                let game_roots = self.game_roots()?;
                self.inspector
                    .inspect(ModStorageDirectoryInspectionRequest {
                        path: directory,
                        exclusive_roots: &game_roots,
                        current_root: Some(&self.effective_root),
                    })?;
                self.inspector.claim(directory)?;
            }
            None => {
                // The default root is HMM's own app-data directory: no marker, no probe. It still
                // must not nest with the current root (a custom root created inside app-data).
                if self
                    .inspector
                    .directories_overlap(&target_root, &self.effective_root)
                {
                    return Err(ModStorageDirectoryError::OverlapsCurrentRoot.into());
                }
            }
        }

        let mut created = None;
        self.write_gate
            .begin_migration(|| -> Result<(), ModStorageMigrationTaskError> {
                if self
                    .task_manager
                    .has_active_task_of_kind(TaskKind::ModImport)
                    .map_err(|_| ModStorageMigrationTaskError::TaskUnavailable)?
                {
                    return Err(ModStorageMigrationTaskError::ImportsActive);
                }
                let task = self
                    .task_manager
                    .create_task(TaskKind::ModStorageMigration)
                    .map_err(|_| ModStorageMigrationTaskError::TaskUnavailable)?;
                created = Some(task);
                Ok(())
            })?;
        let task = created.ok_or(ModStorageMigrationTaskError::TaskUnavailable)?;
        Ok(ModStorageMigrationLaunch {
            task: TaskStarted {
                task_id: task.task_id,
                kind: task.kind,
                status: task.status,
            },
            source_root: self.effective_root.clone(),
            target_root,
            configured_target: directory,
        })
    }

    /// Closes a launch the command layer could not hand to the runner (queued event failed to
    /// emit) and reopens the gate — nothing has been copied yet.
    pub fn abort_queued(
        &self,
        launch: &ModStorageMigrationLaunch,
    ) -> Result<(), ModStorageMigrationTaskError> {
        let result = match self.task_manager.task_status(&launch.task.task_id) {
            Some(TaskStatus::Queued | TaskStatus::Running) => self
                .task_manager
                .fail_task(&launch.task.task_id)
                .map(|_| ())
                .map_err(|_| ModStorageMigrationTaskError::TaskUnavailable),
            Some(TaskStatus::Failed | TaskStatus::Cancelled | TaskStatus::Completed) => Ok(()),
            None => Err(ModStorageMigrationTaskError::TaskUnavailable),
        };
        self.write_gate.end_migration(false);
        result
    }

    pub fn run(
        &self,
        launch: ModStorageMigrationLaunch,
    ) -> Result<Vec<TaskProgressEvent>, ModStorageMigrationTaskError> {
        self.run_with_observer(launch, &crate::task_manager::noop_task_progress_observer())
    }

    /// Runs the migration and returns the event sequence; live progress goes through
    /// `observer`. Only a broken task registry returns `Err` — failure and cancellation are
    /// terminal events, and both leave the source root in effect with the target rolled back.
    pub fn run_with_observer<O: TaskProgressObserver + ?Sized>(
        &self,
        launch: ModStorageMigrationLaunch,
        observer: &O,
    ) -> Result<Vec<TaskProgressEvent>, ModStorageMigrationTaskError> {
        let task_id = launch.task.task_id.clone();
        let cancelled_event = || {
            migration_event(
                &launch,
                TaskStatus::Cancelled,
                MOD_STORAGE_MIGRATION_CANCELLED_PHASE,
                None,
                None,
            )
        };
        if self.is_cancelled(&task_id) {
            self.write_gate.end_migration(false);
            return Ok(vec![cancelled_event()]);
        }
        match self.task_manager.start_task(&task_id) {
            Ok(_) => {}
            Err(_) if self.is_cancelled(&task_id) => {
                self.write_gate.end_migration(false);
                return Ok(vec![cancelled_event()]);
            }
            Err(_) => {
                self.write_gate.end_migration(false);
                return Err(ModStorageMigrationTaskError::TaskUnavailable);
            }
        }

        let mut events = Vec::new();
        let token = TaskManagerCancellationToken {
            task_manager: Arc::clone(&self.task_manager),
            task_id: task_id.clone(),
        };
        let mut copied = Vec::new();
        let mut package_count = 0;
        let outcome = self.migrate(
            &launch,
            &token,
            &mut copied,
            &mut package_count,
            &mut events,
            observer,
        );
        match outcome {
            Ok(()) => {
                self.write_gate.end_migration(true);
                self.task_manager
                    .complete_task(&task_id)
                    .map_err(|_| ModStorageMigrationTaskError::TaskUnavailable)?;
                observe_task_progress(
                    &mut events,
                    observer,
                    migration_event(
                        &launch,
                        TaskStatus::Completed,
                        MOD_STORAGE_MIGRATION_COMPLETED_PHASE,
                        Some((package_count, package_count)),
                        None,
                    ),
                );
                Ok(events)
            }
            Err(MigrationStop::Cancelled) => {
                // Cancellation is only ever observed through the registry (token), so the task
                // already carries the cancelled status; nothing to transition.
                self.roll_back(&launch, &copied);
                observe_task_progress(&mut events, observer, cancelled_event());
                Ok(events)
            }
            Err(MigrationStop::Failed(failure)) => {
                self.roll_back(&launch, &copied);
                if matches!(
                    self.task_manager.task_status(&task_id),
                    Some(TaskStatus::Queued | TaskStatus::Running)
                ) {
                    self.task_manager
                        .fail_task(&task_id)
                        .map_err(|_| ModStorageMigrationTaskError::TaskUnavailable)?;
                }
                observe_task_progress(
                    &mut events,
                    observer,
                    migration_event(
                        &launch,
                        TaskStatus::Failed,
                        MOD_STORAGE_MIGRATION_FAILED_PHASE,
                        None,
                        Some(failure.code()),
                    ),
                );
                Ok(events)
            }
            Err(MigrationStop::TaskUnavailable) => {
                self.roll_back(&launch, &copied);
                Err(ModStorageMigrationTaskError::TaskUnavailable)
            }
        }
    }

    fn migrate<O: TaskProgressObserver + ?Sized>(
        &self,
        launch: &ModStorageMigrationLaunch,
        token: &TaskManagerCancellationToken,
        copied: &mut Vec<String>,
        package_count: &mut u64,
        events: &mut Vec<TaskProgressEvent>,
        observer: &O,
    ) -> Result<(), MigrationStop> {
        let task_id = &launch.task.task_id;
        let packages = self.migrator.list_packages(&launch.source_root)?;
        let total = packages.len() as u64;
        *package_count = total;
        let mut journal = ModStorageMigrationJournal {
            version: MOD_STORAGE_MIGRATION_JOURNAL_VERSION,
            state: ModStorageMigrationState::Copying,
            source_root: launch.source_root.clone(),
            target_root: launch.target_root.clone(),
            packages: packages.clone(),
            // Informational only (diagnostics); a broken clock must not block a migration.
            started_at_unix_millis: self.clock.now_unix_millis().unwrap_or(0),
        };
        self.journal
            .save(&journal)
            .map_err(|_| ModStorageMigrationError::JournalUnavailable)?;

        for (index, package_id) in packages.iter().enumerate() {
            if token.is_cancelled() {
                return Err(MigrationStop::Cancelled);
            }
            observe_task_progress(
                events,
                observer,
                migration_event(
                    launch,
                    TaskStatus::Running,
                    MOD_STORAGE_MIGRATION_COPYING_PHASE,
                    Some((index as u64, total)),
                    None,
                ),
            );
            // Registered before copying so an interrupted, partial copy is rolled back too.
            copied.push(package_id.clone());
            self.migrator.copy_package(
                &launch.source_root,
                &launch.target_root,
                package_id,
                token,
            )?;
            observe_task_progress(
                events,
                observer,
                migration_event(
                    launch,
                    TaskStatus::Running,
                    MOD_STORAGE_MIGRATION_VERIFYING_PHASE,
                    Some((index as u64, total)),
                    None,
                ),
            );
            self.migrator.verify_package(
                &launch.source_root,
                &launch.target_root,
                package_id,
                token,
            )?;
        }

        // Point of no return: from here a cancel request is deferred by the barrier, so the
        // journal and the setting cannot diverge because of a late cancellation.
        match self.task_manager.block_task_cancellation(task_id) {
            Ok(_) => {}
            Err(_) if token.is_cancelled() => return Err(MigrationStop::Cancelled),
            Err(_) => return Err(MigrationStop::TaskUnavailable),
        }
        observe_task_progress(
            events,
            observer,
            migration_event(
                launch,
                TaskStatus::Running,
                MOD_STORAGE_MIGRATION_SWITCHING_PHASE,
                Some((total, total)),
                None,
            ),
        );
        journal.state = ModStorageMigrationState::Switched;
        self.journal
            .save(&journal)
            .map_err(|_| ModStorageMigrationError::JournalUnavailable)?;
        self.app_settings
            .update_mod_storage_dir(launch.configured_target.clone())
            .map_err(|_| MigrationStop::Failed(ModStorageMigrationFailure::SettingsUnavailable))?;
        Ok(())
    }

    /// Removes the copies made so far and the journal; the setting was never switched. When a
    /// copy cannot be removed the journal is kept so the next start retries the rollback.
    fn roll_back(&self, launch: &ModStorageMigrationLaunch, copied: &[String]) {
        let mut complete = true;
        for package_id in copied {
            if self
                .migrator
                .remove_package(&launch.target_root, package_id)
                .is_err()
            {
                complete = false;
            }
        }
        if complete {
            let _ = self.journal.clear();
        }
        self.write_gate.end_migration(false);
    }

    fn is_cancelled(&self, task_id: &str) -> bool {
        self.task_manager.task_status(task_id) == Some(TaskStatus::Cancelled)
    }

    fn game_roots(&self) -> Result<Vec<PathBuf>, ModStorageMigrationTaskError> {
        let mut roots = Vec::new();
        for game_id in &self.game_ids {
            if let Some(instance) = self
                .game_config
                .load_game_instance(game_id)
                .map_err(|_| ModStorageMigrationTaskError::GameConfigUnavailable)?
            {
                roots.push(instance.root_dir);
            }
        }
        Ok(roots)
    }
}

/// The queued event the command layer emits before returning the task identity.
pub fn queued_mod_storage_migration_event(launch: &ModStorageMigrationLaunch) -> TaskProgressEvent {
    migration_event(
        launch,
        launch.task.status,
        MOD_STORAGE_MIGRATION_QUEUED_PHASE,
        None,
        None,
    )
}

/// `progress` is `(packages done, packages total)`; events without it carry neither field.
fn migration_event(
    launch: &ModStorageMigrationLaunch,
    status: TaskStatus,
    phase: &'static str,
    progress: Option<(u64, u64)>,
    error: Option<&'static str>,
) -> TaskProgressEvent {
    let mut event =
        TaskProgressEvent::new(launch.task.task_id.clone(), launch.task.kind, status, phase);
    if let Some((current, total)) = progress {
        event.current = Some(current);
        event.total = Some(total);
    }
    event.error = error.map(str::to_owned);
    event
}

struct TaskManagerCancellationToken {
    task_manager: Arc<TaskManager>,
    task_id: String,
}

impl CancellationToken for TaskManagerCancellationToken {
    fn is_cancelled(&self) -> bool {
        self.task_manager.task_status(&self.task_id) == Some(TaskStatus::Cancelled)
    }
}

/// What the startup settlement did with a journal left behind by a previous process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModStorageMigrationSettlement {
    NoJournal,
    /// The switched root is in effect; the source copies were removed and the journal cleared.
    SourceCleaned {
        package_count: u64,
    },
    /// The switched root is in effect but the source could not be cleaned yet (source root
    /// unavailable, a package missing from the target, or a removal failure). Journal kept.
    CleanupBlocked(ModStorageMigrationError),
    /// The setting never switched (crash while copying, or between journal and setting); the
    /// listed packages were removed from the target and the journal cleared.
    RolledBack {
        package_count: u64,
    },
    /// Rollback could not remove every listed package; journal kept for the next start.
    RollbackBlocked(ModStorageMigrationError),
    /// The effective root is unknown (settings unreadable) or names the journal's target while
    /// the journal still says `copying`; no action is safe, so the journal is kept.
    Deferred,
    JournalUnreadable(ModStorageMigrationError),
}

/// Finishes or rolls back a migration interrupted before the previous process exited. Runs
/// once per start, after the storage root is resolved and before any sandbox writer exists.
/// `effective_root` is `None` when `settings.json` could not be read.
pub fn settle_pending_mod_storage_migration(
    journal: &dyn ModStorageMigrationJournalRepository,
    migrator: &dyn ModStorageMigrator,
    effective_root: Option<&Path>,
) -> ModStorageMigrationSettlement {
    let pending = match journal.load() {
        Ok(Some(pending)) => pending,
        Ok(None) => return ModStorageMigrationSettlement::NoJournal,
        Err(error) => return ModStorageMigrationSettlement::JournalUnreadable(error),
    };
    let Some(effective_root) = effective_root else {
        return ModStorageMigrationSettlement::Deferred;
    };
    let target_in_effect = effective_root == pending.target_root;
    let package_count = pending.packages.len() as u64;
    match pending.state {
        ModStorageMigrationState::Switched if target_in_effect => {
            if !pending.source_root.is_dir() {
                return ModStorageMigrationSettlement::CleanupBlocked(
                    ModStorageMigrationError::SourceUnavailable,
                );
            }
            for package_id in &pending.packages {
                match migrator.package_exists(&pending.target_root, package_id) {
                    Ok(true) => {}
                    Ok(false) => {
                        return ModStorageMigrationSettlement::CleanupBlocked(
                            ModStorageMigrationError::TargetPackageMissing,
                        );
                    }
                    Err(error) => return ModStorageMigrationSettlement::CleanupBlocked(error),
                }
            }
            for package_id in &pending.packages {
                if let Err(error) = migrator.remove_package(&pending.source_root, package_id) {
                    return ModStorageMigrationSettlement::CleanupBlocked(error);
                }
            }
            match journal.clear() {
                Ok(()) => ModStorageMigrationSettlement::SourceCleaned { package_count },
                Err(error) => ModStorageMigrationSettlement::CleanupBlocked(error),
            }
        }
        ModStorageMigrationState::Copying if target_in_effect => {
            ModStorageMigrationSettlement::Deferred
        }
        ModStorageMigrationState::Switched | ModStorageMigrationState::Copying => {
            for package_id in &pending.packages {
                if let Err(error) = migrator.remove_package(&pending.target_root, package_id) {
                    return ModStorageMigrationSettlement::RollbackBlocked(error);
                }
            }
            match journal.clear() {
                Ok(()) => ModStorageMigrationSettlement::RolledBack { package_count },
                Err(error) => ModStorageMigrationSettlement::RollbackBlocked(error),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModStorageWriteFreeze;
    use hmm_core::{GameDirectoryStatus, GameInstance};
    use hmm_ports::{
        AppSettings, AppSettingsRepository, AppSettingsRepositoryError,
        AppSettingsRepositoryResult, GameConfigRepositoryResult, ModStorageDirectoryInspection,
        ModStoragePackageCopyReport,
    };
    use std::sync::Mutex;

    type Hook = Box<dyn FnMut(&str) + Send>;

    #[derive(Default)]
    struct FakeMigrator {
        packages: Vec<String>,
        list_error: Option<ModStorageMigrationError>,
        fail_copy_on: Option<String>,
        fail_verify_on: Option<String>,
        fail_remove_on: Option<String>,
        missing_in_target: Vec<String>,
        source_root: PathBuf,
        target_root: PathBuf,
        calls: Mutex<Vec<String>>,
        /// Called with the call label before it executes; tests use it to cancel mid-flight.
        hook: Mutex<Option<Hook>>,
    }

    impl FakeMigrator {
        fn record(&self, label: String) {
            if let Some(hook) = self.hook.lock().expect("hook lock").as_mut() {
                hook(&label);
            }
            self.calls.lock().expect("calls lock").push(label);
        }

        fn root_label(&self, root: &Path) -> &'static str {
            if root == self.source_root {
                "source"
            } else if root == self.target_root {
                "target"
            } else {
                "other"
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().expect("calls lock").clone()
        }
    }

    impl ModStorageMigrator for FakeMigrator {
        fn list_packages(
            &self,
            storage_root: &Path,
        ) -> Result<Vec<String>, ModStorageMigrationError> {
            self.record(format!("list:{}", self.root_label(storage_root)));
            if let Some(error) = self.list_error {
                return Err(error);
            }
            Ok(self.packages.clone())
        }

        fn copy_package(
            &self,
            source_root: &Path,
            target_root: &Path,
            package_id: &str,
            cancellation: &dyn CancellationToken,
        ) -> Result<ModStoragePackageCopyReport, ModStorageMigrationError> {
            assert_eq!(self.root_label(source_root), "source");
            assert_eq!(self.root_label(target_root), "target");
            self.record(format!("copy:{package_id}"));
            if cancellation.is_cancelled() {
                return Err(ModStorageMigrationError::Cancelled);
            }
            if self.fail_copy_on.as_deref() == Some(package_id) {
                return Err(ModStorageMigrationError::CopyFailed);
            }
            Ok(ModStoragePackageCopyReport {
                file_count: 1,
                byte_count: 1,
            })
        }

        fn verify_package(
            &self,
            _source_root: &Path,
            _target_root: &Path,
            package_id: &str,
            cancellation: &dyn CancellationToken,
        ) -> Result<(), ModStorageMigrationError> {
            self.record(format!("verify:{package_id}"));
            if cancellation.is_cancelled() {
                return Err(ModStorageMigrationError::Cancelled);
            }
            if self.fail_verify_on.as_deref() == Some(package_id) {
                return Err(ModStorageMigrationError::VerifyMismatch);
            }
            Ok(())
        }

        fn remove_package(
            &self,
            storage_root: &Path,
            package_id: &str,
        ) -> Result<(), ModStorageMigrationError> {
            self.record(format!(
                "remove:{}:{package_id}",
                self.root_label(storage_root)
            ));
            if self.fail_remove_on.as_deref() == Some(package_id) {
                return Err(ModStorageMigrationError::PackageUnreadable);
            }
            Ok(())
        }

        fn package_exists(
            &self,
            storage_root: &Path,
            package_id: &str,
        ) -> Result<bool, ModStorageMigrationError> {
            self.record(format!(
                "exists:{}:{package_id}",
                self.root_label(storage_root)
            ));
            Ok(!self
                .missing_in_target
                .iter()
                .any(|missing| missing == package_id))
        }
    }

    #[derive(Default)]
    struct FakeJournal {
        current: Mutex<Option<ModStorageMigrationJournal>>,
        saved: Mutex<Vec<ModStorageMigrationJournal>>,
        cleared: Mutex<usize>,
        fail_save_in_state: Option<ModStorageMigrationState>,
        fail_load: bool,
        hook: Mutex<Option<Hook>>,
    }

    impl FakeJournal {
        fn with_pending(journal: ModStorageMigrationJournal) -> Self {
            Self {
                current: Mutex::new(Some(journal)),
                ..Self::default()
            }
        }

        fn saved_states(&self) -> Vec<ModStorageMigrationState> {
            self.saved
                .lock()
                .expect("saved lock")
                .iter()
                .map(|journal| journal.state)
                .collect()
        }

        fn current(&self) -> Option<ModStorageMigrationJournal> {
            self.current.lock().expect("current lock").clone()
        }

        fn cleared(&self) -> usize {
            *self.cleared.lock().expect("cleared lock")
        }
    }

    impl ModStorageMigrationJournalRepository for FakeJournal {
        fn load(&self) -> Result<Option<ModStorageMigrationJournal>, ModStorageMigrationError> {
            if self.fail_load {
                return Err(ModStorageMigrationError::JournalUnavailable);
            }
            Ok(self.current())
        }

        fn save(
            &self,
            journal: &ModStorageMigrationJournal,
        ) -> Result<(), ModStorageMigrationError> {
            if let Some(hook) = self.hook.lock().expect("hook lock").as_mut() {
                hook(&format!("save:{:?}", journal.state));
            }
            if self.fail_save_in_state == Some(journal.state) {
                return Err(ModStorageMigrationError::JournalUnavailable);
            }
            self.saved.lock().expect("saved lock").push(journal.clone());
            *self.current.lock().expect("current lock") = Some(journal.clone());
            Ok(())
        }

        fn clear(&self) -> Result<(), ModStorageMigrationError> {
            *self.cleared.lock().expect("cleared lock") += 1;
            *self.current.lock().expect("current lock") = None;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeSettings {
        stored: Mutex<AppSettings>,
        fail_save: bool,
    }

    impl AppSettingsRepository for FakeSettings {
        fn load_settings(&self) -> AppSettingsRepositoryResult<AppSettings> {
            Ok(self.stored.lock().expect("settings lock").clone())
        }

        fn save_settings(&self, settings: &AppSettings) -> AppSettingsRepositoryResult<()> {
            if self.fail_save {
                return Err(AppSettingsRepositoryError::StorageFailed(
                    "fixture failure".to_owned(),
                ));
            }
            *self.stored.lock().expect("settings lock") = settings.clone();
            Ok(())
        }
    }

    /// (candidate, exclusive game roots, current root) as passed to `inspect`.
    type InspectionRecord = (PathBuf, Vec<PathBuf>, Option<PathBuf>);

    #[derive(Default)]
    struct FakeInspector {
        inspect_error: Option<ModStorageDirectoryError>,
        claim_error: Option<ModStorageDirectoryError>,
        overlap_pairs: Vec<(PathBuf, PathBuf)>,
        inspected: Mutex<Vec<InspectionRecord>>,
        claimed: Mutex<Vec<PathBuf>>,
    }

    impl ModStorageDirectoryInspector for FakeInspector {
        fn inspect(
            &self,
            request: ModStorageDirectoryInspectionRequest<'_>,
        ) -> Result<ModStorageDirectoryInspection, ModStorageDirectoryError> {
            self.inspected.lock().expect("inspected lock").push((
                request.path.to_path_buf(),
                request.exclusive_roots.to_vec(),
                request.current_root.map(Path::to_path_buf),
            ));
            if let Some(current_root) = request.current_root {
                if self.directories_overlap(request.path, current_root) {
                    return Err(ModStorageDirectoryError::OverlapsCurrentRoot);
                }
            }
            if let Some(error) = self.inspect_error {
                return Err(error);
            }
            Ok(ModStorageDirectoryInspection {
                exists: false,
                claimed: false,
            })
        }

        fn claim(&self, path: &Path) -> Result<(), ModStorageDirectoryError> {
            self.claimed
                .lock()
                .expect("claimed lock")
                .push(path.to_path_buf());
            match self.claim_error {
                Some(error) => Err(error),
                None => Ok(()),
            }
        }

        fn verify_claimed(&self, _path: &Path) -> Result<(), ModStorageDirectoryError> {
            Ok(())
        }

        fn sandbox_directory_has_entries(
            &self,
            _storage_root: &Path,
        ) -> Result<bool, ModStorageDirectoryError> {
            Ok(true)
        }

        fn directories_overlap(&self, left: &Path, right: &Path) -> bool {
            left == right
                || self
                    .overlap_pairs
                    .iter()
                    .any(|(a, b)| (a == left && b == right) || (a == right && b == left))
        }
    }

    struct FakeGameConfig;

    impl GameConfigRepository for FakeGameConfig {
        fn load_game_instance(
            &self,
            game_id: &GameId,
        ) -> GameConfigRepositoryResult<Option<GameInstance>> {
            Ok(Some(GameInstance {
                id: "mhw-default".to_owned(),
                game_id: game_id.clone(),
                display_name: "MHW".to_owned(),
                root_dir: game_root(),
                status: GameDirectoryStatus::Configured,
                configured_at_unix_millis: 1,
            }))
        }

        fn save_game_instance(&self, _instance: &GameInstance) -> GameConfigRepositoryResult<()> {
            Ok(())
        }
    }

    struct FixedClock;

    impl AppClock for FixedClock {
        fn now_unix_millis(&self) -> anyhow::Result<u128> {
            Ok(4_200)
        }
    }

    fn path(windows: &str, unix: &str) -> PathBuf {
        PathBuf::from(if cfg!(windows) { windows } else { unix })
    }

    fn game_root() -> PathBuf {
        path("D:\\Games\\MHW", "/games/mhw")
    }

    fn default_root() -> PathBuf {
        path("C:\\app-data\\mod-import", "/app-data/mod-import")
    }

    fn custom_root() -> PathBuf {
        path("E:\\HMMMods", "/srv/hmm-mods")
    }

    fn other_root() -> PathBuf {
        path("F:\\OtherMods", "/srv/other-mods")
    }

    fn packages() -> Vec<String> {
        vec!["mod-import-1-0".to_owned(), "mod-import-2-0".to_owned()]
    }

    struct Harness {
        task_manager: Arc<TaskManager>,
        write_gate: Arc<ModStorageWriteGate>,
        settings: Arc<FakeSettings>,
        inspector: Arc<FakeInspector>,
        migrator: Arc<FakeMigrator>,
        journal: Arc<FakeJournal>,
        service: ModStorageMigrationTaskService,
    }

    struct HarnessOptions {
        effective_root: PathBuf,
        configured: Option<PathBuf>,
        target_root: PathBuf,
        migrator: FakeMigrator,
        journal: FakeJournal,
        settings: FakeSettings,
        inspector: FakeInspector,
    }

    impl Default for HarnessOptions {
        fn default() -> Self {
            Self {
                effective_root: default_root(),
                configured: None,
                target_root: custom_root(),
                migrator: FakeMigrator {
                    packages: packages(),
                    ..FakeMigrator::default()
                },
                journal: FakeJournal::default(),
                settings: FakeSettings::default(),
                inspector: FakeInspector::default(),
            }
        }
    }

    fn harness(options: HarnessOptions) -> Harness {
        let task_manager = Arc::new(TaskManager::new());
        let write_gate = Arc::new(ModStorageWriteGate::new());
        let settings = Arc::new(FakeSettings {
            stored: Mutex::new(AppSettings {
                mod_storage_dir: options.configured,
                ..AppSettings::default()
            }),
            ..options.settings
        });
        let inspector = Arc::new(options.inspector);
        let migrator = Arc::new(FakeMigrator {
            source_root: options.effective_root.clone(),
            target_root: options.target_root,
            ..options.migrator
        });
        let journal = Arc::new(options.journal);
        let service =
            ModStorageMigrationTaskService::new(ModStorageMigrationTaskServiceDependencies {
                task_manager: Arc::clone(&task_manager),
                write_gate: Arc::clone(&write_gate),
                app_settings: Arc::new(AppSettingsService::new(Arc::clone(&settings) as Arc<_>)),
                inspector: Arc::clone(&inspector) as Arc<_>,
                migrator: Arc::clone(&migrator) as Arc<_>,
                journal: Arc::clone(&journal) as Arc<_>,
                game_config: Arc::new(FakeGameConfig),
                game_ids: vec![GameId::mhw()],
                clock: Arc::new(FixedClock),
                effective_root: options.effective_root,
                default_root: default_root(),
            });
        Harness {
            task_manager,
            write_gate,
            settings,
            inspector,
            migrator,
            journal,
            service,
        }
    }

    fn phases(events: &[TaskProgressEvent]) -> Vec<(String, Option<u64>, Option<u64>)> {
        events
            .iter()
            .map(|event| (event.phase.clone(), event.current, event.total))
            .collect()
    }

    fn stored_dir(harness: &Harness) -> Option<PathBuf> {
        harness
            .settings
            .stored
            .lock()
            .expect("settings lock")
            .mod_storage_dir
            .clone()
    }

    #[test]
    fn start_inspects_claims_registers_a_queued_task_and_freezes_writes() {
        let harness = harness(HarnessOptions::default());

        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");

        assert!(launch.task.task_id.starts_with("mod-storage-migration-"));
        assert_eq!(launch.task.kind, TaskKind::ModStorageMigration);
        assert_eq!(launch.task.status, TaskStatus::Queued);
        assert_eq!(launch.source_root, default_root());
        assert_eq!(launch.target_root, custom_root());
        assert_eq!(launch.configured_target, Some(custom_root()));
        assert_eq!(
            harness
                .inspector
                .inspected
                .lock()
                .expect("inspected")
                .as_slice(),
            [(custom_root(), vec![game_root()], Some(default_root()))]
        );
        assert_eq!(
            harness
                .inspector
                .claimed
                .lock()
                .expect("claimed")
                .as_slice(),
            [custom_root()]
        );
        assert_eq!(
            harness.write_gate.freeze(),
            ModStorageWriteFreeze::Migration
        );
        assert_eq!(
            harness.task_manager.task_status(&launch.task.task_id),
            Some(TaskStatus::Queued)
        );
        assert!(
            harness.migrator.calls().is_empty(),
            "start does no package IO"
        );
    }

    #[test]
    fn start_is_refused_while_an_import_is_active_and_leaves_the_gate_open() {
        let harness = harness(HarnessOptions::default());
        let import = harness
            .task_manager
            .create_task(TaskKind::ModImport)
            .expect("import task");

        let error = harness
            .service
            .start(Some(custom_root()))
            .expect_err("active import blocks the migration");

        assert_eq!(error, ModStorageMigrationTaskError::ImportsActive);
        assert_eq!(error.code(), "mod_storage_migration_imports_active");
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
        assert_eq!(
            harness
                .task_manager
                .has_active_task_of_kind(TaskKind::ModStorageMigration),
            Ok(false)
        );

        harness
            .task_manager
            .fail_task(&import.task_id)
            .expect("import ends");
        harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts once the import ended");
    }

    #[test]
    fn start_is_refused_while_writes_are_frozen_before_touching_the_target() {
        let harness = harness(HarnessOptions::default());
        harness
            .write_gate
            .admit_root_switch(|| Ok::<(), ()>(()))
            .expect("switch admitted")
            .expect("switch write");

        let error = harness
            .service
            .start(Some(custom_root()))
            .expect_err("frozen gate refuses");

        assert_eq!(error.code(), "mod_storage_restart_required");
        assert!(harness
            .inspector
            .claimed
            .lock()
            .expect("claimed")
            .is_empty());
    }

    #[test]
    fn start_refuses_the_current_root_and_a_default_target_when_the_default_is_in_effect() {
        let harness = harness(HarnessOptions::default());

        assert_eq!(
            harness.service.start(Some(default_root())),
            Err(ModStorageMigrationTaskError::Directory(
                ModStorageDirectoryError::OverlapsCurrentRoot
            ))
        );
        assert_eq!(
            harness.service.start(None),
            Err(ModStorageMigrationTaskError::Directory(
                ModStorageDirectoryError::OverlapsCurrentRoot
            ))
        );
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
        assert!(harness
            .inspector
            .claimed
            .lock()
            .expect("claimed")
            .is_empty());
    }

    #[test]
    fn start_back_to_the_default_root_skips_marker_handling_but_still_checks_nesting() {
        let to_default = harness(HarnessOptions {
            effective_root: custom_root(),
            configured: Some(custom_root()),
            target_root: default_root(),
            ..HarnessOptions::default()
        });

        let launch = to_default
            .service
            .start(None)
            .expect("migration to default");

        assert_eq!(launch.target_root, default_root());
        assert_eq!(launch.configured_target, None);
        assert!(to_default
            .inspector
            .inspected
            .lock()
            .expect("inspected")
            .is_empty());
        assert!(to_default
            .inspector
            .claimed
            .lock()
            .expect("claimed")
            .is_empty());

        let nested = harness(HarnessOptions {
            effective_root: custom_root(),
            configured: Some(custom_root()),
            target_root: default_root(),
            inspector: FakeInspector {
                overlap_pairs: vec![(default_root(), custom_root())],
                ..FakeInspector::default()
            },
            ..HarnessOptions::default()
        });
        assert_eq!(
            nested.service.start(None),
            Err(ModStorageMigrationTaskError::Directory(
                ModStorageDirectoryError::OverlapsCurrentRoot
            ))
        );
    }

    #[test]
    fn start_does_not_freeze_when_claiming_fails() {
        let harness = harness(HarnessOptions {
            inspector: FakeInspector {
                claim_error: Some(ModStorageDirectoryError::NotWritable),
                ..FakeInspector::default()
            },
            ..HarnessOptions::default()
        });

        let error = harness
            .service
            .start(Some(custom_root()))
            .expect_err("claim failure");

        assert_eq!(error.code(), "mod_storage_dir_not_writable");
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
    }

    #[test]
    fn run_copies_verifies_every_package_then_switches_and_freezes_until_restart() {
        let harness = harness(HarnessOptions::default());
        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");
        assert_eq!(
            queued_mod_storage_migration_event(&launch).phase,
            MOD_STORAGE_MIGRATION_QUEUED_PHASE
        );

        let events = harness.service.run(launch.clone()).expect("runner");

        assert_eq!(
            phases(&events),
            vec![
                (
                    MOD_STORAGE_MIGRATION_COPYING_PHASE.to_owned(),
                    Some(0),
                    Some(2)
                ),
                (
                    MOD_STORAGE_MIGRATION_VERIFYING_PHASE.to_owned(),
                    Some(0),
                    Some(2)
                ),
                (
                    MOD_STORAGE_MIGRATION_COPYING_PHASE.to_owned(),
                    Some(1),
                    Some(2)
                ),
                (
                    MOD_STORAGE_MIGRATION_VERIFYING_PHASE.to_owned(),
                    Some(1),
                    Some(2)
                ),
                (
                    MOD_STORAGE_MIGRATION_SWITCHING_PHASE.to_owned(),
                    Some(2),
                    Some(2)
                ),
                (
                    MOD_STORAGE_MIGRATION_COMPLETED_PHASE.to_owned(),
                    Some(2),
                    Some(2)
                ),
            ]
        );
        assert!(events.iter().all(|event| event.error.is_none()));
        assert_eq!(
            events.last().expect("terminal").status,
            TaskStatus::Completed
        );
        assert_eq!(
            harness.migrator.calls(),
            vec![
                "list:source",
                "copy:mod-import-1-0",
                "verify:mod-import-1-0",
                "copy:mod-import-2-0",
                "verify:mod-import-2-0",
            ]
        );
        assert_eq!(
            harness.journal.saved_states(),
            vec![
                ModStorageMigrationState::Copying,
                ModStorageMigrationState::Switched
            ]
        );
        let journal = harness
            .journal
            .current()
            .expect("journal kept for startup cleanup");
        assert_eq!(journal.state, ModStorageMigrationState::Switched);
        assert_eq!(journal.source_root, default_root());
        assert_eq!(journal.target_root, custom_root());
        assert_eq!(journal.packages, packages());
        assert_eq!(journal.started_at_unix_millis, 4_200);
        assert_eq!(harness.journal.cleared(), 0);
        assert_eq!(stored_dir(&harness), Some(custom_root()));
        assert_eq!(
            harness.write_gate.freeze(),
            ModStorageWriteFreeze::RestartRequired
        );
        assert_eq!(
            harness.task_manager.task_status(&launch.task.task_id),
            Some(TaskStatus::Completed)
        );
    }

    #[test]
    fn run_back_to_the_default_root_clears_the_setting() {
        let harness = harness(HarnessOptions {
            effective_root: custom_root(),
            configured: Some(custom_root()),
            target_root: default_root(),
            ..HarnessOptions::default()
        });
        let launch = harness.service.start(None).expect("migration starts");

        let events = harness.service.run(launch).expect("runner");

        assert_eq!(
            events.last().expect("terminal").phase,
            MOD_STORAGE_MIGRATION_COMPLETED_PHASE
        );
        assert_eq!(stored_dir(&harness), None);
        assert_eq!(
            harness.journal.current().expect("journal").target_root,
            default_root()
        );
    }

    #[test]
    fn run_rolls_back_every_copy_when_a_package_fails_to_copy() {
        let harness = harness(HarnessOptions {
            migrator: FakeMigrator {
                packages: packages(),
                fail_copy_on: Some("mod-import-2-0".to_owned()),
                ..FakeMigrator::default()
            },
            ..HarnessOptions::default()
        });
        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");

        let events = harness.service.run(launch.clone()).expect("runner");

        let terminal = events.last().expect("terminal");
        assert_eq!(terminal.phase, MOD_STORAGE_MIGRATION_FAILED_PHASE);
        assert_eq!(terminal.status, TaskStatus::Failed);
        assert_eq!(
            terminal.error.as_deref(),
            Some("mod_storage_migration_copy_failed")
        );
        assert_eq!(
            harness.migrator.calls(),
            vec![
                "list:source",
                "copy:mod-import-1-0",
                "verify:mod-import-1-0",
                "copy:mod-import-2-0",
                "remove:target:mod-import-1-0",
                "remove:target:mod-import-2-0",
            ],
            "the partial second copy is removed too; the source is never touched"
        );
        assert_eq!(
            harness.journal.saved_states(),
            vec![ModStorageMigrationState::Copying]
        );
        assert_eq!(harness.journal.cleared(), 1);
        assert_eq!(harness.journal.current(), None);
        assert_eq!(stored_dir(&harness), None);
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
        assert_eq!(
            harness.task_manager.task_status(&launch.task.task_id),
            Some(TaskStatus::Failed)
        );
    }

    #[test]
    fn run_rolls_back_when_verification_finds_a_mismatch() {
        let harness = harness(HarnessOptions {
            migrator: FakeMigrator {
                packages: packages(),
                fail_verify_on: Some("mod-import-1-0".to_owned()),
                ..FakeMigrator::default()
            },
            ..HarnessOptions::default()
        });
        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");

        let events = harness.service.run(launch).expect("runner");

        assert_eq!(
            events.last().expect("terminal").error.as_deref(),
            Some("mod_storage_migration_verify_mismatch")
        );
        assert_eq!(
            harness.migrator.calls(),
            vec![
                "list:source",
                "copy:mod-import-1-0",
                "verify:mod-import-1-0",
                "remove:target:mod-import-1-0",
            ]
        );
        assert_eq!(stored_dir(&harness), None);
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
    }

    #[test]
    fn run_keeps_the_journal_when_rollback_cannot_remove_a_copy() {
        let harness = harness(HarnessOptions {
            migrator: FakeMigrator {
                packages: packages(),
                fail_copy_on: Some("mod-import-2-0".to_owned()),
                fail_remove_on: Some("mod-import-1-0".to_owned()),
                ..FakeMigrator::default()
            },
            ..HarnessOptions::default()
        });
        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");

        let events = harness.service.run(launch).expect("runner");

        assert_eq!(
            events.last().expect("terminal").error.as_deref(),
            Some("mod_storage_migration_copy_failed"),
            "the primary failure stays the reported code"
        );
        assert_eq!(harness.journal.cleared(), 0);
        assert_eq!(
            harness.journal.current().expect("journal kept").state,
            ModStorageMigrationState::Copying,
            "the next start retries the rollback"
        );
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
        assert_eq!(stored_dir(&harness), None);
    }

    #[test]
    fn run_rolls_back_when_the_settings_write_fails_after_all_packages_passed() {
        let harness = harness(HarnessOptions {
            settings: FakeSettings {
                fail_save: true,
                ..FakeSettings::default()
            },
            ..HarnessOptions::default()
        });
        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");

        let events = harness.service.run(launch.clone()).expect("runner");

        let terminal = events.last().expect("terminal");
        assert_eq!(terminal.status, TaskStatus::Failed);
        assert_eq!(
            terminal.error.as_deref(),
            Some("mod_storage_migration_settings_unavailable")
        );
        assert_eq!(
            harness.journal.saved_states(),
            vec![
                ModStorageMigrationState::Copying,
                ModStorageMigrationState::Switched
            ]
        );
        assert_eq!(
            harness.journal.current(),
            None,
            "switched journal rolled back"
        );
        assert_eq!(
            harness
                .migrator
                .calls()
                .iter()
                .filter(|call| call.starts_with("remove:target:"))
                .count(),
            2
        );
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
        assert_eq!(
            harness.task_manager.task_status(&launch.task.task_id),
            Some(TaskStatus::Failed)
        );
    }

    #[test]
    fn run_fails_without_copying_when_the_journal_cannot_be_written() {
        let harness = harness(HarnessOptions {
            journal: FakeJournal {
                fail_save_in_state: Some(ModStorageMigrationState::Copying),
                ..FakeJournal::default()
            },
            ..HarnessOptions::default()
        });
        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");

        let events = harness.service.run(launch).expect("runner");

        assert_eq!(
            events.last().expect("terminal").error.as_deref(),
            Some("mod_storage_migration_journal_unavailable")
        );
        assert_eq!(harness.migrator.calls(), vec!["list:source"]);
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
    }

    #[test]
    fn cancelling_between_packages_rolls_back_and_reopens_the_gate() {
        let harness = harness(HarnessOptions::default());
        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");
        let task_manager = Arc::clone(&harness.task_manager);
        let task_id = launch.task.task_id.clone();
        *harness.migrator.hook.lock().expect("hook") = Some(Box::new(move |label: &str| {
            if label == "verify:mod-import-1-0" {
                task_manager.cancel_task(&task_id).expect("cancel");
            }
        }));

        let events = harness.service.run(launch.clone()).expect("runner");

        let terminal = events.last().expect("terminal");
        assert_eq!(terminal.phase, MOD_STORAGE_MIGRATION_CANCELLED_PHASE);
        assert_eq!(terminal.status, TaskStatus::Cancelled);
        assert_eq!(terminal.error, None);
        assert_eq!(
            harness.migrator.calls(),
            vec![
                "list:source",
                "copy:mod-import-1-0",
                "verify:mod-import-1-0",
                "remove:target:mod-import-1-0",
            ]
        );
        assert_eq!(harness.journal.current(), None);
        assert_eq!(stored_dir(&harness), None);
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
        assert_eq!(
            harness.task_manager.task_status(&launch.task.task_id),
            Some(TaskStatus::Cancelled)
        );
    }

    #[test]
    fn a_cancel_request_during_the_switch_is_deferred_and_the_switch_completes() {
        let harness = harness(HarnessOptions::default());
        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");
        let task_manager = Arc::clone(&harness.task_manager);
        let task_id = launch.task.task_id.clone();
        let cancel_outcome = Arc::new(Mutex::new(None));
        let recorded = Arc::clone(&cancel_outcome);
        *harness.journal.hook.lock().expect("hook") = Some(Box::new(move |label: &str| {
            if label == "save:Switched" {
                *recorded.lock().expect("outcome") =
                    Some(task_manager.cancel_task(&task_id).map(|_| ()));
            }
        }));

        let events = harness.service.run(launch.clone()).expect("runner");

        assert!(matches!(
            cancel_outcome.lock().expect("outcome").clone(),
            Some(Err(crate::TaskManagerError::TaskCannotBeCancelled { .. }))
        ));
        assert_eq!(
            events.last().expect("terminal").phase,
            MOD_STORAGE_MIGRATION_COMPLETED_PHASE
        );
        assert_eq!(stored_dir(&harness), Some(custom_root()));
        assert_eq!(
            harness.write_gate.freeze(),
            ModStorageWriteFreeze::RestartRequired
        );
        assert_eq!(
            harness.task_manager.task_status(&launch.task.task_id),
            Some(TaskStatus::Completed)
        );
    }

    #[test]
    fn a_launch_cancelled_before_running_emits_only_the_cancelled_event() {
        let harness = harness(HarnessOptions::default());
        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");
        harness
            .task_manager
            .cancel_task(&launch.task.task_id)
            .expect("cancel queued task");

        let events = harness.service.run(launch).expect("runner");

        assert_eq!(
            phases(&events),
            vec![(MOD_STORAGE_MIGRATION_CANCELLED_PHASE.to_owned(), None, None)]
        );
        assert!(harness.migrator.calls().is_empty());
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
    }

    #[test]
    fn abort_queued_fails_the_task_and_reopens_the_gate() {
        let harness = harness(HarnessOptions::default());
        let launch = harness
            .service
            .start(Some(custom_root()))
            .expect("migration starts");

        harness
            .service
            .abort_queued(&launch)
            .expect("abort queued launch");

        assert_eq!(
            harness.task_manager.task_status(&launch.task.task_id),
            Some(TaskStatus::Failed)
        );
        assert_eq!(harness.write_gate.freeze(), ModStorageWriteFreeze::None);
        assert_eq!(harness.service.abort_queued(&launch), Ok(()));
    }

    #[test]
    fn task_error_codes_are_stable() {
        assert_eq!(
            ModStorageMigrationTaskError::TaskUnavailable.code(),
            "mod_storage_migration_task_unavailable"
        );
        assert_eq!(
            ModStorageMigrationTaskError::GameConfigUnavailable.code(),
            "game_config_unavailable"
        );
        assert_eq!(
            ModStorageMigrationTaskError::Gate(ModStorageWriteGateError::MigrationInProgress)
                .code(),
            "mod_storage_migration_in_progress"
        );
        assert_eq!(
            ModStorageMigrationFailure::Migration(ModStorageMigrationError::Cancelled).code(),
            "mod_storage_migration_cancelled"
        );
    }

    fn pending_journal(
        state: ModStorageMigrationState,
        source_root: PathBuf,
    ) -> ModStorageMigrationJournal {
        ModStorageMigrationJournal {
            version: MOD_STORAGE_MIGRATION_JOURNAL_VERSION,
            state,
            source_root,
            target_root: custom_root(),
            packages: packages(),
            started_at_unix_millis: 1,
        }
    }

    fn existing_source_root() -> PathBuf {
        std::env::temp_dir()
    }

    fn settlement_migrator(source_root: PathBuf, migrator: FakeMigrator) -> FakeMigrator {
        FakeMigrator {
            source_root,
            target_root: custom_root(),
            ..migrator
        }
    }

    #[test]
    fn settlement_without_a_journal_does_nothing() {
        let journal = FakeJournal::default();
        let migrator = settlement_migrator(existing_source_root(), FakeMigrator::default());

        let settlement =
            settle_pending_mod_storage_migration(&journal, &migrator, Some(&custom_root()));

        assert_eq!(settlement, ModStorageMigrationSettlement::NoJournal);
        assert!(migrator.calls().is_empty());
    }

    #[test]
    fn settlement_cleans_the_source_once_the_switched_target_is_in_effect() {
        let source_root = existing_source_root();
        let journal = FakeJournal::with_pending(pending_journal(
            ModStorageMigrationState::Switched,
            source_root.clone(),
        ));
        let migrator = settlement_migrator(source_root, FakeMigrator::default());

        let settlement =
            settle_pending_mod_storage_migration(&journal, &migrator, Some(&custom_root()));

        assert_eq!(
            settlement,
            ModStorageMigrationSettlement::SourceCleaned { package_count: 2 }
        );
        assert_eq!(
            migrator.calls(),
            vec![
                "exists:target:mod-import-1-0",
                "exists:target:mod-import-2-0",
                "remove:source:mod-import-1-0",
                "remove:source:mod-import-2-0",
            ],
            "every target copy is confirmed before any source copy goes"
        );
        assert_eq!(journal.cleared(), 1);
        assert_eq!(journal.current(), None);
    }

    #[test]
    fn settlement_keeps_the_journal_when_a_target_package_is_missing() {
        let source_root = existing_source_root();
        let journal = FakeJournal::with_pending(pending_journal(
            ModStorageMigrationState::Switched,
            source_root.clone(),
        ));
        let migrator = settlement_migrator(
            source_root,
            FakeMigrator {
                missing_in_target: vec!["mod-import-2-0".to_owned()],
                ..FakeMigrator::default()
            },
        );

        let settlement =
            settle_pending_mod_storage_migration(&journal, &migrator, Some(&custom_root()));

        assert_eq!(
            settlement,
            ModStorageMigrationSettlement::CleanupBlocked(
                ModStorageMigrationError::TargetPackageMissing
            )
        );
        assert!(migrator
            .calls()
            .iter()
            .all(|call| call.starts_with("exists:")));
        assert_eq!(journal.cleared(), 0);
    }

    #[test]
    fn settlement_keeps_the_journal_while_the_source_root_is_unavailable() {
        let source_root =
            std::env::temp_dir().join(format!("hmm-missing-source-{}", std::process::id()));
        let journal = FakeJournal::with_pending(pending_journal(
            ModStorageMigrationState::Switched,
            source_root.clone(),
        ));
        let migrator = settlement_migrator(source_root, FakeMigrator::default());

        let settlement =
            settle_pending_mod_storage_migration(&journal, &migrator, Some(&custom_root()));

        assert_eq!(
            settlement,
            ModStorageMigrationSettlement::CleanupBlocked(
                ModStorageMigrationError::SourceUnavailable
            )
        );
        assert!(migrator.calls().is_empty());
        assert_eq!(journal.cleared(), 0);
    }

    #[test]
    fn settlement_rolls_back_a_copying_journal_from_the_target() {
        let source_root = existing_source_root();
        let journal = FakeJournal::with_pending(pending_journal(
            ModStorageMigrationState::Copying,
            source_root.clone(),
        ));
        let migrator = settlement_migrator(source_root.clone(), FakeMigrator::default());

        let settlement =
            settle_pending_mod_storage_migration(&journal, &migrator, Some(&source_root));

        assert_eq!(
            settlement,
            ModStorageMigrationSettlement::RolledBack { package_count: 2 }
        );
        assert_eq!(
            migrator.calls(),
            vec![
                "remove:target:mod-import-1-0",
                "remove:target:mod-import-2-0"
            ]
        );
        assert_eq!(journal.current(), None);
    }

    #[test]
    fn settlement_rolls_back_a_switched_journal_whose_setting_never_switched() {
        let source_root = existing_source_root();
        let journal = FakeJournal::with_pending(pending_journal(
            ModStorageMigrationState::Switched,
            source_root.clone(),
        ));
        let migrator = settlement_migrator(source_root.clone(), FakeMigrator::default());

        let settlement =
            settle_pending_mod_storage_migration(&journal, &migrator, Some(&source_root));

        assert_eq!(
            settlement,
            ModStorageMigrationSettlement::RolledBack { package_count: 2 }
        );
        assert!(migrator
            .calls()
            .iter()
            .all(|call| call.starts_with("remove:target:")));
    }

    #[test]
    fn settlement_reports_a_blocked_rollback_and_keeps_the_journal() {
        let source_root = existing_source_root();
        let journal = FakeJournal::with_pending(pending_journal(
            ModStorageMigrationState::Copying,
            source_root.clone(),
        ));
        let migrator = settlement_migrator(
            source_root.clone(),
            FakeMigrator {
                fail_remove_on: Some("mod-import-1-0".to_owned()),
                ..FakeMigrator::default()
            },
        );

        let settlement =
            settle_pending_mod_storage_migration(&journal, &migrator, Some(&source_root));

        assert_eq!(
            settlement,
            ModStorageMigrationSettlement::RollbackBlocked(
                ModStorageMigrationError::PackageUnreadable
            )
        );
        assert_eq!(journal.cleared(), 0);
    }

    #[test]
    fn settlement_defers_when_no_action_is_safe() {
        let source_root = existing_source_root();
        let journal = FakeJournal::with_pending(pending_journal(
            ModStorageMigrationState::Copying,
            source_root.clone(),
        ));
        let migrator = settlement_migrator(source_root, FakeMigrator::default());

        assert_eq!(
            settle_pending_mod_storage_migration(&journal, &migrator, None),
            ModStorageMigrationSettlement::Deferred,
            "settings unreadable: the effective root is unknown"
        );
        assert_eq!(
            settle_pending_mod_storage_migration(&journal, &migrator, Some(&custom_root())),
            ModStorageMigrationSettlement::Deferred,
            "a copying journal whose target is already in effect must not delete live packages"
        );
        assert!(migrator.calls().is_empty());
        assert_eq!(journal.cleared(), 0);

        let unreadable = FakeJournal {
            fail_load: true,
            ..FakeJournal::default()
        };
        assert_eq!(
            settle_pending_mod_storage_migration(&unreadable, &migrator, Some(&other_root())),
            ModStorageMigrationSettlement::JournalUnreadable(
                ModStorageMigrationError::JournalUnavailable
            )
        );
    }
}
