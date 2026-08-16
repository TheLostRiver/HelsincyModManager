use crate::install::install_manifest_status_code;
use crate::{
    GamePrerequisiteDecision, GamePrerequisiteDecisionProvider, InstallPlanningError,
    InstallPlanningService,
};
use hmm_core::{
    classify_reinstall_targets, is_same_revision_replacement_target_switch,
    resolve_installed_revision, FileLayer, GameId, InstallFileProvider, InstallManifest,
    InstallManifestEntry, InstallManifestStatusConsumption, InstallManifestValidationError,
    InstallPlan, InstallTargetPath, InstalledFileSummary, ModId, ModRevisionId, PackageFileId,
    ProfileId, ReinstallClassificationError, ReinstallManifestError, ReinstallTargetClass,
    ReinstallTargetState, ReplacementBindingSnapshot,
};
use hmm_ports::{
    InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
    ModImportResultRepository, ReinstallRecoveryTransactionRepository, StoredModRevision,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallPreviewRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub candidate_revision_id: ModRevisionId,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReinstallPreviewStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallRevisionSummary {
    pub revision_id: ModRevisionId,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReinstallTargetCounts {
    pub retained: usize,
    pub replaced: usize,
    pub added: usize,
    pub stale: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReinstallBlockingReason {
    PrerequisitesBlocked,
    NotInstalled,
    CandidateNotFound,
    CandidateNotReady,
    CandidateOwnerMismatch,
    CandidateAlreadyInstalled,
    ManifestStateUnsafe,
    InstalledRevisionUnknown,
    SourceUnavailable,
    TargetMissing,
    TargetChanged,
    TargetReadFailed,
    BackupMissing,
    BackupReadFailed,
    PlanConflict,
    CrossModTargetConflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallBlockingReasonSummary {
    pub reason: ReinstallBlockingReason,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallPlanPreview {
    pub status: ReinstallPreviewStatus,
    pub prerequisite_decision: GamePrerequisiteDecision,
    pub installed_revision: Option<ReinstallRevisionSummary>,
    pub candidate_revision: Option<ReinstallRevisionSummary>,
    pub counts: ReinstallTargetCounts,
    pub blocking_reasons: Vec<ReinstallBlockingReasonSummary>,
    pub plan_token: Option<String>,
}

impl ReinstallPlanPreview {
    fn blocked_preview(
        installed_revision: Option<ModRevisionId>,
        candidate_revision: Option<ModRevisionId>,
        reason: ReinstallBlockingReason,
        prerequisite_decision: GamePrerequisiteDecision,
    ) -> Self {
        Self {
            status: ReinstallPreviewStatus::Blocked,
            prerequisite_decision,
            installed_revision: installed_revision.map(revision_summary),
            candidate_revision: candidate_revision.map(revision_summary),
            counts: ReinstallTargetCounts::default(),
            blocking_reasons: vec![ReinstallBlockingReasonSummary { reason, count: 1 }],
            plan_token: None,
        }
    }

    fn blocked(
        installed_revision: Option<ModRevisionId>,
        candidate_revision: Option<ModRevisionId>,
        reason: ReinstallBlockingReason,
        prerequisite_decision: GamePrerequisiteDecision,
    ) -> ReinstallPreparation {
        ReinstallPreparation::Blocked(Self::blocked_preview(
            installed_revision,
            candidate_revision,
            reason,
            prerequisite_decision,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledReplacementReinstallContext {
    pub installed_revision_id: ModRevisionId,
    pub installed_binding: ReplacementBindingSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstalledReplacementReinstallResolution {
    Ready(Box<InstalledReplacementReinstallContext>),
    Blocked(ReinstallPlanPreview),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReinstallCandidatePlanError {
    #[error("candidate revision is not ready")]
    NotReady,
    #[error("candidate install plan is unavailable")]
    Unavailable,
}

pub struct ReinstallCandidatePlanRequest<'a> {
    pub game_id: &'a GameId,
    pub profile_id: &'a ProfileId,
    pub mod_id: &'a ModId,
    pub candidate: &'a StoredModRevision,
    pub layer: &'a FileLayer,
}

pub trait ReinstallCandidatePlanner: Send + Sync {
    fn build_candidate_plan(
        &self,
        request: ReinstallCandidatePlanRequest<'_>,
    ) -> Result<InstallPlan, ReinstallCandidatePlanError>;
}

impl ReinstallCandidatePlanner for InstallPlanningService {
    fn build_candidate_plan(
        &self,
        request: ReinstallCandidatePlanRequest<'_>,
    ) -> Result<InstallPlan, ReinstallCandidatePlanError> {
        self.build_plan_from_imported_revision(
            request.game_id,
            request.mod_id,
            &request.candidate.package_id,
            request.layer,
        )
        .map_err(|error| match error {
            InstallPlanningError::ImportedModSourcesUnavailable
            | InstallPlanningError::GameAdapterNotFound { .. } => {
                ReinstallCandidatePlanError::Unavailable
            }
            InstallPlanningError::InvalidTargetPath { .. }
            | InstallPlanningError::ImportedModNotFound { .. }
            | InstallPlanningError::ImportedModAnalysisUnavailable
            | InstallPlanningError::ImportedModSandboxUnavailable
            | InstallPlanningError::ImportedModFileScanUnavailable => {
                ReinstallCandidatePlanError::NotReady
            }
        })
    }
}

pub trait ReinstallCandidateSourceReader: Send + Sync {
    fn read_candidate_source_file(
        &self,
        candidate: &StoredModRevision,
        package_file_id: &PackageFileId,
    ) -> anyhow::Result<Vec<u8>>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReinstallPreviewError {
    #[error("reinstall catalog is unavailable")]
    CatalogUnavailable,
    #[error("reinstall manifest is unavailable")]
    ManifestUnavailable,
    #[error("reinstall recovery state is unavailable")]
    RecoveryUnavailable,
    #[error("candidate install plan is unavailable")]
    CandidatePlanUnavailable,
}

#[derive(Clone)]
pub struct ReinstallPreviewService {
    prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
    catalog: Arc<dyn ModImportResultRepository>,
    planner: Arc<dyn ReinstallCandidatePlanner>,
    source: Arc<dyn ReinstallCandidateSourceReader>,
    game: Arc<dyn InstallGameFileSystem>,
    backups: Arc<dyn InstallBackupStore>,
    manifests: Arc<dyn InstallManifestRepository>,
    recovery: Arc<dyn ReinstallRecoveryTransactionRepository>,
}

impl ReinstallPreviewService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
        catalog: Arc<dyn ModImportResultRepository>,
        planner: Arc<dyn ReinstallCandidatePlanner>,
        source: Arc<dyn ReinstallCandidateSourceReader>,
        game: Arc<dyn InstallGameFileSystem>,
        backups: Arc<dyn InstallBackupStore>,
        manifests: Arc<dyn InstallManifestRepository>,
        recovery: Arc<dyn ReinstallRecoveryTransactionRepository>,
    ) -> Self {
        Self {
            prerequisites,
            catalog,
            planner,
            source,
            game,
            backups,
            manifests,
            recovery,
        }
    }

    pub fn preview(
        &self,
        request: ReinstallPreviewRequest,
    ) -> Result<ReinstallPlanPreview, ReinstallPreviewError> {
        self.prepare(request)
            .map(ReinstallPreparation::into_preview)
    }

    pub fn prerequisite_decision(&self, game_id: &GameId) -> GamePrerequisiteDecision {
        self.prerequisites.prerequisite_decision(game_id)
    }

    pub fn resolve_installed_replacement_context(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<InstalledReplacementReinstallResolution, ReinstallPreviewError> {
        let prerequisite_decision = self.prerequisite_decision(game_id);
        let blocked_preview = |installed_revision, candidate_revision, reason| {
            ReinstallPlanPreview::blocked_preview(
                installed_revision,
                candidate_revision,
                reason,
                prerequisite_decision.clone(),
            )
        };
        if prerequisite_decision.is_blocked() {
            return Ok(InstalledReplacementReinstallResolution::Blocked(
                blocked_preview(None, None, ReinstallBlockingReason::PrerequisitesBlocked),
            ));
        }
        let logical_mod = self
            .catalog
            .get_mod(mod_id)
            .map_err(|_| ReinstallPreviewError::CatalogUnavailable)?;
        let manifest = self
            .manifests
            .load_manifest(profile_id)
            .map_err(|_| ReinstallPreviewError::ManifestUnavailable)?;
        let Some(manifest) = manifest else {
            return Ok(InstalledReplacementReinstallResolution::Blocked(
                blocked_preview(None, None, ReinstallBlockingReason::NotInstalled),
            ));
        };
        if manifest.profile_id != *profile_id {
            return Ok(InstalledReplacementReinstallResolution::Blocked(
                blocked_preview(None, None, ReinstallBlockingReason::ManifestStateUnsafe),
            ));
        }
        if let Err(error) = manifest.validate() {
            return Ok(InstalledReplacementReinstallResolution::Blocked(
                blocked_preview(None, None, manifest_validation_blocking_reason(error)),
            ));
        }
        let active_recovery = self
            .recovery
            .list_transactions(profile_id)
            .map_err(|_| ReinstallPreviewError::RecoveryUnavailable)?;
        if !active_recovery.is_empty()
            || manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries
        {
            return Ok(InstalledReplacementReinstallResolution::Blocked(
                blocked_preview(None, None, ReinstallBlockingReason::ManifestStateUnsafe),
            ));
        }
        let legacy_provenance = logical_mod
            .as_ref()
            .filter(|logical_mod| logical_mod.mod_id == *mod_id)
            .map(|logical_mod| vec![logical_mod.origin_revision_id.clone()])
            .unwrap_or_default();
        let installed_revision_id =
            match resolve_installed_revision(&manifest, mod_id, &legacy_provenance) {
                Ok(revision_id) => revision_id,
                Err(ReinstallManifestError::ModNotInstalled) => {
                    return Ok(InstalledReplacementReinstallResolution::Blocked(
                        blocked_preview(None, None, ReinstallBlockingReason::NotInstalled),
                    ));
                }
                Err(_) => {
                    return Ok(InstalledReplacementReinstallResolution::Blocked(
                        blocked_preview(
                            None,
                            None,
                            ReinstallBlockingReason::InstalledRevisionUnknown,
                        ),
                    ));
                }
            };
        let installed_revision = self
            .catalog
            .get_revision(&installed_revision_id)
            .map_err(|_| ReinstallPreviewError::CatalogUnavailable)?;
        if installed_revision
            .as_ref()
            .is_none_or(|revision| revision.mod_id != *mod_id)
        {
            return Ok(InstalledReplacementReinstallResolution::Blocked(
                blocked_preview(
                    Some(installed_revision_id),
                    None,
                    ReinstallBlockingReason::InstalledRevisionUnknown,
                ),
            ));
        }
        let mut bindings = manifest
            .replacement_bindings
            .iter()
            .filter(|snapshot| snapshot.mod_id() == mod_id);
        let (Some(installed_binding), None) = (bindings.next(), bindings.next()) else {
            return Ok(InstalledReplacementReinstallResolution::Blocked(
                blocked_preview(
                    Some(installed_revision_id.clone()),
                    Some(installed_revision_id),
                    ReinstallBlockingReason::CandidateNotReady,
                ),
            ));
        };

        Ok(InstalledReplacementReinstallResolution::Ready(Box::new(
            InstalledReplacementReinstallContext {
                installed_revision_id,
                installed_binding: installed_binding.clone(),
            },
        )))
    }

    pub(crate) fn prepare(
        &self,
        request: ReinstallPreviewRequest,
    ) -> Result<ReinstallPreparation, ReinstallPreviewError> {
        self.prepare_with_candidate_plan(request, None, false)
    }

    pub fn prepare_replacement_target_switch(
        &self,
        request: ReinstallPreviewRequest,
        candidate_plan: InstallPlan,
    ) -> Result<ReinstallPreparation, ReinstallPreviewError> {
        self.prepare_with_candidate_plan(request, Some(candidate_plan), true)
    }

    fn prepare_with_candidate_plan(
        &self,
        request: ReinstallPreviewRequest,
        candidate_plan: Option<InstallPlan>,
        allow_same_revision_target_switch: bool,
    ) -> Result<ReinstallPreparation, ReinstallPreviewError> {
        let prerequisite_decision = self.prerequisite_decision(&request.game_id);
        let blocked = |installed_revision, candidate_revision, reason| {
            ReinstallPlanPreview::blocked(
                installed_revision,
                candidate_revision,
                reason,
                prerequisite_decision.clone(),
            )
        };
        if prerequisite_decision.is_blocked() {
            return Ok(blocked(
                None,
                None,
                ReinstallBlockingReason::PrerequisitesBlocked,
            ));
        }
        let candidate = self
            .catalog
            .get_revision(&request.candidate_revision_id)
            .map_err(|_| ReinstallPreviewError::CatalogUnavailable)?;
        let Some(candidate) = candidate else {
            return Ok(blocked(
                None,
                None,
                ReinstallBlockingReason::CandidateNotFound,
            ));
        };
        if candidate.revision_id != request.candidate_revision_id {
            return Err(ReinstallPreviewError::CatalogUnavailable);
        }
        if candidate.mod_id != request.mod_id {
            return Ok(blocked(
                None,
                Some(candidate.revision_id),
                ReinstallBlockingReason::CandidateOwnerMismatch,
            ));
        }
        let candidate_revision_id = candidate.revision_id.clone();

        let logical_mod = self
            .catalog
            .get_mod(&request.mod_id)
            .map_err(|_| ReinstallPreviewError::CatalogUnavailable)?;
        let manifest = self
            .manifests
            .load_manifest(&request.profile_id)
            .map_err(|_| ReinstallPreviewError::ManifestUnavailable)?;
        let Some(manifest) = manifest else {
            return Ok(blocked(
                None,
                Some(candidate_revision_id),
                ReinstallBlockingReason::NotInstalled,
            ));
        };

        if manifest.profile_id != request.profile_id {
            return Ok(blocked(
                None,
                Some(candidate_revision_id),
                ReinstallBlockingReason::ManifestStateUnsafe,
            ));
        }
        if let Err(error) = manifest.validate() {
            let reason = match error {
                InstallManifestValidationError::UnsupportedSchemaVersion { .. } => {
                    ReinstallBlockingReason::ManifestStateUnsafe
                }
                InstallManifestValidationError::RevisionedEntriesRequireSchemaV2
                | InstallManifestValidationError::MixedRevisionSet { .. }
                | InstallManifestValidationError::MultipleRevisionSet { .. } => {
                    ReinstallBlockingReason::InstalledRevisionUnknown
                }
                InstallManifestValidationError::DuplicateReplacementBindingId
                | InstallManifestValidationError::DuplicateReplacementBindingMod
                | InstallManifestValidationError::ReplacementBindingProfileMismatch
                | InstallManifestValidationError::ReplacementBindingOwnerMissing
                | InstallManifestValidationError::ReplacementBindingRevisionMismatch => {
                    ReinstallBlockingReason::ManifestStateUnsafe
                }
            };
            return Ok(blocked(None, Some(candidate_revision_id), reason));
        }

        let active_recovery = self
            .recovery
            .list_transactions(&request.profile_id)
            .map_err(|_| ReinstallPreviewError::RecoveryUnavailable)?;
        if !active_recovery.is_empty()
            || manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries
        {
            return Ok(blocked(
                None,
                Some(candidate_revision_id),
                ReinstallBlockingReason::ManifestStateUnsafe,
            ));
        }

        let legacy_provenance = logical_mod
            .as_ref()
            .filter(|logical_mod| logical_mod.mod_id == request.mod_id)
            .map(|logical_mod| vec![logical_mod.origin_revision_id.clone()])
            .unwrap_or_default();
        let installed_revision_id =
            match resolve_installed_revision(&manifest, &request.mod_id, &legacy_provenance) {
                Ok(revision_id) => revision_id,
                Err(ReinstallManifestError::ModNotInstalled) => {
                    return Ok(blocked(
                        None,
                        Some(candidate_revision_id),
                        ReinstallBlockingReason::NotInstalled,
                    ));
                }
                Err(_) => {
                    return Ok(blocked(
                        None,
                        Some(candidate_revision_id),
                        ReinstallBlockingReason::InstalledRevisionUnknown,
                    ));
                }
            };
        let installed_revision = self
            .catalog
            .get_revision(&installed_revision_id)
            .map_err(|_| ReinstallPreviewError::CatalogUnavailable)?;
        if installed_revision
            .as_ref()
            .is_none_or(|revision| revision.mod_id != request.mod_id)
        {
            return Ok(blocked(
                None,
                Some(candidate_revision_id),
                ReinstallBlockingReason::InstalledRevisionUnknown,
            ));
        }
        if installed_revision_id == candidate_revision_id && !allow_same_revision_target_switch {
            return Ok(blocked(
                Some(installed_revision_id),
                Some(candidate_revision_id),
                ReinstallBlockingReason::CandidateAlreadyInstalled,
            ));
        }

        let plan = match candidate_plan {
            Some(plan) => plan,
            None => match self
                .planner
                .build_candidate_plan(ReinstallCandidatePlanRequest {
                    game_id: &request.game_id,
                    profile_id: &request.profile_id,
                    mod_id: &request.mod_id,
                    candidate: &candidate,
                    layer: &request.layer,
                }) {
                Ok(plan) => plan,
                Err(ReinstallCandidatePlanError::NotReady) => {
                    return Ok(blocked(
                        Some(installed_revision_id),
                        Some(candidate_revision_id),
                        ReinstallBlockingReason::CandidateNotReady,
                    ));
                }
                Err(ReinstallCandidatePlanError::Unavailable) => {
                    return Err(ReinstallPreviewError::CandidatePlanUnavailable);
                }
            },
        };

        let candidate_summary = Some(candidate_revision_id.clone());
        let installed_summary = Some(installed_revision_id.clone());
        if plan.has_blocking_conflicts() {
            return Ok(blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::PlanConflict,
            ));
        }
        if plan.actions.is_empty() {
            return Ok(blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::CandidateNotReady,
            ));
        }
        if plan
            .validate_replacement_bindings_for_profile_and_revision(
                &request.profile_id,
                Some(&candidate_revision_id),
            )
            .is_err()
            || plan
                .replacement_bindings
                .iter()
                .any(|snapshot| snapshot.mod_id() != &request.mod_id)
        {
            return Ok(blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::CandidateNotReady,
            ));
        }
        if allow_same_revision_target_switch && installed_revision_id != candidate_revision_id {
            return Ok(blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::CandidateNotReady,
            ));
        }
        if installed_revision_id == candidate_revision_id
            && !is_same_revision_replacement_target_switch(
                &manifest,
                &request.mod_id,
                &candidate_revision_id,
                &plan.replacement_bindings,
            )
        {
            return Ok(blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::CandidateAlreadyInstalled,
            ));
        }
        if plan.actions.iter().any(|action| {
            action.provider.mod_id != request.mod_id
                || action.provider.target_path != action.target_path
        }) {
            return Ok(blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::CrossModTargetConflict,
            ));
        }

        let protected_targets = manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == request.mod_id)
            .map(|entry| entry.target_path.clone())
            .chain(plan.actions.iter().map(|action| action.target_path.clone()))
            .collect::<BTreeSet<_>>();
        if manifest.entries.iter().any(|entry| {
            entry.mod_id != request.mod_id && protected_targets.contains(&entry.target_path)
        }) {
            return Ok(blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::CrossModTargetConflict,
            ));
        }

        let (candidate_states, source_facts) = match self.preload_candidate(&candidate, &plan) {
            Ok(result) => result,
            Err(reason) => {
                return Ok(blocked(installed_summary, candidate_summary, reason));
            }
        };
        let (installed_states, target_facts, backup_facts) = match self.preflight_installed(
            &request.mod_id,
            &installed_revision_id,
            &manifest,
            &candidate_states,
        ) {
            Ok(result) => result,
            Err(reason) => {
                return Ok(blocked(installed_summary, candidate_summary, reason));
            }
        };

        let classifications =
            match classify_reinstall_targets(&request.mod_id, installed_states, candidate_states) {
                Ok(classifications) => classifications,
                Err(error) => {
                    let reason = match error {
                        ReinstallClassificationError::CrossModTargetOwnership { .. } => {
                            ReinstallBlockingReason::CrossModTargetConflict
                        }
                        _ => ReinstallBlockingReason::PlanConflict,
                    };
                    return Ok(blocked(installed_summary, candidate_summary, reason));
                }
            };

        let mut counts = ReinstallTargetCounts::default();
        for classification in &classifications {
            match classification.class {
                ReinstallTargetClass::Retained => counts.retained += 1,
                ReinstallTargetClass::Replaced => counts.replaced += 1,
                ReinstallTargetClass::Added => counts.added += 1,
                ReinstallTargetClass::Stale => counts.stale += 1,
            }
        }

        let plan_token = canonical_plan_token(
            &request,
            &manifest,
            &installed_revision_id,
            &prerequisite_decision,
            CandidatePlanTokenFacts {
                revision: &candidate,
                source_files: &source_facts,
                target_files: &target_facts,
                backup_files: &backup_facts,
                replacement_bindings: &plan.replacement_bindings,
            },
        );
        let targets = build_prepared_targets(
            &manifest,
            classifications,
            source_facts.clone(),
            target_facts,
            &backup_facts,
        );
        Ok(ReinstallPreparation::Ready(Box::new(PreparedReinstall {
            request,
            candidate,
            installed_revision_id,
            legacy_provenance,
            old_manifest: manifest,
            candidate_replacement_bindings: plan.replacement_bindings,
            source_files: source_facts,
            backup_files: backup_facts,
            targets,
            counts,
            prerequisite_decision,
            plan_hash: plan_token.clone(),
            plan_token,
        })))
    }

    fn preload_candidate(
        &self,
        candidate: &StoredModRevision,
        plan: &InstallPlan,
    ) -> Result<(Vec<ReinstallTargetState>, Vec<PreparedSourceFile>), ReinstallBlockingReason> {
        let mut grouped = BTreeMap::<InstallTargetPath, Vec<PreparedSourceFile>>::new();
        let mut facts = Vec::with_capacity(plan.actions.len());
        for action in &plan.actions {
            let bytes = self
                .source
                .read_candidate_source_file(candidate, &action.provider.package_file_id)
                .map_err(|_| ReinstallBlockingReason::SourceUnavailable)?;
            let summary = summarize(&bytes);
            let source = PreparedSourceFile {
                provider: action.provider.clone(),
                summary,
                bytes: Arc::from(bytes),
            };
            grouped
                .entry(action.target_path.clone())
                .or_default()
                .push(source.clone());
            facts.push(source);
        }

        let states = grouped
            .into_iter()
            .map(|(target_path, providers)| {
                let final_file = providers
                    .iter()
                    .max_by_key(|source| source.provider.layer.priority)
                    .expect("candidate target group is non-empty")
                    .summary
                    .clone();
                ReinstallTargetState::new(
                    target_path,
                    candidate.revision_id.clone(),
                    providers
                        .into_iter()
                        .map(|source| source.provider)
                        .collect(),
                    final_file,
                )
            })
            .collect();
        Ok((states, facts))
    }

    fn preflight_installed(
        &self,
        requested_mod_id: &ModId,
        installed_revision_id: &ModRevisionId,
        manifest: &InstallManifest,
        candidate_states: &[ReinstallTargetState],
    ) -> Result<InstalledPreflight, ReinstallBlockingReason> {
        let mut grouped = BTreeMap::<InstallTargetPath, Vec<&InstallManifestEntry>>::new();
        for entry in manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == *requested_mod_id)
        {
            grouped
                .entry(entry.target_path.clone())
                .or_default()
                .push(entry);
        }

        let mut states = Vec::with_capacity(grouped.len());
        let mut target_facts = BTreeMap::new();
        let mut backup_refs = BTreeSet::new();
        for (target_path, entries) in grouped {
            let winner = entries
                .iter()
                .max_by_key(|entry| entry.layer.priority)
                .expect("installed target group is non-empty");
            let expected = winner
                .installed_file
                .clone()
                .ok_or(ReinstallBlockingReason::ManifestStateUnsafe)?;
            let bytes = self
                .game
                .read_game_file(&target_path)
                .map_err(|_| ReinstallBlockingReason::TargetReadFailed)?
                .ok_or(ReinstallBlockingReason::TargetMissing)?;
            let current = summarize(&bytes);
            if current != expected {
                return Err(ReinstallBlockingReason::TargetChanged);
            }
            target_facts.insert(
                target_path.clone(),
                Some(PreparedFile {
                    bytes: Arc::from(bytes),
                    summary: current,
                }),
            );

            let refs = entries
                .iter()
                .filter_map(|entry| entry.backup_ref.as_deref())
                .collect::<BTreeSet<_>>();
            if refs.len() > 1 || refs.iter().any(|reference| reference.trim().is_empty()) {
                return Err(ReinstallBlockingReason::ManifestStateUnsafe);
            }
            backup_refs.extend(refs.into_iter().map(str::to_owned));

            states.push(ReinstallTargetState::new(
                target_path,
                installed_revision_id.clone(),
                entries
                    .into_iter()
                    .map(|entry| InstallFileProvider {
                        mod_id: entry.mod_id.clone(),
                        package_file_id: entry.package_file_id.clone(),
                        target_path: entry.target_path.clone(),
                        layer: entry.layer.clone(),
                    })
                    .collect(),
                expected,
            ));
        }

        for candidate in candidate_states {
            if target_facts.contains_key(&candidate.target_path) {
                continue;
            }
            let bytes = self
                .game
                .read_game_file(&candidate.target_path)
                .map_err(|_| ReinstallBlockingReason::TargetReadFailed)?;
            target_facts.insert(
                candidate.target_path.clone(),
                bytes.map(|bytes| PreparedFile {
                    summary: summarize(&bytes),
                    bytes: Arc::from(bytes),
                }),
            );
        }

        let mut backup_facts = BTreeMap::new();
        for backup_ref in backup_refs {
            let bytes = self
                .backups
                .read_backup(&backup_ref)
                .map_err(|_| ReinstallBlockingReason::BackupReadFailed)?
                .ok_or(ReinstallBlockingReason::BackupMissing)?;
            backup_facts.insert(
                backup_ref,
                PreparedFile {
                    summary: summarize(&bytes),
                    bytes: Arc::from(bytes),
                },
            );
        }
        Ok((states, target_facts, backup_facts))
    }
}

type InstalledPreflight = (
    Vec<ReinstallTargetState>,
    BTreeMap<InstallTargetPath, Option<PreparedFile>>,
    BTreeMap<String, PreparedFile>,
);

#[derive(Clone)]
pub(crate) struct PreparedSourceFile {
    pub(crate) provider: InstallFileProvider,
    pub(crate) summary: InstalledFileSummary,
    pub(crate) bytes: Arc<[u8]>,
}

#[derive(Clone)]
pub(crate) struct PreparedFile {
    pub(crate) summary: InstalledFileSummary,
    pub(crate) bytes: Arc<[u8]>,
}

#[derive(Clone)]
pub(crate) struct PreparedReinstallTarget {
    pub(crate) target_path: InstallTargetPath,
    pub(crate) class: ReinstallTargetClass,
    pub(crate) pre_file: Option<PreparedFile>,
    pub(crate) candidate_files: Vec<PreparedSourceFile>,
    pub(crate) original_backup_ref: Option<String>,
    pub(crate) original_backup_file: Option<PreparedFile>,
}

#[derive(Clone)]
pub struct PreparedReinstall {
    pub(crate) request: ReinstallPreviewRequest,
    pub(crate) candidate: StoredModRevision,
    pub(crate) installed_revision_id: ModRevisionId,
    pub(crate) legacy_provenance: Vec<ModRevisionId>,
    pub(crate) old_manifest: InstallManifest,
    pub(crate) candidate_replacement_bindings: Vec<ReplacementBindingSnapshot>,
    pub(crate) source_files: Vec<PreparedSourceFile>,
    pub(crate) backup_files: BTreeMap<String, PreparedFile>,
    pub(crate) targets: Vec<PreparedReinstallTarget>,
    pub(crate) counts: ReinstallTargetCounts,
    pub(crate) prerequisite_decision: GamePrerequisiteDecision,
    pub(crate) plan_token: String,
    pub(crate) plan_hash: String,
}

impl PreparedReinstall {
    pub fn plan_token(&self) -> &str {
        &self.plan_token
    }
}

pub enum ReinstallPreparation {
    Ready(Box<PreparedReinstall>),
    Blocked(ReinstallPlanPreview),
}

impl ReinstallPreparation {
    pub fn into_preview(self) -> ReinstallPlanPreview {
        match self {
            Self::Ready(prepared) => ReinstallPlanPreview {
                status: ReinstallPreviewStatus::Ready,
                prerequisite_decision: prepared.prerequisite_decision,
                installed_revision: Some(revision_summary(prepared.installed_revision_id)),
                candidate_revision: Some(revision_summary(prepared.candidate.revision_id)),
                counts: prepared.counts,
                blocking_reasons: Vec::new(),
                plan_token: Some(prepared.plan_token),
            },
            Self::Blocked(preview) => preview,
        }
    }
}

fn build_prepared_targets(
    manifest: &InstallManifest,
    classifications: Vec<hmm_core::ReinstallTargetClassification>,
    source_files: Vec<PreparedSourceFile>,
    mut target_facts: BTreeMap<InstallTargetPath, Option<PreparedFile>>,
    backup_facts: &BTreeMap<String, PreparedFile>,
) -> Vec<PreparedReinstallTarget> {
    let mut sources_by_target = BTreeMap::<InstallTargetPath, Vec<PreparedSourceFile>>::new();
    for source in source_files {
        sources_by_target
            .entry(source.provider.target_path.clone())
            .or_default()
            .push(source);
    }

    let mut targets = classifications
        .into_iter()
        .map(|classification| {
            let original_backup_ref = manifest
                .entries
                .iter()
                .filter(|entry| entry.target_path == classification.target_path)
                .find_map(|entry| entry.backup_ref.clone());
            let original_backup_file = original_backup_ref
                .as_ref()
                .and_then(|reference| backup_facts.get(reference))
                .cloned();
            PreparedReinstallTarget {
                pre_file: target_facts.remove(&classification.target_path).flatten(),
                candidate_files: sources_by_target
                    .remove(&classification.target_path)
                    .unwrap_or_default(),
                original_backup_ref,
                original_backup_file,
                target_path: classification.target_path,
                class: classification.class,
            }
        })
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| left.target_path.cmp(&right.target_path));
    targets
}

