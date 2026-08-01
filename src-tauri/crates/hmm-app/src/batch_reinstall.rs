use crate::batch_install::{events_contain_audit_degradation, ParentTaskObserver};
use crate::reinstall::manifest_status_code;
use crate::reinstall_task::ReinstallTaskOrchestrationError;
use crate::{
    PreparedReinstall, ReinstallBlockingReason, ReinstallPlanPreview, ReinstallPreparation,
    ReinstallPreviewRequest, ReinstallPreviewService, ReinstallTaskExecutor, ReinstallTaskRunner,
    RetargetReinstallTaskExecutor, StartReinstallTaskRequest, StartRetargetReinstallTaskRequest,
    TaskKind, TaskManager, TaskStatus,
};
use hmm_core::{
    BatchActionSummary, BatchItemFacts, BatchItemInput, BatchOperation, BatchPlanFacts,
    BatchPreflightDecision, BatchPreflightStatus, BatchReasonSummary, BatchTargetClaim,
    BatchTargetWriteKind, GameId, InstallManifestStatusConsumption, NormalizedBatchPlanRequest,
    ProfileId, ReinstallBatchItemInput, ReinstallTargetClass, ReplacementBindingSnapshot,
};
use hmm_ports::{
    BatchPlanFactsProvider, InstallManifestRepository, ReinstallRecoveryTransactionRepository,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{BatchInstallItemExecution, BatchInstallItemExecutor, BatchInstallItemRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchReinstallItemFactsRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub input: ReinstallBatchItemInput,
}

/// Supplies one pure, read-only reinstall snapshot. Runtime-specific same-revision retarget
/// planning implements this boundary without exposing game-specific paths to generic batch code.
pub trait BatchReinstallItemFactsReader: Send + Sync {
    fn read_item_facts(
        &self,
        request: &BatchReinstallItemFactsRequest,
    ) -> anyhow::Result<BatchItemFacts>;
}

/// The standard reader covers cross-revision true reinstall directly through the existing single
/// item preview service. Same-revision retarget needs the configured replacement workflow and is
/// therefore supplied by the runtime composition boundary.
pub struct ReinstallPreviewBatchItemFactsReader {
    preview: Arc<ReinstallPreviewService>,
}

impl ReinstallPreviewBatchItemFactsReader {
    pub fn new(preview: Arc<ReinstallPreviewService>) -> Self {
        Self { preview }
    }
}

impl BatchReinstallItemFactsReader for ReinstallPreviewBatchItemFactsReader {
    fn read_item_facts(
        &self,
        request: &BatchReinstallItemFactsRequest,
    ) -> anyhow::Result<BatchItemFacts> {
        anyhow::ensure!(
            request.input.installed_revision_id != request.input.candidate_revision_id,
            "same-revision retarget preview requires configured replacement facts"
        );
        let preparation = self.preview.prepare(ReinstallPreviewRequest {
            game_id: request.game_id.clone(),
            profile_id: request.profile_id.clone(),
            mod_id: request.input.mod_id.clone(),
            candidate_revision_id: request.input.candidate_revision_id.clone(),
            layer: request.input.layer.clone(),
        })?;
        Ok(match preparation {
            ReinstallPreparation::Ready(prepared) => prepared.batch_item_facts(&request.input),
            ReinstallPreparation::Blocked(preview) => blocked_item_facts(&request.input, &preview)?,
        })
    }
}

impl PreparedReinstall {
    /// Stable, Mod-scoped identity used by batch orchestration. It intentionally excludes
    /// unrelated manifest entries and profile-level commit metadata that earlier non-overlapping
    /// batch items are expected to change.
    pub fn batch_plan_digest(&self) -> String {
        let mut manifest_entries = self
            .old_manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == self.request.mod_id)
            .map(canonical_json)
            .collect::<Vec<_>>();
        manifest_entries.sort();
        let mut installed_bindings = self
            .old_manifest
            .replacement_bindings
            .iter()
            .filter(|binding| binding.mod_id() == &self.request.mod_id)
            .map(canonical_json)
            .collect::<Vec<_>>();
        installed_bindings.sort();
        let mut candidate_bindings = self
            .candidate_replacement_bindings
            .iter()
            .map(canonical_json)
            .collect::<Vec<_>>();
        candidate_bindings.sort();
        let mut legacy_provenance = self
            .legacy_provenance
            .iter()
            .map(|revision| revision.as_str().to_owned())
            .collect::<Vec<_>>();
        legacy_provenance.sort();

        let mut sources = self
            .source_files
            .iter()
            .map(|source| {
                serde_json::json!({
                    "provider": canonical_json(&source.provider),
                    "summary": canonical_json(&source.summary),
                })
            })
            .collect::<Vec<_>>();
        sources.sort_by_key(serde_json::Value::to_string);
        let targets = self
            .targets
            .iter()
            .map(|target| {
                let mut candidate_files = target
                    .candidate_files
                    .iter()
                    .map(|source| {
                        serde_json::json!({
                            "provider": canonical_json(&source.provider),
                            "summary": canonical_json(&source.summary),
                        })
                    })
                    .collect::<Vec<_>>();
                candidate_files.sort_by_key(serde_json::Value::to_string);
                serde_json::json!({
                    "target": target.target_path.as_str(),
                    "class": reinstall_target_class_code(target.class),
                    "preFile": target.pre_file.as_ref().map(|file| canonical_json(&file.summary)),
                    "candidateFiles": candidate_files,
                    "originalBackupRef": target.original_backup_ref,
                    "originalBackupFile": target
                        .original_backup_file
                        .as_ref()
                        .map(|file| canonical_json(&file.summary)),
                })
            })
            .collect::<Vec<_>>();
        let backup_files = self
            .backup_files
            .iter()
            .map(|(reference, file)| {
                serde_json::json!({
                    "reference": reference,
                    "summary": canonical_json(&file.summary),
                })
            })
            .collect::<Vec<_>>();
        let mut prerequisite_codes = self
            .prerequisite_decision
            .codes
            .iter()
            .map(|code| code.as_str())
            .collect::<Vec<_>>();
        prerequisite_codes.sort();

        let canonical = serde_json::json!({
            "schema": "hmm-batch-reinstall-item-v1",
            "gameId": self.request.game_id.as_str(),
            "profileId": self.request.profile_id.as_str(),
            "modId": self.request.mod_id.as_str(),
            "installedRevisionId": self.installed_revision_id.as_str(),
            "candidate": {
                "revisionId": self.candidate.revision_id.as_str(),
                "modId": self.candidate.mod_id.as_str(),
                "packageId": self.candidate.package_id,
            },
            "layer": canonical_json(&self.request.layer),
            "manifest": {
                "profileId": self.old_manifest.profile_id.as_str(),
                "schemaVersion": self.old_manifest.schema_version,
                "status": manifest_status_code(&self.old_manifest),
                "entries": manifest_entries,
                "bindings": installed_bindings,
            },
            "legacyProvenance": legacy_provenance,
            "candidateBindings": candidate_bindings,
            "sources": sources,
            "targets": targets,
            "backupFiles": backup_files,
            "prerequisite": {
                "status": self.prerequisite_decision.status.as_str(),
                "rulesVersion": self.prerequisite_decision.rules_version,
                "codes": prerequisite_codes,
            },
        });
        sha256_prefixed(
            &serde_json::to_vec(&canonical)
                .expect("validated reinstall batch facts are serializable"),
        )
    }

    pub(crate) fn batch_item_facts(&self, input: &ReinstallBatchItemInput) -> BatchItemFacts {
        let single_plan_digest = self.batch_plan_digest();
        let actual_bindings = normalized_bindings(&self.candidate_replacement_bindings);
        let expected_bindings = normalized_bindings(
            &input
                .replacement_binding_snapshot
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
        );
        let blocking_reasons = if actual_bindings == expected_bindings {
            Vec::new()
        } else {
            vec!["replacement_binding_changed".to_owned()]
        };
        let prerequisite = batch_prerequisite(&self.prerequisite_decision);
        let warning_codes = if prerequisite.status == BatchPreflightStatus::Warning {
            prerequisite.codes.clone()
        } else {
            Vec::new()
        };
        let target_claims = self
            .targets
            .iter()
            .map(|target| BatchTargetClaim {
                target_path: target.target_path.clone(),
                kind: match target.class {
                    ReinstallTargetClass::Stale if target.original_backup_ref.is_some() => {
                        BatchTargetWriteKind::Restore
                    }
                    ReinstallTargetClass::Stale => BatchTargetWriteKind::Remove,
                    ReinstallTargetClass::Retained
                    | ReinstallTargetClass::Replaced
                    | ReinstallTargetClass::Added => BatchTargetWriteKind::Install,
                },
            })
            .collect::<Vec<_>>();
        let fact_digest = sha256_prefixed(
            serde_json::json!({
                "schema": "hmm-batch-reinstall-facts-v1",
                "singlePlanDigest": single_plan_digest,
                "blockingReasons": blocking_reasons,
                "warningCodes": warning_codes,
            })
            .to_string()
            .as_bytes(),
        );
        BatchItemFacts {
            mod_id: self.request.mod_id.clone(),
            source_revision_id: Some(self.candidate.revision_id.clone()),
            installed_revision_id: Some(self.installed_revision_id.clone()),
            fact_digest,
            single_plan_digest,
            target_claims,
            action_summary: BatchActionSummary {
                actions: self.targets.len(),
                retained: self.counts.retained,
                replaced: self.counts.replaced,
                added: self.counts.added,
                stale: self.counts.stale,
            },
            prerequisite,
            blocking_reasons,
            warning_codes,
        }
    }
}

