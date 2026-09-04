//! Mod storage directory settings (#275, slice 1).
//!
//! The effective storage root is fixed when the runtime composes (see `hmm-runtime`); this
//! service only reads/writes the *setting* and validates candidate directories. Changing the
//! setting therefore takes effect after a restart. While the library holds packages a change
//! must go through migration (`mod_storage_migration`), because switching the root underneath
//! existing packages would leave them unreachable — the service refuses with
//! `MigrationRequired`. A successful `set` freezes sandbox writes until restart through the
//! shared [`ModStorageWriteGate`], for the same reason.

use crate::{
    AppSettingsService, ModStorageWriteFreeze, ModStorageWriteGate, ModStorageWriteGateError,
};
use hmm_core::GameId;
use hmm_ports::{
    GameConfigRepository, ModImportResultRepository, ModStorageDirectoryError,
    ModStorageDirectoryInspectionRequest, ModStorageDirectoryInspector,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModStorageSettingsError {
    #[error("{0}")]
    Directory(ModStorageDirectoryError),
    #[error("the library holds packages; changing the storage directory requires migration")]
    MigrationRequired,
    #[error("{0}")]
    WriteFrozen(ModStorageWriteGateError),
    #[error("app settings unavailable")]
    SettingsUnavailable,
    #[error("mod library unavailable")]
    LibraryUnavailable,
    #[error("game configuration unavailable")]
    GameConfigUnavailable,
}

impl ModStorageSettingsError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Directory(error) => error.code(),
            Self::MigrationRequired => "mod_storage_migration_required",
            Self::WriteFrozen(error) => error.code(),
            Self::SettingsUnavailable => "app_settings_unavailable",
            Self::LibraryUnavailable => "mod_library_unavailable",
            Self::GameConfigUnavailable => "game_config_unavailable",
        }
    }
}

impl From<ModStorageWriteGateError> for ModStorageSettingsError {
    fn from(error: ModStorageWriteGateError) -> Self {
        Self::WriteFrozen(error)
    }
}

impl From<ModStorageDirectoryError> for ModStorageSettingsError {
    fn from(error: ModStorageDirectoryError) -> Self {
        Self::Directory(error)
    }
}

/// Facts the settings page needs. Paths are the user's own directories (default root included),
/// never package sandboxes, so returning them is in line with how the game directory is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModStorageSettingsSnapshot {
    /// Root in effect for this process.
    pub effective_root: PathBuf,
    pub default_root: PathBuf,
    /// Persisted setting right now (may differ from `effective_root` until restart).
    pub configured: Option<PathBuf>,
    /// No revision in the catalog and no entry below `<effective root>/sandboxes`.
    pub library_empty: bool,
    /// The persisted setting differs from what this process started with.
    pub restart_required: bool,
    /// Whether sandbox writes (import, external import, delete) are refused right now.
    pub writes_frozen: ModStorageWriteFreeze,
}

/// Result of a read-only directory check. Failures are facts to display, not errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModStorageDirectoryValidation {
    pub ok: bool,
    pub code: Option<&'static str>,
    pub exists: bool,
    pub claimed: bool,
}

pub struct ModStorageSettingsService {
    app_settings: Arc<AppSettingsService>,
    inspector: Arc<dyn ModStorageDirectoryInspector>,
    game_config: Arc<dyn GameConfigRepository>,
    game_ids: Vec<GameId>,
    catalog: Arc<dyn ModImportResultRepository>,
    write_gate: Arc<ModStorageWriteGate>,
    effective_root: PathBuf,
    default_root: PathBuf,
    startup_configured: Option<PathBuf>,
    update_lock: Mutex<()>,
}