fn revision_summary(revision_id: ModRevisionId) -> ReinstallRevisionSummary {
    ReinstallRevisionSummary { revision_id }
}

fn manifest_validation_blocking_reason(
    error: InstallManifestValidationError,
) -> ReinstallBlockingReason {
    match error {
        InstallManifestValidationError::UnsupportedSchemaVersion { .. } => {
            ReinstallBlockingReason::ManifestStateUnsafe
        }
        InstallManifestValidationError::RevisionedEntriesRequireSchemaV2
        | InstallManifestValidationError::MixedRevisionSet { .. }
        | InstallManifestValidationError::MultipleRevisionSet { .. } => {
            ReinstallBlockingReason::InstalledRevisionUnknown
        }
        InstallManifestValidationError::DuplicateReplacementBindingId
        | InstallManifestValidationError::DuplicateReplacementBindingMod
        | InstallManifestValidationError::ReplacementBindingProfileMismatch
        | InstallManifestValidationError::ReplacementBindingOwnerMissing
        | InstallManifestValidationError::ReplacementBindingRevisionMismatch => {
            ReinstallBlockingReason::ManifestStateUnsafe
        }
    }
}

pub(crate) fn summarize(bytes: &[u8]) -> InstalledFileSummary {
    InstalledFileSummary {
        size_bytes: bytes.len() as u64,
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}

struct CandidatePlanTokenFacts<'a> {
    revision: &'a StoredModRevision,
    source_files: &'a [PreparedSourceFile],
    target_files: &'a BTreeMap<InstallTargetPath, Option<PreparedFile>>,
    backup_files: &'a BTreeMap<String, PreparedFile>,
    replacement_bindings: &'a [ReplacementBindingSnapshot],
}