pub struct BatchReinstallPlanFactsProvider {
    item_facts: Arc<dyn BatchReinstallItemFactsReader>,
    manifests: Arc<dyn InstallManifestRepository>,
    recovery: Arc<dyn ReinstallRecoveryTransactionRepository>,
    environment_digest: String,
}

impl BatchReinstallPlanFactsProvider {
    pub fn new(
        item_facts: Arc<dyn BatchReinstallItemFactsReader>,
        manifests: Arc<dyn InstallManifestRepository>,
        recovery: Arc<dyn ReinstallRecoveryTransactionRepository>,
        environment_digest: impl Into<String>,
    ) -> Self {
        Self {
            item_facts,
            manifests,
            recovery,
            environment_digest: environment_digest.into(),
        }
    }
}

impl BatchPlanFactsProvider for BatchReinstallPlanFactsProvider {
    fn read_batch_plan_facts(
        &self,
        request: &NormalizedBatchPlanRequest,
    ) -> anyhow::Result<BatchPlanFacts> {
        anyhow::ensure!(
            request.operation == BatchOperation::Reinstall,
            "batch operation is not reinstall"
        );
        anyhow::ensure!(
            !self.environment_digest.trim().is_empty(),
            "batch environment digest is empty"
        );

        let manifest = self.manifests.load_manifest(&request.profile_id)?;
        let active_recovery = self.recovery.list_transactions(&request.profile_id)?;
        let mut global_reasons = BTreeMap::<String, usize>::new();
        if manifest.as_ref().is_some_and(|manifest| {
            manifest.profile_id != request.profile_id || manifest.validate().is_err()
        }) {
            global_reasons.insert("batch_global_manifest_invalid".to_owned(), 1);
        }
        if manifest.as_ref().is_some_and(|manifest| {
            manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries
        }) {
            global_reasons.insert("batch_global_manifest_unsafe".to_owned(), 1);
        }
        if !active_recovery.is_empty() {
            global_reasons.insert(
                "batch_global_recovery_active".to_owned(),
                active_recovery.len(),
            );
        }

        let mut items = Vec::with_capacity(request.items.len());
        let mut rules_version = None::<Option<u32>>;
        for item in &request.items {
            let BatchItemInput::Reinstall(input) = item else {
                anyhow::bail!("batch item operation is not reinstall");
            };
            let facts = self
                .item_facts
                .read_item_facts(&BatchReinstallItemFactsRequest {
                    game_id: request.game_id.clone(),
                    profile_id: request.profile_id.clone(),
                    input: input.clone(),
                })?;
            anyhow::ensure!(
                facts.mod_id == input.mod_id
                    && !facts.fact_digest.trim().is_empty()
                    && !facts.single_plan_digest.trim().is_empty(),
                "reinstall item facts do not match the requested Mod"
            );
            match rules_version {
                None => rules_version = Some(facts.prerequisite.rules_version),
                Some(expected) if expected == facts.prerequisite.rules_version => {}
                Some(_) => {
                    anyhow::bail!("reinstall prerequisite rules changed while reading batch facts")
                }
            }
            items.push(facts);
        }

        Ok(BatchPlanFacts {
            environment_digest: self.environment_digest.clone(),
            prerequisite_rules_version: rules_version.flatten(),
            global_blocking_reasons: global_reasons
                .into_iter()
                .map(|(code, count)| BatchReasonSummary { code, count })
                .collect(),
            items,
        })
    }
}

