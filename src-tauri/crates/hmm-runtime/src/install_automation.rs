use crate::game_automation::{is_canonically_within, is_safe_absolute_path};
use crate::RuntimeEnvironment;
use hmm_app::{
    is_identity_replacement_binding, BatchReinstallItemFactsReader, BatchReinstallItemFactsRequest,
    BatchReinstallPlanFactsProvider, BatchUninstallPlanFactsProvider,
    BuildImportedModInstallPlanRequest, GamePrerequisiteDecision, GamePrerequisiteDecisionProvider,
    GameSetupService, ImportedModInstallPreflightService, InitialRetargetInstallStatusError,
    InitialRetargetInstallStatusReader, InstallManifestQueryRequest, InstallManifestQueryService,
    InstallManifestStatus, InstallPlanningError, InstallPlanningService,
    InstallRecoveryActionAvailability, InstallRecoveryActionBlockReason, InstallRecoveryActionKind,
    InstallRecoveryActionPreview, InstallRecoveryActionPreviewRequest,
    InstallRecoveryActionPreviewService, InstallRecoveryIssue, InstallRecoveryScanRequest,
    InstallRecoveryScanService, InstallRecoveryStatus, InstallRecoverySummary,
    InstalledReplacementReinstallResolution, PreviewRetargetReinstallRequest,
    ReinstallBlockingReason, ReinstallBlockingReasonSummary, ReinstallCandidateSourceReader,
    ReinstallPlanPreview, ReinstallPreparation, ReinstallPreviewBatchItemFactsReader,
    ReinstallPreviewError, ReinstallPreviewRequest, ReinstallPreviewService,
    ReinstallPreviewStatus, ReinstallRevisionSummary, ReinstallTargetCounts,
    ReplacementWorkflowService,
};
use hmm_core::{
    BatchItemFacts, BatchPlanFacts, FileLayer, GameId, GameInstance, InstallManifest,
    InstallRecoveryRecord, InstallTargetPath, ModId, ModRevisionId, NormalizedBatchPlanRequest,
    PackageFileId, ProfileId, ReinstallBatchItemInput, ReinstallRecoveryTransaction,
    ReplacementBindingSnapshot, ReplacementTargetId,
};
use hmm_games_mhw::{MhwReplacementAdapter, MhwReplacementCatalog, MonsterHunterWorldAdapter};
use hmm_infra::{
    FileSystemInstallBackupStore, FileSystemInstallGameFileSystem,
    FileSystemInstallSourceFileReader, JsonGameConfigRepository, JsonInstallManifestRepository,
    JsonInstallRecoveryRecordRepository, JsonModImportResultRepository,
    JsonReinstallRecoveryTransactionRepository, PlatformSteamRootProvider,
    ReadOnlyJsonGamePrerequisiteRuleRepository, RealGameDirectoryProbeFactory,
    SandboxModPackageInstallFileScanner, SteamGameDiscoveryService, SystemClock,
    TaskScopedModImportSandboxLocator,
};
use hmm_ports::{
    BatchPlanFactsProvider, GameAdapter, GameConfigRepository, GamePrerequisiteRuleRepository,
    InstallManifestRepository, InstallRecoveryRecordRepository, InstallSourceFileReader,
    ModImportResultRepository, ModImportSandboxLocator, ModPackageInstallFileReader,
    ModPackageInstallFileScanner, ReinstallRecoveryTransactionRepository, ReinstallSnapshotStore,
    ReplacementAdapter, ReplacementCatalogProvider, StoredModRevision,
};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAX_QUERY_MOD_IDS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanSnapshot {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub action_count: usize,
    pub conflict_count: usize,
    pub has_blocking_conflicts: bool,
    pub prerequisite_decision: GamePrerequisiteDecisionSnapshot,
    pub actions: Vec<InstallPlanActionSnapshot>,
    pub conflicts: Vec<InstallPlanConflictSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_millis: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePrerequisiteDecisionSnapshot {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules_version: Option<u32>,
    pub codes: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanActionSnapshot {
    pub target_path: String,
    pub layer_priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanConflictSnapshot {
    pub target_path: String,
    pub provider_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallPlanSnapshot {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub status: &'static str,
    pub available: bool,
    pub managed_file_count: usize,
    pub backup_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_millis: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReinstallPlanSnapshot {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub candidate_revision_id: String,
    pub status: &'static str,
    pub prerequisite_decision: GamePrerequisiteDecisionSnapshot,
    pub installed_revision_id: Option<String>,
    pub retained_count: usize,
    pub replaced_count: usize,
    pub added_count: usize,
    pub stale_count: usize,
    pub blocking_reasons: Vec<ReinstallBlockingReasonSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_millis: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReinstallBlockingReasonSnapshot {
    pub code: &'static str,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallStatusSnapshot {
    pub game_id: Option<String>,
    pub profile_id: String,
    pub item_count: usize,
    pub items: Vec<InstallStatusItemSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallStatusItemSnapshot {
    pub mod_id: String,
    pub status: &'static str,
    pub managed_file_count: usize,
    pub backup_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryScanSnapshot {
    pub game_id: String,
    pub profile_id: String,
    pub item_count: usize,
    pub items: Vec<InstallRecoveryItemSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryItemSnapshot {
    pub mod_id: String,
    pub status: &'static str,
    pub managed_file_count: usize,
    pub backup_count: usize,
    pub issue_count: usize,
    pub issues: Vec<InstallRecoveryIssueSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryIssueSnapshot {
    pub code: &'static str,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryPreviewSnapshot {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub action: &'static str,
    pub availability: &'static str,
    pub remove_file_count: usize,
    pub restore_file_count: usize,
    pub backup_count: usize,
    pub blocking_issue_count: usize,
    pub blocking_reasons: Vec<InstallRecoveryBlockReasonSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_millis: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecoveryBlockReasonSnapshot {
    pub code: &'static str,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyInstallRecoveryAction {
    RollbackInstall,
    ReconcileReinstall,
}

impl ReadOnlyInstallRecoveryAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RollbackInstall => "rollback_install",
            Self::ReconcileReinstall => "reconcile_reinstall",
        }
    }
}

impl From<ReadOnlyInstallRecoveryAction> for InstallRecoveryActionKind {
    fn from(value: ReadOnlyInstallRecoveryAction) -> Self {
        match value {
            ReadOnlyInstallRecoveryAction::RollbackInstall => Self::RollbackInstall,
            ReadOnlyInstallRecoveryAction::ReconcileReinstall => Self::ReconcileReinstall,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyInstallAutomationError {
    AppDataUnavailable,
    UnsupportedGame,
    ProfileIdInvalid,
    ModIdInvalid,
    SourceRevisionIdInvalid,
    CandidateRevisionIdInvalid,
    SandboxStoragePathRejected,
    ConfiguredGamePathRejected,
    SandboxGamePathRejected,
    GameConfigCorrupted,
    GameConfigUnavailable,
    GameInstanceUnavailable,
    ImportedModNotFound,
    ImportedModCatalogUnavailable,
    ImportedModSandboxUnavailable,
    ImportedModFilesUnavailable,
    // #284：合集包需要调用方自己决定装哪个，不能混进「文件不可用」。
    ImportedModAmbiguousContentRoot,
    InstallPlanInvalid,
    InstallStateInvalid,
    InstallManifestUnavailable,
    InstallRecoveryUnavailable,
    InstallRecoveryPreviewUnavailable,
}

impl ReadOnlyInstallAutomationError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::AppDataUnavailable => "app_data_unavailable",
            Self::UnsupportedGame => "unsupported_game",
            Self::ProfileIdInvalid => "profile_id_invalid",
            Self::ModIdInvalid => "mod_id_invalid",
            Self::SourceRevisionIdInvalid => "source_revision_id_invalid",
            Self::CandidateRevisionIdInvalid => "candidate_revision_id_invalid",
            Self::SandboxStoragePathRejected => "sandbox_storage_path_rejected",
            Self::ConfiguredGamePathRejected => "configured_game_path_rejected",
            Self::SandboxGamePathRejected => "sandbox_game_path_rejected",
            Self::GameConfigCorrupted => "game_config_corrupted",
            Self::GameConfigUnavailable => "game_config_unavailable",
            Self::GameInstanceUnavailable => "game_instance_unavailable",
            Self::ImportedModNotFound => "install_planning_imported_mod_not_found",
            Self::ImportedModCatalogUnavailable => {
                "install_planning_imported_mod_analysis_unavailable"
            }
            Self::ImportedModSandboxUnavailable => {
                "install_planning_imported_mod_sandbox_unavailable"
            }
            Self::ImportedModFilesUnavailable => "install_planning_imported_mod_files_unavailable",
            Self::ImportedModAmbiguousContentRoot => {
                "install_planning_imported_mod_ambiguous_content_root"
            }
            Self::InstallPlanInvalid => "install_plan_invalid",
            Self::InstallStateInvalid => "install_state_invalid",
            Self::InstallManifestUnavailable => "install_manifest_unavailable",
            Self::InstallRecoveryUnavailable => "install_recovery_unavailable",
            Self::InstallRecoveryPreviewUnavailable => {
                "install_recovery_action_preview_unavailable"
            }
        }
    }
}

impl fmt::Display for ReadOnlyInstallAutomationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ReadOnlyInstallAutomationError {}

pub struct ReadOnlyInstallAutomation {
    app_data_dir: PathBuf,
    sandbox_fixture_root: Option<PathBuf>,
    game_config_repository: Arc<dyn GameConfigRepository>,
    catalog: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    planning: Arc<InstallPlanningService>,
    prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
    preflight: Arc<ImportedModInstallPreflightService>,
    manifest_repository: Arc<dyn InstallManifestRepository>,
    install_recovery_repository: Arc<dyn InstallRecoveryRecordRepository>,
    manifest_query: InstallManifestQueryService,
    reinstall_recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository>,
    replacement_workflow: Arc<ReplacementWorkflowService>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleInstallState {
    manifest: Option<InstallManifest>,
    install_recovery: Option<InstallRecoveryRecord>,
    reinstall_recovery: Option<ReinstallRecoveryTransaction>,
}

struct ContainedReadOnlyModImportSandboxLocator {
    delegate: TaskScopedModImportSandboxLocator,
    app_data_root: PathBuf,
    sandbox_root: PathBuf,
}

impl ContainedReadOnlyModImportSandboxLocator {
    fn new(app_data_dir: &Path) -> Self {
        Self {
            delegate: TaskScopedModImportSandboxLocator::new_in_app_data(
                app_data_dir.to_path_buf(),
            ),
            app_data_root: app_data_dir.to_path_buf(),
            sandbox_root: app_data_dir.join("mod-import").join("sandboxes"),
        }
    }
}

impl ModImportSandboxLocator for ContainedReadOnlyModImportSandboxLocator {
    fn sandbox_root_for_package(&self, package_id: &str) -> anyhow::Result<PathBuf> {
        let package_root = self.delegate.sandbox_root_for_package(package_id)?;
        anyhow::ensure!(
            is_canonically_within(&self.sandbox_root, &self.app_data_root),
            "imported mod sandbox root is outside app data"
        );
        anyhow::ensure!(
            is_canonically_within(&package_root, &self.sandbox_root),
            "imported mod sandbox is outside its controlled root"
        );
        Ok(package_root)
    }
}

struct ReadOnlyReinstallCandidateSourceReader {
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
}

impl ReinstallCandidateSourceReader for ReadOnlyReinstallCandidateSourceReader {
    fn read_candidate_source_file(
        &self,
        candidate: &StoredModRevision,
        package_file_id: &PackageFileId,
    ) -> anyhow::Result<Vec<u8>> {
        let source_root = self
            .sandbox_locator
            .sandbox_root_for_package(&candidate.package_id)?;
        FileSystemInstallSourceFileReader::new(source_root).read_source_file(package_file_id)
    }
}

struct ReadOnlyInitialRetargetInstallStatusReader;

impl InitialRetargetInstallStatusReader for ReadOnlyInitialRetargetInstallStatusReader {
    fn recovery_status(
        &self,
        _game_id: &GameId,
        _profile_id: &ProfileId,
        _mod_id: &ModId,
    ) -> Result<InstallRecoveryStatus, InitialRetargetInstallStatusError> {
        Err(InitialRetargetInstallStatusError::Unavailable)
    }
}

struct ReadOnlyBatchReinstallItemFactsReader {
    preview: Arc<ReinstallPreviewService>,
    replacement_workflow: Arc<ReplacementWorkflowService>,
}

impl BatchReinstallItemFactsReader for ReadOnlyBatchReinstallItemFactsReader {
    fn read_item_facts(
        &self,
        request: &BatchReinstallItemFactsRequest,
    ) -> anyhow::Result<BatchItemFacts> {
        if request.input.installed_revision_id != request.input.candidate_revision_id {
            return ReinstallPreviewBatchItemFactsReader::new(Arc::clone(&self.preview))
                .read_item_facts(request);
        }
        let expected_binding = request
            .input
            .replacement_binding_snapshot
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("same-revision replacement binding is missing"))?;
        let context = self.preview.resolve_installed_replacement_context(
            &request.game_id,
            &request.profile_id,
            &request.input.mod_id,
        )?;
        let context = match context {
            InstalledReplacementReinstallResolution::Ready(context)
                if context.installed_revision_id == request.input.installed_revision_id =>
            {
                context
            }
            InstalledReplacementReinstallResolution::Ready(_) => {
                return ReinstallPreviewBatchItemFactsReader::facts_from_preparation(
                    request,
                    ReinstallPreparation::Blocked(blocked_reinstall_preview(
                        &self.preview,
                        request,
                        ReinstallBlockingReason::InstalledRevisionUnknown,
                    )),
                );
            }
            InstalledReplacementReinstallResolution::Blocked(preview) => {
                return ReinstallPreviewBatchItemFactsReader::facts_from_preparation(
                    request,
                    ReinstallPreparation::Blocked(preview),
                );
            }
        };
        let planned = match self.replacement_workflow.preview_reinstall_target(
            PreviewRetargetReinstallRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                mod_id: request.input.mod_id.clone(),
                installed_revision_id: context.installed_revision_id.clone(),
                installed_binding: context.installed_binding,
                target_id: expected_binding.binding().target_id().clone(),
                layer: request.input.layer.clone(),
            },
        ) {
            Ok(planned) => planned,
            Err(_) => {
                return ReinstallPreviewBatchItemFactsReader::facts_from_preparation(
                    request,
                    ReinstallPreparation::Blocked(blocked_reinstall_preview(
                        &self.preview,
                        request,
                        ReinstallBlockingReason::CandidateNotReady,
                    )),
                );
            }
        };
        let preparation = self.preview.prepare_replacement_target_switch(
            ReinstallPreviewRequest {
                game_id: request.game_id.clone(),
                profile_id: request.profile_id.clone(),
                mod_id: request.input.mod_id.clone(),
                candidate_revision_id: context.installed_revision_id,
                layer: request.input.layer.clone(),
            },
            planned.install_plan().clone(),
        )?;
        ReinstallPreviewBatchItemFactsReader::facts_from_preparation(request, preparation)
    }
}

fn blocked_reinstall_preview(
    preview: &ReinstallPreviewService,
    request: &BatchReinstallItemFactsRequest,
    reason: ReinstallBlockingReason,
) -> ReinstallPlanPreview {
    ReinstallPlanPreview {
        status: ReinstallPreviewStatus::Blocked,
        prerequisite_decision: preview.prerequisite_decision(&request.game_id),
        installed_revision: Some(ReinstallRevisionSummary {
            revision_id: request.input.installed_revision_id.clone(),
        }),
        candidate_revision: Some(ReinstallRevisionSummary {
            revision_id: request.input.candidate_revision_id.clone(),
        }),
        counts: ReinstallTargetCounts::default(),
        blocking_reasons: vec![ReinstallBlockingReasonSummary { reason, count: 1 }],
        plan_token: None,
    }
}

impl ReadOnlyInstallAutomation {
    pub fn from_environment(
        environment: &RuntimeEnvironment,
    ) -> Result<Self, ReadOnlyInstallAutomationError> {
        let (app_data_dir, sandbox_fixture_root) =
            if let Some(data_dir) = environment.sandbox_data_dir() {
                validate_sandbox_storage_paths(data_dir)?;
                (data_dir.to_path_buf(), Some(data_dir.join("fixtures")))
            } else {
                (
                    environment
                        .resolved_production_app_data_dir()
                        .ok_or(ReadOnlyInstallAutomationError::AppDataUnavailable)?,
                    None,
                )
            };

        let game_config_repository: Arc<dyn GameConfigRepository> = Arc::new(
            JsonGameConfigRepository::new(app_data_dir.join("config").join("games.json")),
        );
        let catalog: Arc<dyn ModImportResultRepository> =
            Arc::new(JsonModImportResultRepository::new_read_only(
                app_data_dir.join("mod-import").join("results.json"),
            ));
        let sandbox_locator: Arc<dyn ModImportSandboxLocator> =
            Arc::new(ContainedReadOnlyModImportSandboxLocator::new(&app_data_dir));
        let file_scanner: Arc<dyn ModPackageInstallFileScanner> =
            Arc::new(SandboxModPackageInstallFileScanner);
        let file_reader: Arc<dyn ModPackageInstallFileReader> =
            Arc::new(SandboxModPackageInstallFileScanner);
        let prerequisite_rules: Arc<dyn GamePrerequisiteRuleRepository> =
            Arc::new(ReadOnlyJsonGamePrerequisiteRuleRepository::new(
                app_data_dir
                    .join("config")
                    .join("prerequisite-rules")
                    .join("mhw.json"),
            ));
        let game_adapters: Vec<Arc<dyn GameAdapter>> =
            vec![Arc::new(MonsterHunterWorldAdapter::new(prerequisite_rules))];
        let planning = Arc::new(InstallPlanningService::with_imported_mod_sources(
            Arc::clone(&catalog),
            Arc::clone(&sandbox_locator),
            Arc::clone(&file_scanner),
            game_adapters.clone(),
        ));
        let game_setup = Arc::new(GameSetupService::new(
            game_adapters,
            Arc::clone(&game_config_repository),
            Arc::new(RealGameDirectoryProbeFactory),
            Arc::new(SteamGameDiscoveryService::new(Arc::new(
                PlatformSteamRootProvider,
            ))),
            Arc::new(SystemClock),
        ));
        let prerequisites: Arc<dyn GamePrerequisiteDecisionProvider> = game_setup;
        let preflight = Arc::new(ImportedModInstallPreflightService::new(
            Arc::clone(&planning),
            Arc::clone(&prerequisites),
        ));
        let manifest_repository: Arc<dyn InstallManifestRepository> = Arc::new(
            JsonInstallManifestRepository::new(app_data_dir.join("install").join("manifests")),
        );
        let manifest_query = InstallManifestQueryService::new(Arc::clone(&manifest_repository));
        let install_recovery_repository: Arc<dyn InstallRecoveryRecordRepository> = Arc::new(
            JsonInstallRecoveryRecordRepository::new(app_data_dir.join("install").join("recovery")),
        );
        let reinstall_recovery_repository: Arc<dyn ReinstallRecoveryTransactionRepository> =
            Arc::new(JsonReinstallRecoveryTransactionRepository::new(
                app_data_dir.join("install").join("reinstall-recovery"),
            ));
        // WR-05 门禁翻转后武器与防具共用同一聚合 adapter/catalog；
        // environment 只影响 root admission，不再选择 developer seed。
        let replacement_adapters: Vec<Arc<dyn ReplacementAdapter>> =
            vec![Arc::new(MhwReplacementAdapter)];
        let replacement_catalogs: Vec<Arc<dyn ReplacementCatalogProvider>> =
            vec![Arc::new(MhwReplacementCatalog)];
        let replacement_workflow = Arc::new(ReplacementWorkflowService::new(
            replacement_adapters,
            replacement_catalogs,
            Arc::clone(&catalog),
            Arc::clone(&sandbox_locator),
            file_scanner,
            file_reader,
            Arc::new(ReadOnlyInitialRetargetInstallStatusReader),
            Arc::clone(&manifest_repository),
            Arc::new(SystemClock),
        ));

        Ok(Self {
            app_data_dir,
            sandbox_fixture_root,
            game_config_repository,
            catalog,
            sandbox_locator,
            planning,
            prerequisites,
            preflight,
            manifest_repository,
            install_recovery_repository,
            manifest_query,
            reinstall_recovery_repository,
            replacement_workflow,
        })
    }

    pub fn plan(
        &self,
        game_id: &str,
        mod_id: &str,
    ) -> Result<InstallPlanSnapshot, ReadOnlyInstallAutomationError> {
        self.plan_for_profile(game_id, "default", mod_id)
    }

    /// Token 环境与数据根同源：显式 sandbox 根 → Sandbox，OS 解析 app data → Production。
    /// 两种 token 互不通用，preview 签发与写侧校验共用这一推导。
    pub(crate) fn token_environment(
        &self,
    ) -> crate::lifecycle_automation::LifecycleTokenEnvironment {
        if self.sandbox_fixture_root.is_some() {
            crate::lifecycle_automation::LifecycleTokenEnvironment::Sandbox
        } else {
            crate::lifecycle_automation::LifecycleTokenEnvironment::Production
        }
    }

    pub fn plan_for_profile(
        &self,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
    ) -> Result<InstallPlanSnapshot, ReadOnlyInstallAutomationError> {
        let (game_id, profile_id, mod_id, plan, prerequisite_decision) =
            self.build_install_plan(game_id, profile_id, mod_id)?;
        let issued_token = (!plan.has_blocking_conflicts() && !prerequisite_decision.is_blocked())
            .then(|| {
                crate::lifecycle_automation::issue_install_plan_token(
                    self.token_environment(),
                    &game_id,
                    &profile_id,
                    &mod_id,
                    &plan,
                    &prerequisite_decision,
                )
            })
            .transpose()
            .map_err(|_| ReadOnlyInstallAutomationError::InstallPlanInvalid)?;
        let actions = plan
            .actions
            .iter()
            .map(|action| {
                Ok(InstallPlanActionSnapshot {
                    target_path: project_install_target_path(&action.target_path)?,
                    layer_priority: action.provider.layer.priority,
                })
            })
            .collect::<Result<Vec<_>, ReadOnlyInstallAutomationError>>()?;
        let conflicts = plan
            .conflicts
            .iter()
            .map(|conflict| {
                Ok(InstallPlanConflictSnapshot {
                    target_path: project_install_target_path(&conflict.target_path)?,
                    provider_count: conflict.providers.len(),
                })
            })
            .collect::<Result<Vec<_>, ReadOnlyInstallAutomationError>>()?;

        Ok(InstallPlanSnapshot {
            game_id: game_id.as_str().to_owned(),
            profile_id: profile_id.as_str().to_owned(),
            mod_id: mod_id.as_str().to_owned(),
            action_count: actions.len(),
            conflict_count: conflicts.len(),
            has_blocking_conflicts: plan.has_blocking_conflicts(),
            prerequisite_decision: project_prerequisite_decision(&prerequisite_decision),
            actions,
            conflicts,
            plan_token: issued_token.as_ref().map(|issued| issued.token.clone()),
            expires_at_unix_millis: issued_token.map(|issued| issued.expires_at_unix_millis),
        })
    }

    pub(crate) fn build_install_plan(
        &self,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
    ) -> Result<
        (
            GameId,
            ProfileId,
            ModId,
            hmm_core::InstallPlan,
            GamePrerequisiteDecision,
        ),
        ReadOnlyInstallAutomationError,
    > {
        let game_id = parse_game_id(game_id)?;
        let profile_id = ProfileId::new(parse_safe_id(
            profile_id,
            ReadOnlyInstallAutomationError::ProfileIdInvalid,
        )?);
        let mod_id = ModId::new(parse_safe_id(
            mod_id,
            ReadOnlyInstallAutomationError::ModIdInvalid,
        )?);
        let preflight = self
            .preflight
            .preview(BuildImportedModInstallPlanRequest {
                game_id: game_id.clone(),
                mod_id: mod_id.clone(),
                layer: base_file_layer(),
            })
            .map_err(map_planning_error)?;
        Ok((
            game_id,
            profile_id,
            mod_id,
            preflight.plan,
            preflight.prerequisite_decision,
        ))
    }

    pub(crate) fn build_install_plan_for_revision(
        &self,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
        revision_id: &str,
        layer: &FileLayer,
    ) -> Result<
        (
            GameId,
            ProfileId,
            ModId,
            ModRevisionId,
            hmm_core::InstallPlan,
            GamePrerequisiteDecision,
        ),
        ReadOnlyInstallAutomationError,
    > {
        let game_id = parse_game_id(game_id)?;
        let profile_id = ProfileId::new(parse_safe_id(
            profile_id,
            ReadOnlyInstallAutomationError::ProfileIdInvalid,
        )?);
        let mod_id = ModId::new(parse_safe_id(
            mod_id,
            ReadOnlyInstallAutomationError::ModIdInvalid,
        )?);
        let revision_id = ModRevisionId::new(parse_safe_id(
            revision_id,
            ReadOnlyInstallAutomationError::SourceRevisionIdInvalid,
        )?);
        let preflight = self
            .preflight
            .preview_revision(&game_id, &mod_id, &revision_id, layer)
            .map_err(map_planning_error)?;
        let mut plan = preflight.plan;
        if !plan.replacement_bindings.is_empty() {
            return Err(ReadOnlyInstallAutomationError::InstallPlanInvalid);
        }
        if let Ok(Some(canonical_plan)) = self
            .replacement_workflow
            .preview_canonical_source_install_plan(
                &game_id,
                &profile_id,
                &mod_id,
                &revision_id,
                layer,
            )
        {
            if let [binding] = canonical_plan.replacement_bindings.as_slice() {
                if canonical_plan.actions == plan.actions
                    && canonical_plan.conflicts == plan.conflicts
                    && is_identity_replacement_binding(binding)
                    && binding.mod_id() == &mod_id
                    && binding.profile_id() == &profile_id
                    && binding.revision_id() == Some(&revision_id)
                {
                    plan = plan
                        .with_replacement_bindings(vec![binding.clone()])
                        .map_err(|_| ReadOnlyInstallAutomationError::InstallPlanInvalid)?;
                }
            }
        }
        Ok((
            game_id,
            profile_id,
            mod_id,
            revision_id,
            plan,
            preflight.prerequisite_decision,
        ))
    }

    pub fn reinstall_preview(
        &self,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
        candidate_revision_id: &str,
    ) -> Result<ReinstallPlanSnapshot, ReadOnlyInstallAutomationError> {
        let (game_id, profile_id, mod_id, candidate_revision_id, preview) =
            self.build_reinstall_facts(game_id, profile_id, mod_id, candidate_revision_id)?;
        let available = preview.status == ReinstallPreviewStatus::Ready;
        let issued_token = available
            .then(|| {
                crate::lifecycle_automation::issue_reinstall_plan_token(
                    self.token_environment(),
                    &game_id,
                    &profile_id,
                    &mod_id,
                    &candidate_revision_id,
                    &preview,
                )
            })
            .transpose()
            .map_err(|_| ReadOnlyInstallAutomationError::InstallPlanInvalid)?;

        Ok(ReinstallPlanSnapshot {
            game_id: game_id.as_str().to_owned(),
            profile_id: profile_id.as_str().to_owned(),
            mod_id: mod_id.as_str().to_owned(),
            candidate_revision_id: candidate_revision_id.as_str().to_owned(),
            status: reinstall_preview_status_code(preview.status),
            prerequisite_decision: project_prerequisite_decision(&preview.prerequisite_decision),
            installed_revision_id: preview
                .installed_revision
                .map(|revision| revision.revision_id.as_str().to_owned()),
            retained_count: preview.counts.retained,
            replaced_count: preview.counts.replaced,
            added_count: preview.counts.added,
            stale_count: preview.counts.stale,
            blocking_reasons: preview
                .blocking_reasons
                .into_iter()
                .map(|reason| ReinstallBlockingReasonSnapshot {
                    code: reinstall_blocking_reason_code(reason.reason),
                    count: reason.count,
                })
                .collect(),
            plan_token: issued_token.as_ref().map(|issued| issued.token.clone()),
            expires_at_unix_millis: issued_token.map(|issued| issued.expires_at_unix_millis),
        })
    }

    pub(crate) fn build_reinstall_facts(
        &self,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
        candidate_revision_id: &str,
    ) -> Result<
        (
            GameId,
            ProfileId,
            ModId,
            ModRevisionId,
            ReinstallPlanPreview,
        ),
        ReadOnlyInstallAutomationError,
    > {
        let game_id = parse_game_id(game_id)?;
        let profile_id = ProfileId::new(parse_safe_id(
            profile_id,
            ReadOnlyInstallAutomationError::ProfileIdInvalid,
        )?);
        let mod_id = ModId::new(parse_safe_id(
            mod_id,
            ReadOnlyInstallAutomationError::ModIdInvalid,
        )?);
        let candidate_revision_id = ModRevisionId::new(parse_safe_id(
            candidate_revision_id,
            ReadOnlyInstallAutomationError::CandidateRevisionIdInvalid,
        )?);
        let service = self.reinstall_preview_service(&game_id)?;
        let preview = service
            .preview(ReinstallPreviewRequest {
                game_id: game_id.clone(),
                profile_id: profile_id.clone(),
                mod_id: mod_id.clone(),
                candidate_revision_id: candidate_revision_id.clone(),
                layer: base_file_layer(),
            })
            .map_err(map_reinstall_preview_error)?;
        Ok((game_id, profile_id, mod_id, candidate_revision_id, preview))
    }

    pub(crate) fn resolve_batch_replacement_binding(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        input: &ReinstallBatchItemInput,
        target_id: &ReplacementTargetId,
    ) -> anyhow::Result<ReplacementBindingSnapshot> {
        anyhow::ensure!(
            input.installed_revision_id == input.candidate_revision_id
                && input.replacement_binding_snapshot.is_none(),
            "replacement target is only valid for unresolved same-revision reinstall"
        );
        let preview = self.reinstall_preview_service(game_id)?;
        let context =
            preview.resolve_installed_replacement_context(game_id, profile_id, &input.mod_id)?;
        let InstalledReplacementReinstallResolution::Ready(context) = context else {
            anyhow::bail!("installed replacement context is unavailable");
        };
        anyhow::ensure!(
            context.installed_revision_id == input.installed_revision_id,
            "installed replacement revision changed"
        );
        let planned = self.replacement_workflow.preview_reinstall_target(
            PreviewRetargetReinstallRequest {
                game_id: game_id.clone(),
                profile_id: profile_id.clone(),
                mod_id: input.mod_id.clone(),
                installed_revision_id: context.installed_revision_id,
                installed_binding: context.installed_binding,
                target_id: target_id.clone(),
                layer: input.layer.clone(),
            },
        )?;
        let [binding] = planned.install_plan().replacement_bindings.as_slice() else {
            anyhow::bail!("replacement plan has an invalid binding snapshot");
        };
        // 比对 catalog 解析出的规范目标，而不是请求里那个原样字符串。
        // 请求可能带的是 legacy_ids 里的旧 slug（玩家已安装 manifest 或既有脚本里
        // 存的就是它），解析后 binding 记录的必然是规范 ID，直接比原串会必错。
        // 校验强度不变：仍然确认拿回来的计划就是所请求目标的计划。
        anyhow::ensure!(
            binding.mod_id() == &input.mod_id
                && binding.profile_id() == profile_id
                && binding.revision_id() == Some(&input.installed_revision_id)
                && binding.binding().target_id() == planned.target().id(),
            "replacement binding does not match the requested identity"
        );
        Ok(binding.clone())
    }

    pub(crate) fn read_batch_uninstall_facts(
        &self,
        request: &NormalizedBatchPlanRequest,
        environment_digest: String,
    ) -> anyhow::Result<BatchPlanFacts> {
        let game_instance = self.load_admitted_game_instance(&request.game_id)?;
        let backup_store = Arc::new(FileSystemInstallBackupStore::new(
            self.app_data_dir.join("install").join("backups"),
        ));
        let snapshot_store: Arc<dyn ReinstallSnapshotStore> = backup_store.clone();
        let recovery_scan = InstallRecoveryScanService::new_with_recovery_records(
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            backup_store,
            Arc::clone(&self.manifest_repository),
            Arc::clone(&self.install_recovery_repository),
        )
        .with_reinstall_recovery_transactions(
            Arc::clone(&self.reinstall_recovery_repository),
            snapshot_store,
        );
        BatchUninstallPlanFactsProvider::new(
            Arc::clone(&self.manifest_repository),
            recovery_scan,
            environment_digest,
        )
        .read_batch_plan_facts(request)
    }

    pub(crate) fn read_batch_reinstall_facts(
        &self,
        request: &NormalizedBatchPlanRequest,
        environment_digest: String,
    ) -> anyhow::Result<BatchPlanFacts> {
        let preview = self.reinstall_preview_service(&request.game_id)?;
        let item_facts: Arc<dyn BatchReinstallItemFactsReader> =
            Arc::new(ReadOnlyBatchReinstallItemFactsReader {
                preview,
                replacement_workflow: Arc::clone(&self.replacement_workflow),
            });
        BatchReinstallPlanFactsProvider::new(
            item_facts,
            Arc::clone(&self.manifest_repository),
            Arc::clone(&self.install_recovery_repository),
            Arc::clone(&self.reinstall_recovery_repository),
            environment_digest,
        )
        .read_batch_plan_facts(request)
    }

    pub fn uninstall_preview(
        &self,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
    ) -> Result<UninstallPlanSnapshot, ReadOnlyInstallAutomationError> {
        let (game_id, profile_id, mod_id, summary, state_binding) =
            self.build_uninstall_facts(game_id, profile_id, mod_id)?;
        let available = summary.status == InstallRecoveryStatus::Completed;
        let issued_token = available
            .then(|| {
                crate::lifecycle_automation::issue_uninstall_plan_token(
                    self.token_environment(),
                    &game_id,
                    &profile_id,
                    &mod_id,
                    &summary,
                    &state_binding,
                )
            })
            .transpose()
            .map_err(|_| ReadOnlyInstallAutomationError::InstallStateInvalid)?;

        Ok(UninstallPlanSnapshot {
            game_id: game_id.as_str().to_owned(),
            profile_id: profile_id.as_str().to_owned(),
            mod_id: mod_id.as_str().to_owned(),
            status: recovery_status_code(summary.status),
            available,
            managed_file_count: summary.managed_file_count,
            backup_count: summary.backup_count,
            plan_token: issued_token.as_ref().map(|issued| issued.token.clone()),
            expires_at_unix_millis: issued_token.map(|issued| issued.expires_at_unix_millis),
        })
    }

    pub(crate) fn build_uninstall_facts(
        &self,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
    ) -> Result<
        (GameId, ProfileId, ModId, InstallRecoverySummary, String),
        ReadOnlyInstallAutomationError,
    > {
        let game_id = parse_game_id(game_id)?;
        let profile_id = ProfileId::new(parse_safe_id(
            profile_id,
            ReadOnlyInstallAutomationError::ProfileIdInvalid,
        )?);
        let mod_id = ModId::new(parse_safe_id(
            mod_id,
            ReadOnlyInstallAutomationError::ModIdInvalid,
        )?);
        let state_before = self.load_lifecycle_install_state(&profile_id, &mod_id)?;
        let mut summaries = self.scan_recovery(
            game_id.clone(),
            InstallRecoveryScanRequest {
                profile_id: profile_id.clone(),
                mod_ids: vec![mod_id.clone()],
            },
        )?;
        if summaries.len() != 1 {
            return Err(ReadOnlyInstallAutomationError::InstallStateInvalid);
        }
        let state_after = self.load_lifecycle_install_state(&profile_id, &mod_id)?;
        if state_before != state_after {
            return Err(ReadOnlyInstallAutomationError::InstallStateInvalid);
        }
        let state_binding = crate::lifecycle_automation::lifecycle_state_binding(&state_after)
            .map_err(|_| ReadOnlyInstallAutomationError::InstallStateInvalid)?;
        Ok((
            game_id,
            profile_id,
            mod_id,
            summaries.remove(0),
            state_binding,
        ))
    }

    pub fn status(
        &self,
        game_id: Option<&str>,
        profile_id: &str,
        mod_ids: &[String],
    ) -> Result<InstallStatusSnapshot, ReadOnlyInstallAutomationError> {
        let profile_id = ProfileId::new(parse_safe_id(
            profile_id,
            ReadOnlyInstallAutomationError::ProfileIdInvalid,
        )?);
        let mod_ids = parse_mod_ids(mod_ids, false)?;
        let (game_id, items) = if let Some(game_id) = game_id {
            let game_id = parse_game_id(game_id)?;
            let summaries = self.scan_recovery(
                game_id.clone(),
                InstallRecoveryScanRequest {
                    profile_id: profile_id.clone(),
                    mod_ids,
                },
            )?;
            let items: Vec<InstallStatusItemSnapshot> = summaries
                .into_iter()
                .map(|summary| InstallStatusItemSnapshot {
                    mod_id: summary.mod_id.as_str().to_owned(),
                    status: recovery_status_code(summary.status),
                    managed_file_count: summary.managed_file_count,
                    backup_count: summary.backup_count,
                })
                .collect();
            (Some(game_id.as_str().to_owned()), items)
        } else {
            let summaries = self
                .manifest_query
                .query_statuses(InstallManifestQueryRequest {
                    profile_id: profile_id.clone(),
                    mod_ids,
                })
                .map_err(|_| ReadOnlyInstallAutomationError::InstallManifestUnavailable)?;
            let items: Vec<InstallStatusItemSnapshot> = summaries
                .into_iter()
                .map(|summary| InstallStatusItemSnapshot {
                    mod_id: summary.mod_id.as_str().to_owned(),
                    status: manifest_status_code(summary.status),
                    managed_file_count: summary.managed_file_count,
                    backup_count: summary.backup_count,
                })
                .collect();
            (None, items)
        };

        Ok(InstallStatusSnapshot {
            game_id,
            profile_id: profile_id.as_str().to_owned(),
            item_count: items.len(),
            items,
        })
    }

    pub fn recovery_scan(
        &self,
        game_id: &str,
        profile_id: &str,
        mod_ids: &[String],
    ) -> Result<InstallRecoveryScanSnapshot, ReadOnlyInstallAutomationError> {
        let game_id = parse_game_id(game_id)?;
        let profile_id = ProfileId::new(parse_safe_id(
            profile_id,
            ReadOnlyInstallAutomationError::ProfileIdInvalid,
        )?);
        let summaries = self.scan_recovery(
            game_id.clone(),
            InstallRecoveryScanRequest {
                profile_id: profile_id.clone(),
                mod_ids: parse_mod_ids(mod_ids, true)?,
            },
        )?;
        let items = summaries
            .into_iter()
            .map(|summary| InstallRecoveryItemSnapshot {
                mod_id: summary.mod_id.as_str().to_owned(),
                status: recovery_status_code(summary.status),
                managed_file_count: summary.managed_file_count,
                backup_count: summary.backup_count,
                issue_count: summary.issue_count,
                issues: summary
                    .issues
                    .into_iter()
                    .map(|issue| InstallRecoveryIssueSnapshot {
                        code: recovery_issue_code(issue.issue),
                        count: issue.count,
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();

        Ok(InstallRecoveryScanSnapshot {
            game_id: game_id.as_str().to_owned(),
            profile_id: profile_id.as_str().to_owned(),
            item_count: items.len(),
            items,
        })
    }

    pub fn recovery_preview(
        &self,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
        action: ReadOnlyInstallRecoveryAction,
    ) -> Result<InstallRecoveryPreviewSnapshot, ReadOnlyInstallAutomationError> {
        let (game_id, profile_id, mod_id, preview, state_binding) =
            self.build_recovery_preview_facts(game_id, profile_id, mod_id, action)?;
        let available = preview.availability == InstallRecoveryActionAvailability::Available;
        let issued_token = available
            .then(|| {
                crate::lifecycle_automation::issue_recovery_plan_token(
                    self.token_environment(),
                    &game_id,
                    &profile_id,
                    &mod_id,
                    &preview,
                    &state_binding,
                )
            })
            .transpose()
            .map_err(|_| ReadOnlyInstallAutomationError::InstallRecoveryPreviewUnavailable)?;

        Ok(InstallRecoveryPreviewSnapshot {
            game_id: game_id.as_str().to_owned(),
            profile_id: profile_id.as_str().to_owned(),
            mod_id: mod_id.as_str().to_owned(),
            action: action.as_str(),
            availability: if available { "available" } else { "blocked" },
            remove_file_count: preview.remove_file_count,
            restore_file_count: preview.restore_file_count,
            backup_count: preview.backup_count,
            blocking_issue_count: preview.blocking_issue_count,
            blocking_reasons: preview
                .blocking_reasons
                .into_iter()
                .map(|reason| InstallRecoveryBlockReasonSnapshot {
                    code: recovery_block_reason_code(reason.reason),
                    count: reason.count,
                })
                .collect(),
            plan_token: issued_token.as_ref().map(|issued| issued.token.clone()),
            expires_at_unix_millis: issued_token.map(|issued| issued.expires_at_unix_millis),
        })
    }

    pub(crate) fn build_recovery_preview_facts(
        &self,
        game_id: &str,
        profile_id: &str,
        mod_id: &str,
        action: ReadOnlyInstallRecoveryAction,
    ) -> Result<
        (
            GameId,
            ProfileId,
            ModId,
            InstallRecoveryActionPreview,
            String,
        ),
        ReadOnlyInstallAutomationError,
    > {
        let game_id = parse_game_id(game_id)?;
        let profile_id = ProfileId::new(parse_safe_id(
            profile_id,
            ReadOnlyInstallAutomationError::ProfileIdInvalid,
        )?);
        let mod_id = ModId::new(parse_safe_id(
            mod_id,
            ReadOnlyInstallAutomationError::ModIdInvalid,
        )?);
        let state_before = self.load_lifecycle_install_state(&profile_id, &mod_id)?;
        let game_instance = self.load_admitted_game_instance(&game_id)?;
        let service = InstallRecoveryActionPreviewService::new(
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            Arc::new(FileSystemInstallBackupStore::new(
                self.app_data_dir.join("install").join("backups"),
            )),
            Arc::clone(&self.install_recovery_repository),
        );
        let preview = service
            .preview(InstallRecoveryActionPreviewRequest {
                profile_id: profile_id.clone(),
                mod_id: mod_id.clone(),
                action_kind: action.into(),
            })
            .map_err(|_| ReadOnlyInstallAutomationError::InstallRecoveryPreviewUnavailable)?;
        let state_after = self.load_lifecycle_install_state(&profile_id, &mod_id)?;
        if state_before != state_after {
            return Err(ReadOnlyInstallAutomationError::InstallRecoveryPreviewUnavailable);
        }
        let state_binding = crate::lifecycle_automation::lifecycle_state_binding(&state_after)
            .map_err(|_| ReadOnlyInstallAutomationError::InstallRecoveryPreviewUnavailable)?;
        Ok((game_id, profile_id, mod_id, preview, state_binding))
    }

    fn load_lifecycle_install_state(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<LifecycleInstallState, ReadOnlyInstallAutomationError> {
        let manifest = self
            .manifest_repository
            .load_manifest(profile_id)
            .map_err(|_| ReadOnlyInstallAutomationError::InstallManifestUnavailable)?;
        let install_recovery = self
            .install_recovery_repository
            .load_record(profile_id, mod_id)
            .map_err(|_| ReadOnlyInstallAutomationError::InstallRecoveryUnavailable)?;
        let reinstall_recovery = self
            .reinstall_recovery_repository
            .load_transaction(profile_id, mod_id)
            .map_err(|_| ReadOnlyInstallAutomationError::InstallRecoveryUnavailable)?;
        Ok(LifecycleInstallState {
            manifest,
            install_recovery,
            reinstall_recovery,
        })
    }

    pub(crate) fn load_lifecycle_state_binding(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<String, ReadOnlyInstallAutomationError> {
        let state = self.load_lifecycle_install_state(profile_id, mod_id)?;
        crate::lifecycle_automation::lifecycle_state_binding(&state)
            .map_err(|_| ReadOnlyInstallAutomationError::InstallStateInvalid)
    }

    fn scan_recovery(
        &self,
        game_id: GameId,
        request: InstallRecoveryScanRequest,
    ) -> Result<Vec<hmm_app::InstallRecoverySummary>, ReadOnlyInstallAutomationError> {
        let game_instance = self.load_admitted_game_instance(&game_id)?;
        let backup_store = Arc::new(FileSystemInstallBackupStore::new(
            self.app_data_dir.join("install").join("backups"),
        ));
        let snapshot_store: Arc<dyn ReinstallSnapshotStore> = backup_store.clone();
        let service = InstallRecoveryScanService::new_with_recovery_records(
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            backup_store,
            Arc::new(JsonInstallManifestRepository::new(
                self.app_data_dir.join("install").join("manifests"),
            )),
            Arc::new(JsonInstallRecoveryRecordRepository::new(
                self.app_data_dir.join("install").join("recovery"),
            )),
        )
        .with_reinstall_recovery_transactions(
            Arc::clone(&self.reinstall_recovery_repository),
            snapshot_store,
        );

        let summaries = service
            .scan(request)
            .map_err(|_| ReadOnlyInstallAutomationError::InstallRecoveryUnavailable)?;
        if summaries
            .iter()
            .any(|summary| !is_safe_projected_id(summary.mod_id.as_str()))
        {
            return Err(ReadOnlyInstallAutomationError::InstallStateInvalid);
        }
        Ok(summaries)
    }

    fn reinstall_preview_service(
        &self,
        game_id: &GameId,
    ) -> Result<Arc<ReinstallPreviewService>, ReadOnlyInstallAutomationError> {
        let game_instance = self.load_admitted_game_instance(game_id)?;
        let backup_store = Arc::new(FileSystemInstallBackupStore::new(
            self.app_data_dir.join("install").join("backups"),
        ));
        let source: Arc<dyn ReinstallCandidateSourceReader> =
            Arc::new(ReadOnlyReinstallCandidateSourceReader {
                sandbox_locator: Arc::clone(&self.sandbox_locator),
            });
        Ok(Arc::new(ReinstallPreviewService::new(
            Arc::clone(&self.prerequisites),
            Arc::clone(&self.catalog),
            self.planning.clone(),
            source,
            Arc::new(FileSystemInstallGameFileSystem::new(game_instance.root_dir)),
            backup_store,
            Arc::clone(&self.manifest_repository),
            Arc::clone(&self.reinstall_recovery_repository),
        )))
    }

    fn load_admitted_game_instance(
        &self,
        game_id: &GameId,
    ) -> Result<GameInstance, ReadOnlyInstallAutomationError> {
        let instance = self
            .game_config_repository
            .load_game_instance(game_id)
            .map_err(|error| match error {
                hmm_ports::GameConfigRepositoryError::StorageCorrupted => {
                    ReadOnlyInstallAutomationError::GameConfigCorrupted
                }
                hmm_ports::GameConfigRepositoryError::StorageFailed(_) => {
                    ReadOnlyInstallAutomationError::GameConfigUnavailable
                }
            })?
            .ok_or(ReadOnlyInstallAutomationError::GameInstanceUnavailable)?;

        if !is_safe_absolute_path(&instance.root_dir) {
            return Err(ReadOnlyInstallAutomationError::ConfiguredGamePathRejected);
        }
        if self
            .sandbox_fixture_root
            .as_ref()
            .is_some_and(|fixture_root| !is_canonically_within(&instance.root_dir, fixture_root))
        {
            return Err(ReadOnlyInstallAutomationError::SandboxGamePathRejected);
        }

        Ok(instance)
    }
}

fn validate_sandbox_storage_paths(data_dir: &Path) -> Result<(), ReadOnlyInstallAutomationError> {
    if !data_dir.is_dir() || !is_canonically_within(data_dir, data_dir) {
        return Err(ReadOnlyInstallAutomationError::SandboxStoragePathRejected);
    }

    let managed_paths = [
        data_dir.join("config").join("games.json"),
        data_dir
            .join("config")
            .join("prerequisite-rules")
            .join("mhw.json"),
        data_dir.join("mod-import").join("results.json"),
        data_dir.join("mod-import").join("sandboxes"),
        data_dir.join("install").join("manifests"),
        data_dir.join("install").join("recovery"),
        data_dir.join("install").join("reinstall-recovery"),
        data_dir.join("install").join("backups"),
    ];
    if managed_paths
        .iter()
        .any(|path| !is_canonically_within(path, data_dir))
    {
        return Err(ReadOnlyInstallAutomationError::SandboxStoragePathRejected);
    }

    Ok(())
}

fn base_file_layer() -> FileLayer {
    FileLayer::new("base", 0)
}

fn parse_game_id(value: &str) -> Result<GameId, ReadOnlyInstallAutomationError> {
    GameId::parse(value).map_err(|_| ReadOnlyInstallAutomationError::UnsupportedGame)
}

fn parse_mod_ids(
    values: &[String],
    allow_empty: bool,
) -> Result<Vec<ModId>, ReadOnlyInstallAutomationError> {
    if values.len() > MAX_QUERY_MOD_IDS || (!allow_empty && values.is_empty()) {
        return Err(ReadOnlyInstallAutomationError::ModIdInvalid);
    }

    values
        .iter()
        .map(|value| {
            parse_safe_id(value, ReadOnlyInstallAutomationError::ModIdInvalid).map(ModId::new)
        })
        .collect::<Result<BTreeSet<_>, _>>()
        .map(BTreeSet::into_iter)
        .map(Iterator::collect)
}

fn parse_safe_id(
    value: &str,
    error: ReadOnlyInstallAutomationError,
) -> Result<String, ReadOnlyInstallAutomationError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || matches!(value, "." | "..")
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(error);
    }
    Ok(value.to_owned())
}

fn is_safe_projected_id(value: &str) -> bool {
    parse_safe_id(value, ReadOnlyInstallAutomationError::InstallStateInvalid)
        .is_ok_and(|parsed| parsed == value)
}

fn project_prerequisite_decision(
    decision: &GamePrerequisiteDecision,
) -> GamePrerequisiteDecisionSnapshot {
    GamePrerequisiteDecisionSnapshot {
        status: decision.status.as_str(),
        rules_version: decision.rules_version,
        codes: decision.codes.iter().map(|code| code.as_str()).collect(),
    }
}

fn project_install_target_path(
    target_path: &InstallTargetPath,
) -> Result<String, ReadOnlyInstallAutomationError> {
    let value = target_path.as_str();
    if value.chars().any(char::is_control) {
        return Err(ReadOnlyInstallAutomationError::InstallPlanInvalid);
    }
    Ok(value.to_owned())
}

fn map_planning_error(error: InstallPlanningError) -> ReadOnlyInstallAutomationError {
    match error {
        InstallPlanningError::ImportedModNotFound { .. } => {
            ReadOnlyInstallAutomationError::ImportedModNotFound
        }
        InstallPlanningError::ImportedModAnalysisUnavailable => {
            ReadOnlyInstallAutomationError::ImportedModCatalogUnavailable
        }
        InstallPlanningError::ImportedModSandboxUnavailable => {
            ReadOnlyInstallAutomationError::ImportedModSandboxUnavailable
        }
        InstallPlanningError::ImportedModFileScanUnavailable => {
            ReadOnlyInstallAutomationError::ImportedModFilesUnavailable
        }
        InstallPlanningError::ImportedModAmbiguousContentRoot => {
            ReadOnlyInstallAutomationError::ImportedModAmbiguousContentRoot
        }
        InstallPlanningError::InvalidTargetPath { .. }
        | InstallPlanningError::ImportedModSourcesUnavailable
        | InstallPlanningError::GameAdapterNotFound { .. } => {
            ReadOnlyInstallAutomationError::InstallPlanInvalid
        }
    }
}

fn map_reinstall_preview_error(error: ReinstallPreviewError) -> ReadOnlyInstallAutomationError {
    match error {
        ReinstallPreviewError::CatalogUnavailable => {
            ReadOnlyInstallAutomationError::ImportedModCatalogUnavailable
        }
        ReinstallPreviewError::ManifestUnavailable => {
            ReadOnlyInstallAutomationError::InstallManifestUnavailable
        }
        ReinstallPreviewError::RecoveryUnavailable => {
            ReadOnlyInstallAutomationError::InstallRecoveryUnavailable
        }
        ReinstallPreviewError::CandidatePlanUnavailable => {
            ReadOnlyInstallAutomationError::ImportedModFilesUnavailable
        }
    }
}

fn reinstall_preview_status_code(status: ReinstallPreviewStatus) -> &'static str {
    match status {
        ReinstallPreviewStatus::Ready => "ready",
        ReinstallPreviewStatus::Blocked => "blocked",
    }
}

fn reinstall_blocking_reason_code(reason: ReinstallBlockingReason) -> &'static str {
    match reason {
        ReinstallBlockingReason::PrerequisitesBlocked => "prerequisites_blocked",
        ReinstallBlockingReason::NotInstalled => "not_installed",
        ReinstallBlockingReason::CandidateNotFound => "candidate_not_found",
        ReinstallBlockingReason::CandidateNotReady => "candidate_not_ready",
        ReinstallBlockingReason::CandidateOwnerMismatch => "candidate_owner_mismatch",
        ReinstallBlockingReason::CandidateAlreadyInstalled => "candidate_already_installed",
        ReinstallBlockingReason::ManifestStateUnsafe => "manifest_state_unsafe",
        ReinstallBlockingReason::InstalledRevisionUnknown => "installed_revision_unknown",
        ReinstallBlockingReason::SourceUnavailable => "source_unavailable",
        ReinstallBlockingReason::TargetMissing => "target_missing",
        ReinstallBlockingReason::TargetChanged => "target_changed",
        ReinstallBlockingReason::TargetReadFailed => "target_read_failed",
        ReinstallBlockingReason::BackupMissing => "backup_missing",
        ReinstallBlockingReason::BackupReadFailed => "backup_read_failed",
        ReinstallBlockingReason::PlanConflict => "plan_conflict",
        ReinstallBlockingReason::CrossModTargetConflict => "cross_mod_target_conflict",
    }
}

fn manifest_status_code(status: InstallManifestStatus) -> &'static str {
    match status {
        InstallManifestStatus::NotInstalled => "not_installed",
        InstallManifestStatus::Installed => "installed",
        InstallManifestStatus::CommittedCleanupPending => "committed_cleanup_pending",
        InstallManifestStatus::CleanupPending => "cleanup_pending",
        InstallManifestStatus::RollbackRequired => "rollback_required",
        InstallManifestStatus::RepairRequired => "repair_required",
        InstallManifestStatus::Unknown => "unknown",
    }
}

fn recovery_status_code(status: InstallRecoveryStatus) -> &'static str {
    match status {
        InstallRecoveryStatus::NotInstalled => "not_installed",
        InstallRecoveryStatus::Completed => "installed",
        InstallRecoveryStatus::CommittedCleanupPending => "committed_cleanup_pending",
        InstallRecoveryStatus::CleanupPending => "cleanup_pending",
        InstallRecoveryStatus::RollbackRequired => "rollback_required",
        InstallRecoveryStatus::RepairRequired => "repair_required",
        InstallRecoveryStatus::Unknown => "unknown",
    }
}

fn recovery_issue_code(issue: InstallRecoveryIssue) -> &'static str {
    match issue {
        InstallRecoveryIssue::MissingInstalledFileSummary => "missing_installed_file_summary",
        InstallRecoveryIssue::TargetMissing => "target_missing",
        InstallRecoveryIssue::TargetChanged => "target_changed",
        InstallRecoveryIssue::TargetReadFailed => "target_read_failed",
        InstallRecoveryIssue::BackupMissing => "backup_missing",
        InstallRecoveryIssue::BackupReadFailed => "backup_read_failed",
    }
}

fn recovery_block_reason_code(reason: InstallRecoveryActionBlockReason) -> &'static str {
    match reason {
        InstallRecoveryActionBlockReason::RollbackStateMissing => "rollback_state_missing",
        InstallRecoveryActionBlockReason::MissingInstalledFileSummary => {
            "missing_installed_file_summary"
        }
        InstallRecoveryActionBlockReason::TargetMissing => "target_missing",
        InstallRecoveryActionBlockReason::TargetChanged => "target_changed",
        InstallRecoveryActionBlockReason::TargetReadFailed => "target_read_failed",
        InstallRecoveryActionBlockReason::BackupMissing => "backup_missing",
        InstallRecoveryActionBlockReason::BackupReadFailed => "backup_read_failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{InstallManifest, InstallManifestEntry, PackageFileId};
    use std::collections::BTreeMap;
    use std::fs;

    fn write_game_config(data_dir: &Path, game_root: &Path) {
        let config_dir = data_dir.join("config");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(
            config_dir.join("games.json"),
            serde_json::json!({
                "version": 1,
                "games": [{
                    "id": "mhw-default",
                    "game_id": "mhw",
                    "display_name": "Monster Hunter: World - Iceborne",
                    "root_dir": game_root,
                    "status": "configured",
                    "configured_at_unix_millis": 42
                }]
            })
            .to_string(),
        )
        .expect("write game config");
    }

    fn create_game_fixture(data_dir: &Path) -> PathBuf {
        let game_root = data_dir.join("fixtures").join("games").join("mhw-minimal");
        fs::create_dir_all(&game_root).expect("create game fixture");
        fs::write(game_root.join("MonsterHunterWorld.exe"), b"fixture").expect("write game exe");
        game_root
    }

    fn write_v1_mod_catalog_and_sandbox(data_dir: &Path) {
        let catalog_dir = data_dir.join("mod-import");
        fs::create_dir_all(&catalog_dir).expect("create catalog dir");
        fs::write(
            catalog_dir.join("results.json"),
            r#"{
  "version": 1,
  "records": [{
    "mod_id": "mod-a",
    "task_id": "task-a",
    "package_id": "package-a",
    "display_name": "Fixture Mod"
  }]
}"#,
        )
        .expect("write catalog");
        let package_root = catalog_dir.join("sandboxes").join("package-a");
        fs::create_dir_all(package_root.join("nativePC").join("models"))
            .expect("create package fixture");
        fs::write(
            package_root
                .join("nativePC")
                .join("models")
                .join("player.mod3"),
            b"fixture",
        )
        .expect("write package file");
    }

    fn write_manifest_with_mod_id(data_dir: &Path, mod_id: &str) {
        let repository =
            JsonInstallManifestRepository::new(data_dir.join("install").join("manifests"));
        let target_path =
            InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).expect("target");
        repository
            .save_manifest(&InstallManifest::completed(
                ProfileId::new("default"),
                vec![InstallManifestEntry {
                    target_path,
                    mod_id: ModId::new(mod_id),
                    revision_id: None,
                    package_file_id: PackageFileId::new("fixture-file"),
                    layer: base_file_layer(),
                    backup_ref: None,
                    installed_file: None,
                }],
            ))
            .expect("write manifest");
    }

    fn tree_snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, directory: &Path, snapshot: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            let mut entries = fs::read_dir(directory)
                .expect("read snapshot directory")
                .collect::<Result<Vec<_>, _>>()
                .expect("read snapshot entries");
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .expect("snapshot relative path")
                    .to_path_buf();
                if path.is_dir() {
                    snapshot.insert(relative, None);
                    visit(root, &path, snapshot);
                } else {
                    snapshot.insert(relative, Some(fs::read(path).expect("read snapshot file")));
                }
            }
        }

        let mut snapshot = BTreeMap::new();
        visit(root, root, &mut snapshot);
        snapshot
    }

    #[cfg(unix)]
    fn create_directory_link(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create directory symlink");
    }

    #[cfg(windows)]
    fn create_directory_link(target: &Path, link: &Path) {
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_str().expect("link path"),
                target.to_str().expect("target path"),
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
    fn sandbox_install_queries_are_path_safe_and_do_not_write() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let game_root = create_game_fixture(sandbox.path());
        write_game_config(sandbox.path(), &game_root);
        write_v1_mod_catalog_and_sandbox(sandbox.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let before = tree_snapshot(sandbox.path());
        let automation =
            ReadOnlyInstallAutomation::from_environment(&environment).expect("automation");

        let plan = automation.plan("mhw", "mod-a").expect("plan");
        let manifest_status = automation
            .status(None, "default", &["mod-a".to_owned()])
            .expect("manifest status");
        let recovery_status = automation
            .status(Some("mhw"), "default", &["mod-a".to_owned()])
            .expect("recovery-aware status");
        let recovery = automation
            .recovery_scan("mhw", "default", &[])
            .expect("recovery scan");
        let preview = automation
            .recovery_preview(
                "mhw",
                "default",
                "mod-a",
                ReadOnlyInstallRecoveryAction::RollbackInstall,
            )
            .expect("recovery preview");

        assert_eq!(plan.action_count, 1);
        assert_eq!(plan.actions[0].target_path, "nativePC/models/player.mod3");
        assert_eq!(manifest_status.items[0].status, "not_installed");
        assert_eq!(recovery_status.items[0].status, "not_installed");
        assert_eq!(recovery.item_count, 0);
        assert_eq!(preview.availability, "blocked");
        assert_eq!(preview.blocking_reasons[0].code, "rollback_state_missing");
        let serialized =
            serde_json::to_string(&(plan, manifest_status, recovery_status, recovery, preview))
                .expect("serialize snapshots");
        assert!(!serialized.contains(&sandbox.path().to_string_lossy().to_string()));
        assert!(!serialized.contains("package-a"));
        assert_eq!(tree_snapshot(sandbox.path()), before);
        assert!(!sandbox
            .path()
            .join("mod-import")
            .join("results.json.lock")
            .exists());
    }

    #[test]
    fn install_queries_reject_path_like_ids_without_returning_them() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let automation =
            ReadOnlyInstallAutomation::from_environment(&environment).expect("automation");

        assert_eq!(
            automation.status(None, "../profile", &["mod-a".to_owned()]),
            Err(ReadOnlyInstallAutomationError::ProfileIdInvalid)
        );
        assert_eq!(
            automation.plan("mhw", r"C:\private\mod"),
            Err(ReadOnlyInstallAutomationError::ModIdInvalid)
        );
    }

    #[test]
    fn recovery_scan_rejects_unsafe_persisted_mod_ids_before_projection() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let game_root = create_game_fixture(sandbox.path());
        write_game_config(sandbox.path(), &game_root);
        write_manifest_with_mod_id(sandbox.path(), "../private-mod");
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let automation =
            ReadOnlyInstallAutomation::from_environment(&environment).expect("automation");

        assert_eq!(
            automation.recovery_scan("mhw", "default", &[]),
            Err(ReadOnlyInstallAutomationError::InstallStateInvalid)
        );
    }

    #[test]
    fn install_plan_projection_rejects_control_characters_in_targets() {
        let target = InstallTargetPath::parse("nativePC/models/unsafe\nfield.mod3", ["nativePC"])
            .expect("domain target");

        assert_eq!(
            project_install_target_path(&target),
            Err(ReadOnlyInstallAutomationError::InstallPlanInvalid)
        );
    }

    #[test]
    fn sandbox_recovery_rejects_game_root_outside_fixture_tree() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("MonsterHunterWorld.exe"), b"fixture")
            .expect("write outside game exe");
        write_game_config(sandbox.path(), outside.path());
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let automation =
            ReadOnlyInstallAutomation::from_environment(&environment).expect("automation");

        assert_eq!(
            automation.recovery_scan("mhw", "default", &[]),
            Err(ReadOnlyInstallAutomationError::SandboxGamePathRejected)
        );
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn sandbox_plan_rejects_package_root_link_outside_controlled_sandboxes() {
        let sandbox = tempfile::tempdir().expect("sandbox");
        let outside = tempfile::tempdir().expect("outside");
        write_v1_mod_catalog_and_sandbox(sandbox.path());
        let package_root = sandbox
            .path()
            .join("mod-import")
            .join("sandboxes")
            .join("package-a");
        fs::remove_dir_all(&package_root).expect("remove package fixture");
        fs::create_dir_all(outside.path().join("nativePC").join("models"))
            .expect("create outside package");
        fs::write(
            outside
                .path()
                .join("nativePC")
                .join("models")
                .join("outside.mod3"),
            b"outside",
        )
        .expect("write outside Mod file");
        create_directory_link(outside.path(), &package_root);
        let environment =
            RuntimeEnvironment::sandbox(sandbox.path().to_path_buf()).expect("environment");
        let automation =
            ReadOnlyInstallAutomation::from_environment(&environment).expect("automation");

        let result = automation.plan("mhw", "mod-a");

        remove_directory_link(&package_root);
        assert_eq!(
            result,
            Err(ReadOnlyInstallAutomationError::ImportedModSandboxUnavailable)
        );
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn read_only_locator_rejects_sandbox_root_link_outside_app_data() {
        let temp = tempfile::tempdir().expect("temp");
        let app_data_root = temp.path().join("app-data");
        let outside = temp.path().join("outside");
        fs::create_dir_all(app_data_root.join("mod-import")).expect("create app data");
        fs::create_dir_all(outside.join("package-a")).expect("create outside package");
        let sandbox_root = app_data_root.join("mod-import").join("sandboxes");
        create_directory_link(&outside, &sandbox_root);
        let locator = ContainedReadOnlyModImportSandboxLocator::new(&app_data_root);

        let result = locator.sandbox_root_for_package("package-a");

        remove_directory_link(&sandbox_root);
        assert!(result.is_err());
    }
}