fn canonical_plan_token(
    request: &ReinstallPreviewRequest,
    manifest: &InstallManifest,
    installed_revision_id: &ModRevisionId,
    prerequisite_decision: &GamePrerequisiteDecision,
    candidate: CandidatePlanTokenFacts<'_>,
) -> String {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, "reinstall-preview-v1");
    hash_field(&mut hasher, request.game_id.as_str());
    hash_field(&mut hasher, request.profile_id.as_str());
    hash_field(&mut hasher, request.mod_id.as_str());
    hash_field(&mut hasher, installed_revision_id.as_str());
    hash_field(&mut hasher, candidate.revision.revision_id.as_str());
    hash_field(&mut hasher, candidate.revision.mod_id.as_str());
    hash_field(&mut hasher, &candidate.revision.package_id);
    hash_field(&mut hasher, &request.layer.name);
    hash_i32(&mut hasher, request.layer.priority);
    hash_field(&mut hasher, prerequisite_decision.status.as_str());
    hash_optional_u32(&mut hasher, prerequisite_decision.rules_version);
    let mut prerequisite_codes = prerequisite_decision.codes.iter().collect::<Vec<_>>();
    prerequisite_codes.sort();
    hash_u64(&mut hasher, prerequisite_codes.len() as u64);
    for code in prerequisite_codes {
        hash_field(&mut hasher, code.as_str());
    }
    hash_u64(&mut hasher, manifest.schema_version.into());
    hash_field(&mut hasher, install_manifest_status_code(manifest.status));

    let mut entries = manifest.entries.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.target_path
            .cmp(&right.target_path)
            .then_with(|| left.layer.priority.cmp(&right.layer.priority))
            .then_with(|| left.layer.name.cmp(&right.layer.name))
            .then_with(|| left.mod_id.cmp(&right.mod_id))
            .then_with(|| left.package_file_id.cmp(&right.package_file_id))
    });
    hash_u64(&mut hasher, entries.len() as u64);
    for entry in entries {
        hash_field(&mut hasher, entry.target_path.as_str());
        hash_field(&mut hasher, entry.mod_id.as_str());
        hash_optional(
            &mut hasher,
            entry.revision_id.as_ref().map(ModRevisionId::as_str),
        );
        hash_field(&mut hasher, entry.package_file_id.as_str());
        hash_field(&mut hasher, &entry.layer.name);
        hash_i32(&mut hasher, entry.layer.priority);
        hash_optional(&mut hasher, entry.backup_ref.as_deref());
        hash_optional_summary(&mut hasher, entry.installed_file.as_ref());
    }
    hash_replacement_snapshots(&mut hasher, &manifest.replacement_bindings);

    let mut sources = candidate.source_files.to_vec();
    sources.sort_by(|left, right| {
        left.provider
            .target_path
            .cmp(&right.provider.target_path)
            .then_with(|| {
                left.provider
                    .layer
                    .priority
                    .cmp(&right.provider.layer.priority)
            })
            .then_with(|| {
                left.provider
                    .package_file_id
                    .cmp(&right.provider.package_file_id)
            })
    });
    hash_u64(&mut hasher, sources.len() as u64);
    for source in sources {
        hash_field(&mut hasher, source.provider.target_path.as_str());
        hash_field(&mut hasher, source.provider.mod_id.as_str());
        hash_field(&mut hasher, source.provider.package_file_id.as_str());
        hash_field(&mut hasher, &source.provider.layer.name);
        hash_i32(&mut hasher, source.provider.layer.priority);
        hash_summary(&mut hasher, &source.summary);
    }

    hash_u64(&mut hasher, candidate.target_files.len() as u64);
    for (target, summary) in candidate.target_files {
        hash_field(&mut hasher, target.as_str());
        hash_optional_summary(&mut hasher, summary.as_ref().map(|file| &file.summary));
    }
    hash_u64(&mut hasher, candidate.backup_files.len() as u64);
    for (backup_ref, summary) in candidate.backup_files {
        hash_field(&mut hasher, backup_ref);
        hash_summary(&mut hasher, &summary.summary);
    }
    hash_replacement_snapshots(&mut hasher, candidate.replacement_bindings);

    format!("reinstall-preview-v1:{:x}", hasher.finalize())
}