pub struct ReinstallTaskBatchItemExecutor<E>
where
    E: ReinstallTaskExecutor + RetargetReinstallTaskExecutor,
{
    runner: Arc<ReinstallTaskRunner<E>>,
    task_manager: Arc<TaskManager>,
}

impl<E> ReinstallTaskBatchItemExecutor<E>
where
    E: ReinstallTaskExecutor + RetargetReinstallTaskExecutor,
{
    pub fn new(runner: Arc<ReinstallTaskRunner<E>>, task_manager: Arc<TaskManager>) -> Self {
        Self {
            runner,
            task_manager,
        }
    }
}

impl<E> BatchInstallItemExecutor for ReinstallTaskBatchItemExecutor<E>
where
    E: ReinstallTaskExecutor + RetargetReinstallTaskExecutor + 'static,
{
    fn execute(&self, request: BatchInstallItemRequest) -> BatchInstallItemExecution {
        let plan_item = match request
            .plan
            .items
            .iter()
            .find(|item| item.ordinal == request.item.ordinal)
        {
            Some(item) => item,
            None => return operation_mismatch(),
        };
        let BatchItemInput::Reinstall(input) = &plan_item.input_snapshot else {
            return operation_mismatch();
        };
        if !plan_item.is_ready() {
            return BatchInstallItemExecution::Blocked {
                reason_code: plan_item
                    .blocking_reasons
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "batch_reinstall_plan_blocked".to_owned()),
            };
        }
        let same_revision = input.installed_revision_id == input.candidate_revision_id;
        if same_revision && input.replacement_binding_snapshot.is_none() {
            return BatchInstallItemExecution::Blocked {
                reason_code: "batch_retarget_binding_required".to_owned(),
            };
        }

        let child = match self.task_manager.create_task(TaskKind::Install) {
            Ok(task) => task,
            Err(_) => {
                return BatchInstallItemExecution::Failed {
                    reason_code: "task_unavailable".to_owned(),
                    retryable: true,
                    evidence_health_degraded: true,
                };
            }
        };
        let observer = ParentTaskObserver::new(
            Arc::clone(&self.task_manager),
            request.parent_task_id,
            child.task_id.clone(),
        );
        let result = if same_revision {
            let binding = input
                .replacement_binding_snapshot
                .as_ref()
                .expect("same-revision binding was checked above");
            self.runner
                .run_retarget_reinstall_task_for_orchestration_with_observer(
                    &child.task_id,
                    StartRetargetReinstallTaskRequest {
                        game_id: request.plan.game_id.clone(),
                        profile_id: request.plan.profile_id.clone(),
                        mod_id: input.mod_id.clone(),
                        target_id: binding.binding().target_id().clone(),
                        layer: input.layer.clone(),
                        plan_token: String::new(),
                    },
                    &plan_item.single_plan_digest,
                    &observer,
                )
        } else {
            self.runner
                .run_reinstall_task_for_orchestration_with_observer(
                    &child.task_id,
                    StartReinstallTaskRequest {
                        game_id: request.plan.game_id.clone(),
                        profile_id: request.plan.profile_id.clone(),
                        mod_id: input.mod_id.clone(),
                        candidate_revision_id: input.candidate_revision_id.clone(),
                        layer: input.layer.clone(),
                        plan_token: String::new(),
                    },
                    &plan_item.single_plan_digest,
                    &observer,
                )
        };
        match result {
            Ok(ref events)
                if self.task_manager.task_status(&child.task_id) == Some(TaskStatus::Completed) =>
            {
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: events_contain_audit_degradation(events),
                }
            }
            Ok(_) => BatchInstallItemExecution::Cancelled,
            Err(ref error) if error.committed => classify_reinstall_task_failure(error),
            Err(_)
                if self.task_manager.task_status(&child.task_id) == Some(TaskStatus::Cancelled) =>
            {
                BatchInstallItemExecution::Cancelled
            }
            Err(error) => classify_reinstall_task_failure(&error),
        }
    }
}