pub struct ModStorageSettingsServiceDependencies {
    /// Single writer for `settings.json`; see `AppSettingsService::update_mod_storage_dir`.
    pub app_settings: Arc<AppSettingsService>,
    pub inspector: Arc<dyn ModStorageDirectoryInspector>,
    pub game_config: Arc<dyn GameConfigRepository>,
    pub game_ids: Vec<GameId>,
    pub catalog: Arc<dyn ModImportResultRepository>,
    /// Shared with the sandbox writers and the migration task.
    pub write_gate: Arc<ModStorageWriteGate>,
    /// Root the running process resolved at startup.
    pub effective_root: PathBuf,
    pub default_root: PathBuf,
    /// Setting value the running process resolved at startup (raw, even if degraded).
    pub startup_configured: Option<PathBuf>,
}

impl ModStorageSettingsService {
    pub fn new(dependencies: ModStorageSettingsServiceDependencies) -> Self {
        Self {
            app_settings: dependencies.app_settings,
            inspector: dependencies.inspector,
            game_config: dependencies.game_config,
            game_ids: dependencies.game_ids,
            catalog: dependencies.catalog,
            write_gate: dependencies.write_gate,
            effective_root: dependencies.effective_root,
            default_root: dependencies.default_root,
            startup_configured: dependencies.startup_configured,
            update_lock: Mutex::new(()),
        }
    }

    pub fn effective_root(&self) -> &Path {
        &self.effective_root
    }

    pub fn get(&self) -> Result<ModStorageSettingsSnapshot, ModStorageSettingsError> {
        let configured = self.persisted_configured()?;
        let library_empty = self.library_is_empty()?;
        Ok(self.snapshot(configured, library_empty))
    }

    /// Read-only check of a candidate directory against every rule `set` will enforce, minus the
    /// library-empty rule (that one is reported through `library_empty` in the snapshot).
    pub fn validate(
        &self,
        directory: &Path,
    ) -> Result<ModStorageDirectoryValidation, ModStorageSettingsError> {
        let game_roots = self.game_roots()?;
        Ok(
            match self
                .inspector
                .inspect(ModStorageDirectoryInspectionRequest {
                    path: directory,
                    exclusive_roots: &game_roots,
                    current_root: Some(&self.effective_root),
                }) {
                Ok(inspection) => ModStorageDirectoryValidation {
                    ok: true,
                    code: None,
                    exists: inspection.exists,
                    claimed: inspection.claimed,
                },
                Err(error) => ModStorageDirectoryValidation {
                    ok: false,
                    code: Some(error.code()),
                    exists: false,
                    claimed: false,
                },
            },
        )
    }

    /// Persists a new storage root (`None` = back to the default). Only allowed while the
    /// library is empty; otherwise the caller must run a migration first. Refused while a
    /// migration runs or another switch already waits for the restart.
    pub fn set(
        &self,
        directory: Option<PathBuf>,
    ) -> Result<ModStorageSettingsSnapshot, ModStorageSettingsError> {
        let _guard = self
            .update_lock
            .lock()
            .map_err(|_| ModStorageSettingsError::SettingsUnavailable)?;
        let current = self
            .app_settings
            .get_settings()
            .map_err(|_| ModStorageSettingsError::SettingsUnavailable)?;
        if current.mod_storage_dir == directory {
            let library_empty = self.library_is_empty()?;
            return Ok(self.snapshot(directory, library_empty));
        }
        self.write_gate.ensure_open()?;
        if !self.library_is_empty()? {
            return Err(ModStorageSettingsError::MigrationRequired);
        }
        if let Some(directory) = &directory {
            let game_roots = self.game_roots()?;
            self.inspector
                .inspect(ModStorageDirectoryInspectionRequest {
                    path: directory,
                    exclusive_roots: &game_roots,
                    current_root: Some(&self.effective_root),
                })?;
            self.inspector.claim(directory)?;
        }
        self.write_gate
            .admit_root_switch(|| self.app_settings.update_mod_storage_dir(directory.clone()))?
            .map_err(|_| ModStorageSettingsError::SettingsUnavailable)?;
        Ok(self.snapshot(directory, true))
    }

