use crate::batch_install::{
    events_contain_audit_degradation, BatchInstallItemExecution, BatchInstallItemExecutor,
    BatchInstallItemRequest, ParentTaskObserver,
};
use crate::install::{install_manifest_status_code, uninstall_manifest_snapshot_digest};
use crate::{
    InstallRecoveryIssue, InstallRecoveryScanRequest, InstallRecoveryScanService,
    InstallRecoveryStatus, InstallRecoverySummary, StartUninstallTaskRequest, TaskKind,
    TaskManager, TaskStatus, UninstallModError, UninstallTaskRunError, UninstallTaskRunner,
};
use hmm_core::{
    BatchActionSummary, BatchItemFacts, BatchItemInput, BatchPlanFacts, BatchPreflightDecision,
    BatchPreflightStatus, BatchReasonSummary, BatchTargetClaim, BatchTargetWriteKind,
    InstallManifest, InstallManifestEntry, InstallManifestStatusConsumption, ModId, ModRevisionId,
    NormalizedBatchPlanRequest, INSTALL_MANIFEST_SCHEMA_VERSION_V2,
};
use hmm_ports::{BatchPlanFactsProvider, InstallManifestRepository};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone)]
pub struct BatchUninstallPlanFactsProvider {
    manifest_repository: Arc<dyn InstallManifestRepository>,
    recovery_scan_service: InstallRecoveryScanService,
    environment_digest: String,
}

impl BatchUninstallPlanFactsProvider {
    pub fn new(
        manifest_repository: Arc<dyn InstallManifestRepository>,
        recovery_scan_service: InstallRecoveryScanService,
        environment_digest: impl Into<String>,
    ) -> Self {
        Self {
            manifest_repository,
            recovery_scan_service,
            environment_digest: environment_digest.into(),
        }
    }
}