fn classify_reinstall_task_failure(
    error: &ReinstallTaskOrchestrationError,
) -> BatchInstallItemExecution {
    if error.committed {
        return BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: true,
        };
    }
    match error.commit_error.as_ref() {
        Some(crate::ReinstallCommitError::PreviewStale) => BatchInstallItemExecution::Blocked {
            reason_code: "reinstall_plan_stale".to_owned(),
        },
        Some(crate::ReinstallCommitError::RolledBack { .. }) => BatchInstallItemExecution::Failed {
            reason_code: "reinstall_rollback_succeeded".to_owned(),
            retryable: true,
            evidence_health_degraded: false,
        },
        Some(
            crate::ReinstallCommitError::RollbackRequired { .. }
            | crate::ReinstallCommitError::RepairRequired { .. },
        ) => BatchInstallItemExecution::RecoveryRequired {
            reason_code: "reinstall_recovery_required".to_owned(),
        },
        Some(crate::ReinstallCommitError::Failed { phase }) => match phase {
            crate::ReinstallCommitPhase::Revalidation
            | crate::ReinstallCommitPhase::Snapshot
            | crate::ReinstallCommitPhase::Recovery => BatchInstallItemExecution::Failed {
                reason_code: "reinstall_unavailable".to_owned(),
                retryable: true,
                evidence_health_degraded: false,
            },
            crate::ReinstallCommitPhase::Mutation
            | crate::ReinstallCommitPhase::Manifest
            | crate::ReinstallCommitPhase::Rollback
            | crate::ReinstallCommitPhase::PostCommit
            | crate::ReinstallCommitPhase::Cleanup => BatchInstallItemExecution::RecoveryRequired {
                reason_code: "reinstall_recovery_required".to_owned(),
            },
        },
        Some(
            crate::ReinstallCommitError::PostCommit | crate::ReinstallCommitError::CleanupPending,
        ) => BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: true,
        },
        None => {
            let reason = error
                .events
                .iter()
                .rev()
                .find_map(|event| event.error.as_deref())
                .unwrap_or("install_reinstall_failed:unavailable");
            if reason.ends_with(":preflight") {
                BatchInstallItemExecution::Blocked {
                    reason_code: "reinstall_plan_stale".to_owned(),
                }
            } else {
                BatchInstallItemExecution::Failed {
                    reason_code: "reinstall_unavailable".to_owned(),
                    retryable: true,
                    evidence_health_degraded: events_contain_audit_degradation(&error.events),
                }
            }
        }
    }
}