    fn snapshot(
        &self,
        configured: Option<PathBuf>,
        library_empty: bool,
    ) -> ModStorageSettingsSnapshot {
        ModStorageSettingsSnapshot {
            effective_root: self.effective_root.clone(),
            default_root: self.default_root.clone(),
            restart_required: configured != self.startup_configured,
            configured,
            library_empty,
            writes_frozen: self.write_gate.freeze(),
        }
    }

    fn persisted_configured(&self) -> Result<Option<PathBuf>, ModStorageSettingsError> {
        self.app_settings
            .get_settings()
            .map(|settings| settings.mod_storage_dir)
            .map_err(|_| ModStorageSettingsError::SettingsUnavailable)
    }

    fn library_is_empty(&self) -> Result<bool, ModStorageSettingsError> {
        let snapshot = self
            .catalog
            .catalog_snapshot()
            .map_err(|_| ModStorageSettingsError::LibraryUnavailable)?;
        if !snapshot.revisions.is_empty() {
            return Ok(false);
        }
        Ok(!self
            .inspector
            .sandbox_directory_has_entries(&self.effective_root)?)
    }

    fn game_roots(&self) -> Result<Vec<PathBuf>, ModStorageSettingsError> {
        let mut roots = Vec::new();
        for game_id in &self.game_ids {
            if let Some(instance) = self
                .game_config
                .load_game_instance(game_id)
                .map_err(|_| ModStorageSettingsError::GameConfigUnavailable)?
            {
                roots.push(instance.root_dir);
            }
        }
        Ok(roots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameDirectoryStatus, GameInstance};
    use hmm_ports::{
        AppSettings, AppSettingsRepository, AppSettingsRepositoryError,
        AppSettingsRepositoryResult, GameConfigRepositoryResult, ModImportCatalogSnapshot,
        ModStorageDirectoryInspection, StoredImportPreviewImage, StoredLogicalMod,
        StoredModImportAnalysis, StoredModOriginProvenance, StoredModRevision,
    };
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeSettings {
        stored: Mutex<AppSettings>,
        fail_load: bool,
    }

    impl AppSettingsRepository for FakeSettings {
        fn load_settings(&self) -> AppSettingsRepositoryResult<AppSettings> {
            if self.fail_load {
                return Err(AppSettingsRepositoryError::StorageCorrupted);
            }
            Ok(self.stored.lock().expect("settings lock").clone())
        }

        fn save_settings(&self, settings: &AppSettings) -> AppSettingsRepositoryResult<()> {
            *self.stored.lock().expect("settings lock") = settings.clone();
            Ok(())
        }
    }

    /// (candidate, exclusive game roots, current root) as passed to `inspect`.
    type InspectionRecord = (PathBuf, Vec<PathBuf>, Option<PathBuf>);

    struct FakeInspector {
        inspect_result: Mutex<Result<ModStorageDirectoryInspection, ModStorageDirectoryError>>,
        claim_result: Mutex<Result<(), ModStorageDirectoryError>>,
        sandbox_has_entries: bool,
        overlap_pairs: Vec<(PathBuf, PathBuf)>,
        inspected: Mutex<Vec<InspectionRecord>>,
        claimed: Mutex<Vec<PathBuf>>,
    }

    impl Default for FakeInspector {
        fn default() -> Self {
            Self {
                inspect_result: Mutex::new(Ok(ModStorageDirectoryInspection {
                    exists: false,
                    claimed: false,
                })),
                claim_result: Mutex::new(Ok(())),
                sandbox_has_entries: false,
                overlap_pairs: Vec::new(),
                inspected: Mutex::new(Vec::new()),
                claimed: Mutex::new(Vec::new()),
            }
        }
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
            for root in request.exclusive_roots {
                if self.directories_overlap(request.path, root) {
                    return Err(ModStorageDirectoryError::OverlapsGameRoot);
                }
            }
            if let Some(current_root) = request.current_root {
                if self.directories_overlap(request.path, current_root) {
                    return Err(ModStorageDirectoryError::OverlapsCurrentRoot);
                }
            }
            *self.inspect_result.lock().expect("inspect lock")
        }

        fn claim(&self, path: &Path) -> Result<(), ModStorageDirectoryError> {
            self.claimed
                .lock()
                .expect("claimed lock")
                .push(path.to_path_buf());
            *self.claim_result.lock().expect("claim lock")
        }

        fn verify_claimed(&self, _path: &Path) -> Result<(), ModStorageDirectoryError> {
            Ok(())
        }

        fn sandbox_directory_has_entries(
            &self,
            _storage_root: &Path,
        ) -> Result<bool, ModStorageDirectoryError> {
            Ok(self.sandbox_has_entries)
        }

        fn directories_overlap(&self, left: &Path, right: &Path) -> bool {
            self.overlap_pairs
                .iter()
                .any(|(a, b)| (a == left && b == right) || (a == right && b == left))
        }
    }