fn hash_replacement_snapshots(hasher: &mut Sha256, snapshots: &[ReplacementBindingSnapshot]) {
    let mut snapshots = snapshots.iter().collect::<Vec<_>>();
    snapshots.sort_by(|left, right| left.binding_id().cmp(right.binding_id()));
    hash_u64(hasher, snapshots.len() as u64);
    for snapshot in snapshots {
        hash_field(hasher, snapshot.binding_id().as_str());
        hash_field(hasher, snapshot.mod_id().as_str());
        hash_field(hasher, snapshot.profile_id().as_str());
        hash_optional(hasher, snapshot.revision_id().map(ModRevisionId::as_str));
        hash_field(hasher, snapshot.binding().source_id().as_str());
        hash_field(hasher, snapshot.binding().target_id().as_str());
        hash_u128(hasher, snapshot.binding().created_at_unix_millis());
        hash_field(hasher, snapshot.source_internal_id());
        hash_field(hasher, snapshot.target_internal_id());
        hash_field(hasher, snapshot.source_path_family());
        hash_field(hasher, snapshot.target_path_family());
        hash_field(hasher, snapshot.retarget_kind().as_str());
        match snapshot.adapter_facts() {
            Some(facts) => {
                hasher.update([1]);
                hasher.update(b"hmm-replacement-adapter-facts-v1");
                hasher.update(facts.schema_version().to_be_bytes());
                hash_field(hasher, facts.adapter_id());
                hash_field(hasher, facts.strategy_id());
                hasher.update(facts.strategy_version().to_be_bytes());
                hash_field(hasher, facts.source_closure_sha256());
                hash_field(hasher, facts.part_set_sha256());
                hash_field(hasher, facts.transform_set_sha256());
                hasher.update(facts.part_count().to_be_bytes());
                hasher.update(facts.file_count().to_be_bytes());
                hash_u64(hasher, facts.transformer_identities().len() as u64);
                for identity in facts.transformer_identities() {
                    hash_field(hasher, identity.transformer_id());
                    hasher.update(identity.transformer_version().to_be_bytes());
                }
            }
            None => hasher.update([0]),
        }
    }
}

fn hash_field(hasher: &mut Sha256, value: &str) {
    hash_u64(hasher, value.len() as u64);
    hasher.update(value.as_bytes());
}

fn hash_optional(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_field(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn hash_optional_u32(hasher: &mut Sha256, value: Option<u32>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hasher.update(value.to_be_bytes());
        }
        None => hasher.update([0]),
    }
}

fn hash_optional_summary(hasher: &mut Sha256, summary: Option<&InstalledFileSummary>) {
    match summary {
        Some(summary) => {
            hasher.update([1]);
            hash_summary(hasher, summary);
        }
        None => hasher.update([0]),
    }
}

fn hash_summary(hasher: &mut Sha256, summary: &InstalledFileSummary) {
    hash_u64(hasher, summary.size_bytes);
    hash_field(hasher, &summary.sha256);
}

fn hash_i32(hasher: &mut Sha256, value: i32) {
    hasher.update(value.to_be_bytes());
}

fn hash_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_be_bytes());
}

fn hash_u128(hasher: &mut Sha256, value: u128) {
    hasher.update(value.to_be_bytes());
}