fn operation_mismatch() -> BatchInstallItemExecution {
    BatchInstallItemExecution::Blocked {
        reason_code: "batch_operation_not_reinstall".to_owned(),
    }
}

fn blocked_item_facts(
    input: &ReinstallBatchItemInput,
    preview: &ReinstallPlanPreview,
) -> anyhow::Result<BatchItemFacts> {
    let prerequisite = batch_prerequisite(&preview.prerequisite_decision);
    let mut blocking_reasons = preview
        .blocking_reasons
        .iter()
        .map(|summary| reinstall_blocking_code(summary.reason).to_owned())
        .collect::<Vec<_>>();
    blocking_reasons.sort();
    blocking_reasons.dedup();
    let warning_codes = if prerequisite.status == BatchPreflightStatus::Warning {
        prerequisite.codes.clone()
    } else {
        Vec::new()
    };
    let canonical = serde_json::json!({
        "schema": "hmm-batch-reinstall-blocked-v1",
        "input": input,
        "installedRevisionId": preview
            .installed_revision
            .as_ref()
            .map(|revision| revision.revision_id.as_str()),
        "candidateRevisionId": preview
            .candidate_revision
            .as_ref()
            .map(|revision| revision.revision_id.as_str()),
        "counts": {
            "retained": preview.counts.retained,
            "replaced": preview.counts.replaced,
            "added": preview.counts.added,
            "stale": preview.counts.stale,
        },
        "prerequisite": prerequisite,
        "blockingReasons": blocking_reasons,
        "warningCodes": warning_codes,
    });
    let single_plan_digest = sha256_prefixed(&serde_json::to_vec(&canonical)?);
    let fact_digest = sha256_prefixed(
        format!("hmm-batch-reinstall-blocked-facts-v1\0{single_plan_digest}").as_bytes(),
    );
    Ok(BatchItemFacts {
        mod_id: input.mod_id.clone(),
        source_revision_id: preview
            .candidate_revision
            .as_ref()
            .map(|revision| revision.revision_id.clone()),
        installed_revision_id: preview
            .installed_revision
            .as_ref()
            .map(|revision| revision.revision_id.clone()),
        fact_digest,
        single_plan_digest,
        target_claims: Vec::new(),
        action_summary: BatchActionSummary {
            actions: preview.counts.retained
                + preview.counts.replaced
                + preview.counts.added
                + preview.counts.stale,
            retained: preview.counts.retained,
            replaced: preview.counts.replaced,
            added: preview.counts.added,
            stale: preview.counts.stale,
        },
        prerequisite,
        blocking_reasons,
        warning_codes,
    })
}