impl BatchPlanFactsProvider for BatchUninstallPlanFactsProvider {
    fn read_batch_plan_facts(
        &self,
        request: &NormalizedBatchPlanRequest,
    ) -> anyhow::Result<BatchPlanFacts> {
        anyhow::ensure!(
            request.operation == hmm_core::BatchOperation::Uninstall,
            "batch operation is not uninstall"
        );
        anyhow::ensure!(
            !self.environment_digest.trim().is_empty(),
            "batch environment digest is empty"
        );

        let selected_mod_ids = request
            .items
            .iter()
            .map(|input| match input {
                BatchItemInput::Uninstall(input) => Ok(input.mod_id.clone()),
                _ => anyhow::bail!("batch item operation is not uninstall"),
            })
            .collect::<anyhow::Result<BTreeSet<_>>>()?;
        let manifest = self
            .manifest_repository
            .load_manifest(&request.profile_id)?;
        let recovery_summaries = self
            .recovery_scan_service
            .scan(InstallRecoveryScanRequest {
                profile_id: request.profile_id.clone(),
                mod_ids: Vec::new(),
            })?;
        let summaries_by_mod = recovery_summaries
            .iter()
            .cloned()
            .map(|summary| (summary.mod_id.clone(), summary))
            .collect::<BTreeMap<_, _>>();

        let target_owners = target_owners(manifest.as_ref());
        let backup_owners = backup_owners(manifest.as_ref());
        let mut global_reason_counts = BTreeMap::new();
        if manifest.as_ref().is_some_and(|manifest| {
            manifest.profile_id != request.profile_id || manifest.validate().is_err()
        }) {
            global_reason_counts.insert("batch_global_manifest_invalid".to_owned(), 1);
        }
        let shared_backup_count = backup_owners
            .values()
            .filter(|owners| {
                owners
                    .iter()
                    .filter(|owner| selected_mod_ids.contains(*owner))
                    .count()
                    > 1
            })
            .count();
        if shared_backup_count > 0 {
            global_reason_counts.insert(
                "batch_global_backup_conflict".to_owned(),
                shared_backup_count,
            );
        }

        let mut active_recovery_count = recovery_summaries
            .iter()
            .filter(|summary| recovery_is_globally_active(summary))
            .count();
        if manifest.as_ref().is_some_and(|manifest| {
            manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries
        }) {
            active_recovery_count = active_recovery_count.max(1);
        }
        if active_recovery_count > 0 {
            global_reason_counts.insert(
                "batch_global_recovery_active".to_owned(),
                active_recovery_count,
            );
        }

        let mut items = Vec::with_capacity(request.items.len());
        for input in &request.items {
            let BatchItemInput::Uninstall(input) = input else {
                anyhow::bail!("batch item operation is not uninstall");
            };
            let mut entries = manifest
                .as_ref()
                .map(|manifest| {
                    manifest
                        .entries
                        .iter()
                        .filter(|entry| entry.mod_id == input.mod_id)
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            entries.sort_by(|left, right| {
                left.target_path
                    .as_str()
                    .cmp(right.target_path.as_str())
                    .then_with(|| {
                        left.package_file_id
                            .as_str()
                            .cmp(right.package_file_id.as_str())
                    })
            });
            let summary = summaries_by_mod.get(&input.mod_id);
            let mut blocking_reasons = uninstall_blocking_reasons(summary, &entries);

            if manifest.as_ref().is_some_and(|manifest| {
                manifest.schema_version != INSTALL_MANIFEST_SCHEMA_VERSION_V2
            }) {
                blocking_reasons.push("install_manifest_legacy".to_owned());
            }
            if has_duplicate_target(&entries) {
                blocking_reasons.push("install_manifest_target_duplicate".to_owned());
            }
            if has_duplicate_backup(&entries) {
                blocking_reasons.push("install_backup_ownership_invalid".to_owned());
            }
            if entries.iter().any(|entry| {
                target_owners.get(&target_key(entry)).is_some_and(|owners| {
                    owners.iter().any(|owner| !selected_mod_ids.contains(owner))
                })
            }) {
                blocking_reasons.push("install_target_owned_by_other_mod".to_owned());
            }
            if entries.iter().any(|entry| {
                entry.backup_ref.as_ref().is_some_and(|backup_ref| {
                    backup_owners.get(backup_ref).is_some_and(|owners| {
                        owners.iter().any(|owner| !selected_mod_ids.contains(owner))
                    })
                })
            }) {
                blocking_reasons.push("install_backup_owned_by_other_mod".to_owned());
            }
            blocking_reasons.sort();
            blocking_reasons.dedup();

            let installed_revision_id = exact_installed_revision(
                manifest.as_ref().map(|manifest| manifest.schema_version),
                &entries,
            );
            let mut target_claims = entries
                .iter()
                .map(|entry| BatchTargetClaim {
                    target_path: entry.target_path.clone(),
                    kind: if entry.backup_ref.is_some() {
                        BatchTargetWriteKind::Restore
                    } else {
                        BatchTargetWriteKind::Remove
                    },
                })
                .collect::<Vec<_>>();
            target_claims.sort_by(|left, right| {
                left.windows_key()
                    .cmp(&right.windows_key())
                    .then_with(|| left.kind.as_str().cmp(right.kind.as_str()))
            });
            let single_plan_digest = manifest
                .as_ref()
                .map(|manifest| uninstall_manifest_snapshot_digest(manifest, &input.mod_id))
                .unwrap_or_else(|| sha256_digest(b"hmm-uninstall-manifest-missing-v1"));
            let fact_digest = uninstall_fact_digest(
                manifest.as_ref(),
                &entries,
                summary,
                &blocking_reasons,
                &single_plan_digest,
            )?;
            items.push(BatchItemFacts {
                mod_id: input.mod_id.clone(),
                source_revision_id: None,
                installed_revision_id,
                fact_digest,
                single_plan_digest,
                target_claims,
                action_summary: BatchActionSummary {
                    actions: entries.len(),
                    ..BatchActionSummary::default()
                },
                prerequisite: BatchPreflightDecision {
                    status: BatchPreflightStatus::Ready,
                    rules_version: None,
                    codes: Vec::new(),
                },
                blocking_reasons,
                warning_codes: Vec::new(),
            });
        }

        Ok(BatchPlanFacts {
            environment_digest: self.environment_digest.clone(),
            prerequisite_rules_version: None,
            global_blocking_reasons: global_reason_counts
                .into_iter()
                .map(|(code, count)| BatchReasonSummary { code, count })
                .collect(),
            items,
        })
    }
}

pub struct UninstallTaskBatchItemExecutor {
    runner: Arc<UninstallTaskRunner>,
    task_manager: Arc<TaskManager>,
}

impl UninstallTaskBatchItemExecutor {
    pub fn new(runner: Arc<UninstallTaskRunner>, task_manager: Arc<TaskManager>) -> Self {
        Self {
            runner,
            task_manager,
        }
    }
}

impl BatchInstallItemExecutor for UninstallTaskBatchItemExecutor {
    fn execute(&self, request: BatchInstallItemRequest) -> BatchInstallItemExecution {
        let plan_item = match request
            .plan
            .items
            .iter()
            .find(|item| item.ordinal == request.item.ordinal)
        {
            Some(item) => item,
            _ => {
                return BatchInstallItemExecution::Blocked {
                    reason_code: "batch_item_not_planned".to_owned(),
                };
            }
        };
        let BatchItemInput::Uninstall(input) = &plan_item.input_snapshot else {
            return BatchInstallItemExecution::Blocked {
                reason_code: "batch_operation_not_uninstall".to_owned(),
            };
        };
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
        let result = self
            .runner
            .run_uninstall_revision_task_for_orchestration_with_observer(
                &child.task_id,
                StartUninstallTaskRequest {
                    game_id: request.plan.game_id.clone(),
                    profile_id: request.plan.profile_id.clone(),
                    mod_id: input.mod_id.clone(),
                },
                input.expected_installed_revision_id.clone(),
                plan_item.single_plan_digest.clone(),
                &observer,
            );
        match result {
            Ok(ref events)
                if self.task_manager.task_status(&child.task_id) == Some(TaskStatus::Completed) =>
            {
                BatchInstallItemExecution::Succeeded {
                    evidence_health_degraded: events_contain_audit_degradation(events),
                }
            }
            Ok(_) => BatchInstallItemExecution::Cancelled,
            Err(ref error) if error.committed => classify_uninstall_task_failure(error),
            Err(_)
                if self.task_manager.task_status(&child.task_id) == Some(TaskStatus::Cancelled) =>
            {
                BatchInstallItemExecution::Cancelled
            }
            Err(error) => classify_uninstall_task_failure(&error),
        }
    }
}

fn classify_uninstall_task_failure(error: &UninstallTaskRunError) -> BatchInstallItemExecution {
    if error.committed {
        return BatchInstallItemExecution::Succeeded {
            evidence_health_degraded: true,
        };
    }
    match error.uninstall_error.as_ref() {
        Some(UninstallModError::RollbackFailed { .. }) => {
            BatchInstallItemExecution::RecoveryRequired {
                reason_code: "uninstall_rollback_failed".to_owned(),
            }
        }
        Some(
            UninstallModError::ModNotInstalled
            | UninstallModError::InstalledRevisionMismatch
            | UninstallModError::ManifestStateMismatch
            | UninstallModError::MissingInstalledFileSummary
            | UninstallModError::TargetStateMismatch
            | UninstallModError::BackupUnavailable,
        ) => BatchInstallItemExecution::Blocked {
            reason_code: "uninstall_plan_stale".to_owned(),
        },
        Some(
            UninstallModError::ManifestSaveFailed
            | UninstallModError::RemoveFailed
            | UninstallModError::RestoreFailed,
        ) => BatchInstallItemExecution::Failed {
            reason_code: "uninstall_rollback_succeeded".to_owned(),
            retryable: true,
            evidence_health_degraded: false,
        },
        Some(
            UninstallModError::GameInstanceUnavailable | UninstallModError::ManifestUnavailable,
        )
        | None => BatchInstallItemExecution::Failed {
            reason_code: "uninstall_unavailable".to_owned(),
            retryable: true,
            evidence_health_degraded: false,
        },
    }
}

fn uninstall_blocking_reasons(
    summary: Option<&InstallRecoverySummary>,
    entries: &[InstallManifestEntry],
) -> Vec<String> {
    let mut reasons = Vec::new();
    if entries.is_empty() {
        reasons.push("mod_not_installed".to_owned());
    }
    match summary.map(|summary| summary.status) {
        Some(InstallRecoveryStatus::Completed) | None => {}
        Some(InstallRecoveryStatus::NotInstalled) => reasons.push("mod_not_installed".to_owned()),
        Some(InstallRecoveryStatus::CommittedCleanupPending)
        | Some(InstallRecoveryStatus::CleanupPending) => {
            reasons.push("install_recovery_active".to_owned())
        }
        Some(InstallRecoveryStatus::RollbackRequired) => {
            reasons.push("install_rollback_required".to_owned())
        }
        Some(InstallRecoveryStatus::RepairRequired) => {
            if summary.is_some_and(|summary| summary.issues.is_empty()) {
                reasons.push("install_repair_required".to_owned());
            }
        }
        Some(InstallRecoveryStatus::Unknown) => {
            if summary.is_some_and(|summary| summary.issues.is_empty()) {
                reasons.push("install_state_unknown".to_owned());
            }
        }
    }
    if let Some(summary) = summary {
        for issue in &summary.issues {
            reasons.push(recovery_issue_code(issue.issue).to_owned());
        }
    }
    reasons
}

fn recovery_issue_code(issue: InstallRecoveryIssue) -> &'static str {
    match issue {
        InstallRecoveryIssue::MissingInstalledFileSummary => "installed_summary_missing",
        InstallRecoveryIssue::TargetMissing => "installed_target_missing",
        InstallRecoveryIssue::TargetChanged => "installed_target_changed",
        InstallRecoveryIssue::TargetReadFailed => "installed_target_unavailable",
        InstallRecoveryIssue::BackupMissing => "install_backup_missing",
        InstallRecoveryIssue::BackupReadFailed => "install_backup_unavailable",
    }
}

fn recovery_is_globally_active(summary: &InstallRecoverySummary) -> bool {
    summary.issues.is_empty()
        && matches!(
            summary.status,
            InstallRecoveryStatus::CommittedCleanupPending
                | InstallRecoveryStatus::CleanupPending
                | InstallRecoveryStatus::RollbackRequired
                | InstallRecoveryStatus::RepairRequired
                | InstallRecoveryStatus::Unknown
        )
}

fn target_owners(manifest: Option<&InstallManifest>) -> BTreeMap<String, BTreeSet<ModId>> {
    let mut owners = BTreeMap::<String, BTreeSet<ModId>>::new();
    if let Some(manifest) = manifest {
        for entry in &manifest.entries {
            owners
                .entry(target_key(entry))
                .or_default()
                .insert(entry.mod_id.clone());
        }
    }
    owners
}

fn backup_owners(manifest: Option<&InstallManifest>) -> BTreeMap<String, BTreeSet<ModId>> {
    let mut owners = BTreeMap::<String, BTreeSet<ModId>>::new();
    if let Some(manifest) = manifest {
        for entry in &manifest.entries {
            if let Some(backup_ref) = &entry.backup_ref {
                owners
                    .entry(backup_ref.clone())
                    .or_default()
                    .insert(entry.mod_id.clone());
            }
        }
    }
    owners
}

fn target_key(entry: &InstallManifestEntry) -> String {
    entry.target_path.as_str().to_ascii_lowercase()
}

fn has_duplicate_target(entries: &[InstallManifestEntry]) -> bool {
    let mut targets = BTreeSet::new();
    entries
        .iter()
        .any(|entry| !targets.insert(target_key(entry)))
}

fn has_duplicate_backup(entries: &[InstallManifestEntry]) -> bool {
    let mut backups = BTreeSet::new();
    entries.iter().any(|entry| {
        entry
            .backup_ref
            .as_ref()
            .is_some_and(|backup_ref| !backups.insert(backup_ref.clone()))
    })
}

fn exact_installed_revision(
    schema_version: Option<u32>,
    entries: &[InstallManifestEntry],
) -> Option<ModRevisionId> {
    if schema_version != Some(INSTALL_MANIFEST_SCHEMA_VERSION_V2) || entries.is_empty() {
        return None;
    }
    let revision = entries.first()?.revision_id.clone()?;
    entries
        .iter()
        .all(|entry| entry.revision_id.as_ref() == Some(&revision))
        .then_some(revision)
}

fn uninstall_fact_digest(
    manifest: Option<&InstallManifest>,
    entries: &[InstallManifestEntry],
    summary: Option<&InstallRecoverySummary>,
    blocking_reasons: &[String],
    manifest_snapshot_digest: &str,
) -> anyhow::Result<String> {
    let mut canonical_entries = entries
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?;
    canonical_entries.sort();
    let issues = summary
        .map(|summary| {
            summary
                .issues
                .iter()
                .map(|issue| (recovery_issue_code(issue.issue), issue.count))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let canonical = serde_json::json!({
        "manifestSchemaVersion": manifest.map(|manifest| manifest.schema_version),
        "manifestStatus": manifest.map(|manifest| install_manifest_status_code(manifest.status)),
        "manifestSnapshotDigest": manifest_snapshot_digest,
        "entries": canonical_entries,
        "recoveryStatus": summary.map(|summary| recovery_status_code(summary.status)),
        "issues": issues,
        "blockingReasons": blocking_reasons,
    });
    Ok(sha256_digest(&serde_json::to_vec(&canonical)?))
}

fn recovery_status_code(status: InstallRecoveryStatus) -> &'static str {
    match status {
        InstallRecoveryStatus::NotInstalled => "not_installed",
        InstallRecoveryStatus::Completed => "completed",
        InstallRecoveryStatus::CommittedCleanupPending => "committed_cleanup_pending",
        InstallRecoveryStatus::CleanupPending => "cleanup_pending",
        InstallRecoveryStatus::RollbackRequired => "rollback_required",
        InstallRecoveryStatus::RepairRequired => "repair_required",
        InstallRecoveryStatus::Unknown => "unknown",
    }
}

fn sha256_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[cfg(test)]
#[path = "batch_uninstall_tests.rs"]
mod tests;
