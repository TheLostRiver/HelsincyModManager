//! Durable installation bookkeeping for display. This service has no game-file or
//! backup-content reader: integrity verification and write admission remain separate.
use crate::{InstallManifestQueryError, InstallManifestStatus, InstallManifestStatusSummary};
use hmm_core::{
    InstallManifestStatusConsumption, InstallRecoveryRecordStatus, ModId, ProfileId,
    ReinstallRecoveryTransactionStatus,
};
use hmm_ports::{
    InstallManifestRepository, InstallRecoveryRecordRepository,
    ReinstallRecoveryTransactionRepository,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModInstallationStateSnapshot {
    pub profile_id: ProfileId,
    pub default_status: InstallManifestStatus,
    pub states: BTreeMap<ModId, InstallManifestStatusSummary>,
}

impl ModInstallationStateSnapshot {
    pub fn summary(&self, mod_id: &ModId) -> InstallManifestStatusSummary {
        self.states
            .get(mod_id)
            .cloned()
            .unwrap_or_else(|| InstallManifestStatusSummary {
                profile_id: self.profile_id.clone(),
                mod_id: mod_id.clone(),
                status: self.default_status,
                managed_file_count: 0,
                backup_count: 0,
                adopted_file_count: Some(0),
                installed_revision_id: None,
            })
    }

    pub fn changed_mod_ids(&self, previous: &Self) -> BTreeSet<ModId> {
        self.states
            .keys()
            .chain(previous.states.keys())
            .filter(|id| self.summary(id) != previous.summary(id))
            .cloned()
            .collect()
    }
}

pub struct ModInstallationStateQueryService {
    manifest: Arc<dyn InstallManifestRepository>,
    recovery: Arc<dyn InstallRecoveryRecordRepository>,
    reinstall: Arc<dyn ReinstallRecoveryTransactionRepository>,
}

impl ModInstallationStateQueryService {
    pub fn new(
        manifest: Arc<dyn InstallManifestRepository>,
        recovery: Arc<dyn InstallRecoveryRecordRepository>,
        reinstall: Arc<dyn ReinstallRecoveryTransactionRepository>,
    ) -> Self {
        Self {
            manifest,
            recovery,
            reinstall,
        }
    }

    pub fn inspect(
        &self,
        profile_id: &ProfileId,
    ) -> Result<ModInstallationStateSnapshot, InstallManifestQueryError> {
        let unavailable = || InstallManifestQueryError::ManifestUnavailable;
        let manifest = self
            .manifest
            .load_manifest(profile_id)
            .map_err(|_| unavailable())?;
        let records = self
            .recovery
            .list_records(profile_id)
            .map_err(|_| unavailable())?;
        let transactions = self
            .reinstall
            .list_transactions(profile_id)
            .map_err(|_| unavailable())?;
        let mut snapshot = ModInstallationStateSnapshot {
            profile_id: profile_id.clone(),
            default_status: InstallManifestStatus::NotInstalled,
            states: BTreeMap::new(),
        };
        if let Some(manifest) = &manifest {
            if manifest.profile_id != *profile_id || manifest.validate().is_err() {
                return Err(unavailable());
            }
            let status = match manifest.status.consumption() {
                InstallManifestStatusConsumption::TrustEntries => InstallManifestStatus::Installed,
                InstallManifestStatusConsumption::InFlight => InstallManifestStatus::Unknown,
                InstallManifestStatusConsumption::RollbackRequired => {
                    InstallManifestStatus::RollbackRequired
                }
                InstallManifestStatusConsumption::RepairRequired => {
                    InstallManifestStatus::RepairRequired
                }
            };
            if status != InstallManifestStatus::Installed {
                snapshot.default_status = status;
            }
            // One pass over the manifest, independent of the number of requested cards.
            for entry in &manifest.entries {
                let summary = snapshot
                    .states
                    .entry(entry.mod_id.clone())
                    .or_insert_with(|| InstallManifestStatusSummary {
                        profile_id: profile_id.clone(),
                        mod_id: entry.mod_id.clone(),
                        status,
                        managed_file_count: 0,
                        backup_count: 0,
                        adopted_file_count: Some(0),
                        installed_revision_id: (status == InstallManifestStatus::Installed)
                            .then(|| entry.revision_id.clone())
                            .flatten(),
                    });
                summary.managed_file_count += 1;
                summary.backup_count += usize::from(entry.backup_ref.is_some());
                *summary
                    .adopted_file_count
                    .as_mut()
                    .expect("manifest count is present") += usize::from(entry.adopted);
            }
        }
        let mut record_ids = BTreeSet::new();
        for record in records {
            if record.profile_id != *profile_id || !record_ids.insert(record.mod_id.clone()) {
                return Err(unavailable());
            }
            let status = match record.status {
                InstallRecoveryRecordStatus::Committing
                | InstallRecoveryRecordStatus::RollbackRequired => {
                    InstallManifestStatus::RollbackRequired
                }
                InstallRecoveryRecordStatus::RepairRequired => {
                    InstallManifestStatus::RepairRequired
                }
                // These states do not establish a new committed installation. The existing
                // manifest remains authoritative, as in the recovery scanner.
                InstallRecoveryRecordStatus::Planned
                | InstallRecoveryRecordStatus::Completed
                | InstallRecoveryRecordStatus::RolledBack => continue,
            };
            let mut summary = snapshot.summary(&record.mod_id);
            summary.status = status;
            summary.installed_revision_id = None;
            if summary.managed_file_count == 0 {
                summary.managed_file_count = record.entries.len();
                summary.backup_count = record
                    .entries
                    .iter()
                    .filter(|entry| entry.backup_ref.is_some())
                    .count();
            }
            snapshot.states.insert(record.mod_id, summary);
        }
        let mut transaction_ids = BTreeSet::new();
        for transaction in transactions {
            if transaction.profile_id != *profile_id
                || !transaction_ids.insert(transaction.mod_id.clone())
            {
                return Err(unavailable());
            }
            let mut summary = snapshot.summary(&transaction.mod_id);
            summary.status =
                if transaction.validate().is_err() || record_ids.contains(&transaction.mod_id) {
                    InstallManifestStatus::Unknown
                } else {
                    match transaction.status {
                        ReinstallRecoveryTransactionStatus::Planned
                        | ReinstallRecoveryTransactionStatus::Committing
                        | ReinstallRecoveryTransactionStatus::RollbackRequired => {
                            InstallManifestStatus::RollbackRequired
                        }
                        ReinstallRecoveryTransactionStatus::RepairRequired => {
                            InstallManifestStatus::RepairRequired
                        }
                        ReinstallRecoveryTransactionStatus::Completed
                        | ReinstallRecoveryTransactionStatus::RolledBack => {
                            InstallManifestStatus::CleanupPending
                        }
                    }
                };
            // A metadata-only read cannot prove a committing candidate or its snapshots.
            summary.installed_revision_id = None;
            snapshot.states.insert(transaction.mod_id, summary);
        }
        Ok(snapshot)
    }
}

/// Observation follows an attempted file transaction; it never authorizes or rolls back it.
pub trait ModInstallationStateObserver: Send + Sync {
    fn state_changed(
        &self,
        task_id: &str,
        game_id: &hmm_core::GameId,
        profile_id: &ProfileId,
        mod_id: &ModId,
    );
}

#[cfg(test)]
#[path = "mod_installation_state_tests.rs"]
mod tests;