    struct FakeGameConfig {
        instances: Vec<GameInstance>,
    }

    impl GameConfigRepository for FakeGameConfig {
        fn load_game_instance(
            &self,
            game_id: &GameId,
        ) -> GameConfigRepositoryResult<Option<GameInstance>> {
            Ok(self
                .instances
                .iter()
                .find(|instance| &instance.game_id == game_id)
                .cloned())
        }

        fn save_game_instance(&self, _instance: &GameInstance) -> GameConfigRepositoryResult<()> {
            Ok(())
        }
    }

    struct FakeCatalog {
        revisions: Vec<StoredModRevision>,
    }

    impl ModImportResultRepository for FakeCatalog {
        fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
            Ok(())
        }

        fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
            Ok(Vec::new())
        }

        fn get_analysis(&self, _mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
            Ok(None)
        }

        fn remove_analysis(&self, _mod_id: &str) -> anyhow::Result<()> {
            Ok(())
        }

        fn catalog_snapshot(&self) -> anyhow::Result<ModImportCatalogSnapshot> {
            Ok(ModImportCatalogSnapshot {
                logical_mods: self
                    .revisions
                    .iter()
                    .map(|revision| StoredLogicalMod {
                        mod_id: revision.mod_id.clone(),
                        origin_revision_id: revision.revision_id.clone(),
                        display_revision_id: revision.revision_id.clone(),
                        origin_provenance: StoredModOriginProvenance::Imported,
                    })
                    .collect(),
                revisions: self.revisions.clone(),
            })
        }
    }

    fn game_root() -> PathBuf {
        PathBuf::from(if cfg!(windows) {
            "D:\\Games\\MHW"
        } else {
            "/games/mhw"
        })
    }

    fn candidate() -> PathBuf {
        PathBuf::from(if cfg!(windows) {
            "E:\\HMMMods"
        } else {
            "/srv/hmm-mods"
        })
    }

    fn default_root() -> PathBuf {
        PathBuf::from(if cfg!(windows) {
            "C:\\app-data\\mod-import"
        } else {
            "/app-data/mod-import"
        })
    }

    struct Harness {
        settings: Arc<FakeSettings>,
        inspector: Arc<FakeInspector>,
        write_gate: Arc<ModStorageWriteGate>,
        service: ModStorageSettingsService,
    }

    fn harness(
        settings: FakeSettings,
        inspector: FakeInspector,
        revisions: Vec<StoredModRevision>,
        startup_configured: Option<PathBuf>,
    ) -> Harness {
        let settings = Arc::new(settings);
        let inspector = Arc::new(inspector);
        let write_gate = Arc::new(ModStorageWriteGate::new());
        let service = ModStorageSettingsService::new(ModStorageSettingsServiceDependencies {
            app_settings: Arc::new(AppSettingsService::new(settings.clone())),
            inspector: inspector.clone(),
            write_gate: write_gate.clone(),
            game_config: Arc::new(FakeGameConfig {
                instances: vec![GameInstance {
                    id: "mhw-default".to_owned(),
                    game_id: GameId::mhw(),
                    display_name: "MHW".to_owned(),
                    root_dir: game_root(),
                    status: GameDirectoryStatus::Configured,
                    configured_at_unix_millis: 1,
                }],
            }),
            game_ids: vec![GameId::mhw()],
            catalog: Arc::new(FakeCatalog { revisions }),
            effective_root: startup_configured.clone().unwrap_or_else(default_root),
            default_root: default_root(),
            startup_configured,
        });
        Harness {
            settings,
            inspector,
            write_gate,
            service,
        }
    }

    fn revision() -> StoredModRevision {
        StoredModRevision {
            revision_id: hmm_core::ModRevisionId::new("mod-a"),
            mod_id: hmm_core::ModId::new("mod-a"),
            import_task_id: "task-a".to_owned(),
            package_id: "mod-a".to_owned(),
            display_name: "Mod A".to_owned(),
            metadata: Default::default(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: hmm_core::PreviewImageRejectionReason::Missing,
            },
        }
    }

    fn accepting_inspector() -> FakeInspector {
        FakeInspector {
            inspect_result: Mutex::new(Ok(ModStorageDirectoryInspection {
                exists: false,
                claimed: false,
            })),
            ..FakeInspector::default()
        }
    }

    #[test]
    fn get_reports_default_root_and_empty_library_without_restart() {
        let harness = harness(
            FakeSettings::default(),
            accepting_inspector(),
            Vec::new(),
            None,
        );

        let snapshot = harness.service.get().expect("snapshot");

        assert_eq!(
            snapshot,
            ModStorageSettingsSnapshot {
                effective_root: default_root(),
                default_root: default_root(),
                configured: None,
                library_empty: true,
                restart_required: false,
                writes_frozen: ModStorageWriteFreeze::None,
            }
        );
    }

    #[test]
    fn validate_passes_configured_game_roots_to_the_inspector_and_reports_verdicts() {
        let harness = harness(
            FakeSettings::default(),
            accepting_inspector(),
            Vec::new(),
            None,
        );

        let verdict = harness.service.validate(&candidate()).expect("validate");

        assert_eq!(
            verdict,
            ModStorageDirectoryValidation {
                ok: true,
                code: None,
                exists: false,
                claimed: false,
            }
        );
        assert_eq!(
            harness
                .inspector
                .inspected
                .lock()
                .expect("inspected")
                .as_slice(),
            [(candidate(), vec![game_root()], Some(default_root()))]
        );
    }

    #[test]
    fn validate_reports_a_candidate_overlapping_the_current_root() {
        let inspector = FakeInspector {
            overlap_pairs: vec![(candidate(), default_root())],
            ..accepting_inspector()
        };
        let harness = harness(FakeSettings::default(), inspector, Vec::new(), None);

        let verdict = harness.service.validate(&candidate()).expect("validate");

        assert_eq!(verdict.code, Some("mod_storage_dir_overlaps_current_root"));
        assert_eq!(
            harness
                .service
                .set(Some(candidate()))
                .expect_err("set enforces the same rule"),
            ModStorageSettingsError::Directory(ModStorageDirectoryError::OverlapsCurrentRoot)
        );
    }

    #[test]
    fn validate_turns_inspector_errors_into_stable_codes_without_failing() {
        let inspector = FakeInspector {
            inspect_result: Mutex::new(Err(ModStorageDirectoryError::MarkerRequired)),
            ..FakeInspector::default()
        };
        let harness = harness(FakeSettings::default(), inspector, Vec::new(), None);

        let verdict = harness.service.validate(&candidate()).expect("validate");

        assert_eq!(
            verdict,
            ModStorageDirectoryValidation {
                ok: false,
                code: Some("mod_storage_dir_marker_required"),
                exists: false,
                claimed: false,
            }
        );
    }

    #[test]
    fn set_with_an_empty_library_claims_the_directory_and_persists_it() {
        let harness = harness(
            FakeSettings::default(),
            accepting_inspector(),
            Vec::new(),
            None,
        );

        let snapshot = harness
            .service
            .set(Some(candidate()))
            .expect("set storage directory");

        assert_eq!(
            harness
                .inspector
                .claimed
                .lock()
                .expect("claimed")
                .as_slice(),
            [candidate()]
        );
        assert_eq!(
            harness
                .settings
                .stored
                .lock()
                .expect("settings")
                .mod_storage_dir,
            Some(candidate())
        );
        assert_eq!(snapshot.configured, Some(candidate()));
        assert!(snapshot.restart_required);
        assert_eq!(
            snapshot.effective_root,
            default_root(),
            "the running process keeps its startup root until restart"
        );
        assert_eq!(
            snapshot.writes_frozen,
            ModStorageWriteFreeze::RestartRequired,
            "imports after the switch would land in the old root"
        );
        assert_eq!(
            harness
                .write_gate
                .ensure_open()
                .map_err(|error| error.code()),
            Err("mod_storage_restart_required")
        );
    }

    #[test]
    fn set_is_refused_while_a_migration_runs_and_after_a_switch_without_touching_the_directory() {
        let harness = harness(
            FakeSettings::default(),
            accepting_inspector(),
            Vec::new(),
            None,
        );
        harness
            .write_gate
            .begin_migration(|| Ok::<(), ModStorageWriteGateError>(()))
            .expect("migration admitted");

        let error = harness
            .service
            .set(Some(candidate()))
            .expect_err("frozen gate refuses the change");

        assert_eq!(error.code(), "mod_storage_migration_in_progress");
        assert!(harness
            .inspector
            .claimed
            .lock()
            .expect("claimed")
            .is_empty());
        assert_eq!(
            harness
                .settings
                .stored
                .lock()
                .expect("settings")
                .mod_storage_dir,
            None
        );

        harness.write_gate.end_migration(true);
        assert_eq!(
            harness
                .service
                .set(Some(candidate()))
                .expect_err("a switched root waits for the restart")
                .code(),
            "mod_storage_restart_required"
        );
        assert_eq!(
            harness.service.get().expect("snapshot").writes_frozen,
            ModStorageWriteFreeze::RestartRequired
        );
    }

    #[test]
    fn set_refuses_while_the_catalog_holds_revisions() {
        let harness = harness(
            FakeSettings::default(),
            accepting_inspector(),
            vec![revision()],
            None,
        );

        let error = harness
            .service
            .set(Some(candidate()))
            .expect_err("non-empty library requires migration");

        assert_eq!(error, ModStorageSettingsError::MigrationRequired);
        assert_eq!(error.code(), "mod_storage_migration_required");
        assert!(harness
            .inspector
            .claimed
            .lock()
            .expect("claimed")
            .is_empty());
        assert_eq!(
            harness
                .settings
                .stored
                .lock()
                .expect("settings")
                .mod_storage_dir,
            None
        );
    }

    #[test]
    fn set_refuses_while_the_sandbox_directory_holds_entries_even_with_an_empty_catalog() {
        let inspector = FakeInspector {
            sandbox_has_entries: true,
            ..accepting_inspector()
        };
        let harness = harness(FakeSettings::default(), inspector, Vec::new(), None);

        let error = harness
            .service
            .set(Some(candidate()))
            .expect_err("orphan or in-flight sandboxes require migration");

        assert_eq!(error, ModStorageSettingsError::MigrationRequired);
        assert!(!harness.service.get().expect("snapshot").library_empty);
    }

    #[test]
    fn set_rejects_a_directory_overlapping_a_game_root_before_claiming() {
        let inspector = FakeInspector {
            overlap_pairs: vec![(candidate(), game_root())],
            ..accepting_inspector()
        };
        let harness = harness(FakeSettings::default(), inspector, Vec::new(), None);

        let error = harness
            .service
            .set(Some(candidate()))
            .expect_err("overlap must be rejected");

        assert_eq!(
            error,
            ModStorageSettingsError::Directory(ModStorageDirectoryError::OverlapsGameRoot)
        );
        assert!(harness
            .inspector
            .claimed
            .lock()
            .expect("claimed")
            .is_empty());
    }

    #[test]
    fn set_does_not_persist_when_claiming_fails() {
        let inspector = FakeInspector {
            claim_result: Mutex::new(Err(ModStorageDirectoryError::NotWritable)),
            ..accepting_inspector()
        };
        let harness = harness(FakeSettings::default(), inspector, Vec::new(), None);

        let error = harness
            .service
            .set(Some(candidate()))
            .expect_err("claim failure must fail the whole change");

        assert_eq!(error.code(), "mod_storage_dir_not_writable");
        assert_eq!(
            harness
                .settings
                .stored
                .lock()
                .expect("settings")
                .mod_storage_dir,
            None
        );
    }

    #[test]
    fn set_to_none_restores_the_default_and_reports_restart_when_it_changes() {
        let settings = FakeSettings {
            stored: Mutex::new(AppSettings {
                mod_storage_dir: Some(candidate()),
                ..AppSettings::default()
            }),
            fail_load: false,
        };
        let harness = harness(
            settings,
            accepting_inspector(),
            Vec::new(),
            Some(candidate()),
        );

        let snapshot = harness.service.set(None).expect("reset to default");

        assert_eq!(snapshot.configured, None);
        assert!(snapshot.restart_required);
        assert_eq!(snapshot.effective_root, candidate());
        assert!(harness
            .inspector
            .claimed
            .lock()
            .expect("claimed")
            .is_empty());
    }

    #[test]
    fn set_to_the_same_value_is_a_no_op_even_with_a_full_library() {
        let settings = FakeSettings {
            stored: Mutex::new(AppSettings {
                mod_storage_dir: Some(candidate()),
                ..AppSettings::default()
            }),
            fail_load: false,
        };
        let harness = harness(
            settings,
            accepting_inspector(),
            vec![revision()],
            Some(candidate()),
        );

        let snapshot = harness
            .service
            .set(Some(candidate()))
            .expect("re-setting the current value is allowed");

        assert!(!snapshot.restart_required);
        assert!(!snapshot.library_empty);
        assert!(harness
            .inspector
            .claimed
            .lock()
            .expect("claimed")
            .is_empty());
    }

    #[test]
    fn unreadable_settings_surface_as_settings_unavailable() {
        let settings = FakeSettings {
            fail_load: true,
            ..FakeSettings::default()
        };
        let harness = harness(settings, accepting_inspector(), Vec::new(), None);

        assert_eq!(
            harness.service.get().expect_err("unreadable settings"),
            ModStorageSettingsError::SettingsUnavailable
        );
        assert_eq!(
            harness
                .service
                .set(Some(candidate()))
                .expect_err("unreadable settings")
                .code(),
            "app_settings_unavailable"
        );
    }

    #[test]
    fn error_codes_are_stable() {
        let codes: BTreeMap<&str, &str> = [
            (
                "migration",
                ModStorageSettingsError::MigrationRequired.code(),
            ),
            (
                "settings",
                ModStorageSettingsError::SettingsUnavailable.code(),
            ),
            (
                "library",
                ModStorageSettingsError::LibraryUnavailable.code(),
            ),
            (
                "game",
                ModStorageSettingsError::GameConfigUnavailable.code(),
            ),
            (
                "frozen",
                ModStorageSettingsError::WriteFrozen(ModStorageWriteGateError::RestartRequired)
                    .code(),
            ),
        ]
        .into_iter()
        .collect();
        assert_eq!(codes["migration"], "mod_storage_migration_required");
        assert_eq!(codes["settings"], "app_settings_unavailable");
        assert_eq!(codes["library"], "mod_library_unavailable");
        assert_eq!(codes["game"], "game_config_unavailable");
        assert_eq!(codes["frozen"], "mod_storage_restart_required");
    }
}