fn batch_prerequisite(decision: &crate::GamePrerequisiteDecision) -> BatchPreflightDecision {
    let mut codes = decision
        .codes
        .iter()
        .map(|code| code.as_str().to_owned())
        .collect::<Vec<_>>();
    codes.sort();
    codes.dedup();
    BatchPreflightDecision {
        status: match decision.status {
            crate::GamePrerequisiteDecisionStatus::Ready => BatchPreflightStatus::Ready,
            crate::GamePrerequisiteDecisionStatus::Warning => BatchPreflightStatus::Warning,
            crate::GamePrerequisiteDecisionStatus::Blocked => BatchPreflightStatus::Blocked,
        },
        rules_version: decision.rules_version,
        codes,
    }
}

fn reinstall_target_class_code(class: ReinstallTargetClass) -> &'static str {
    match class {
        ReinstallTargetClass::Retained => "retained",
        ReinstallTargetClass::Replaced => "replaced",
        ReinstallTargetClass::Added => "added",
        ReinstallTargetClass::Stale => "stale",
    }
}

fn normalized_bindings(bindings: &[ReplacementBindingSnapshot]) -> Vec<String> {
    let mut bindings = bindings.iter().map(canonical_json).collect::<Vec<_>>();
    bindings.sort();
    bindings
}

fn canonical_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("validated reinstall fact is serializable")
}

fn reinstall_blocking_code(reason: ReinstallBlockingReason) -> &'static str {
    match reason {
        ReinstallBlockingReason::PrerequisitesBlocked => "reinstall_prerequisites_blocked",
        ReinstallBlockingReason::NotInstalled => "mod_not_installed",
        ReinstallBlockingReason::CandidateNotFound => "reinstall_candidate_not_found",
        ReinstallBlockingReason::CandidateNotReady => "reinstall_candidate_not_ready",
        ReinstallBlockingReason::CandidateOwnerMismatch => "reinstall_candidate_owner_mismatch",
        ReinstallBlockingReason::CandidateAlreadyInstalled => {
            "reinstall_candidate_already_installed"
        }
        ReinstallBlockingReason::ManifestStateUnsafe => "reinstall_manifest_state_unsafe",
        ReinstallBlockingReason::InstalledRevisionUnknown => "installed_revision_unknown",
        ReinstallBlockingReason::SourceUnavailable => "reinstall_source_unavailable",
        ReinstallBlockingReason::TargetMissing => "installed_target_missing",
        ReinstallBlockingReason::TargetChanged => "installed_target_changed",
        ReinstallBlockingReason::TargetReadFailed => "installed_target_unavailable",
        ReinstallBlockingReason::BackupMissing => "install_backup_missing",
        ReinstallBlockingReason::BackupReadFailed => "install_backup_unavailable",
        ReinstallBlockingReason::PlanConflict => "reinstall_plan_conflict",
        ReinstallBlockingReason::CrossModTargetConflict => "install_target_owned_by_other_mod",
    }
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[cfg(test)]
#[path = "batch_reinstall_tests.rs"]
mod tests;
