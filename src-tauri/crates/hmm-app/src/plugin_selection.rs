use hmm_core::{
    GameId, InstallManifest, InstallManifestStatusConsumption, InstallTargetPath, ModId,
    ModRevisionId, PackageFileId, PluginFileChoice, PluginFileChoiceKind, PluginSelectionScope,
    PluginSelectionSnapshot, ProfileId,
};
use hmm_ports::{
    GamePluginPolicy, InstallManifestRepository, ModImportResultRepository,
    ModImportSandboxLocator, ModPackageFileSelectionRepository, ModPackageInstallFileReadRequest,
    ModPackageInstallFileReader, ModPackageInstallFileScanRequest, ModPackageInstallFileScanner,
    PluginFileCheck, PluginSelectionRepository,
};
use std::collections::BTreeSet;
use std::sync::Arc;
use thiserror::Error;

#[path = "plugin_plan.rs"]
mod plan;

pub struct PluginSelectionSources {
    pub catalog: Arc<dyn ModImportResultRepository>,
    pub sandboxes: Arc<dyn ModImportSandboxLocator>,
    /// 保留内容根选择，但不提前滤掉文件；包级排除单独参与盘点和摘要。
    pub scanner: Arc<dyn ModPackageInstallFileScanner>,
    pub reader: Arc<dyn ModPackageInstallFileReader>,
    pub package_selection: Arc<dyn ModPackageFileSelectionRepository>,
    pub selections: Arc<dyn PluginSelectionRepository>,
    pub manifests: Arc<dyn InstallManifestRepository>,
    pub policies: Vec<Arc<dyn GamePluginPolicy>>,
}

pub struct PluginSelectionService {
    sources: PluginSelectionSources,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum PluginSelectionServiceError {
    #[error("plugin selection is unavailable")]
    Unavailable,
    #[error("plugin source is unavailable")]
    SourceUnavailable,
    #[error("plugin selection inventory changed")]
    InventoryChanged,
    #[error("plugin selection is invalid")]
    InvalidSelection,
    #[error("plugin selection needs confirmation")]
    ConfirmationRequired,
    #[error("installed plugin facts are not usable")]
    ManifestUnverified,
}

impl PluginSelectionServiceError {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unavailable => "plugin_selection_unavailable",
            Self::SourceUnavailable => "plugin_source_unavailable",
            Self::InventoryChanged => "plugin_inventory_changed",
            Self::InvalidSelection => "plugin_selection_invalid",
            Self::ConfirmationRequired => "plugin_selection_required",
            Self::ManifestUnverified => "plugin_manifest_unverified",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginCandidate {
    pub package_file_id: PackageFileId,
    pub target_path: InstallTargetPath,
    pub size_bytes: u64,
    pub check: PluginFileCheck,
    pub selected: bool,
    pub selectable: bool,
    pub installed: bool,
    pub retain_only: bool,
    pub excluded_by_package: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInventory {
    pub selection: PluginSelectionSnapshot,
    pub candidates: Vec<PluginCandidate>,
    pub confirmation_required: bool,
}

type Result<T> = std::result::Result<T, PluginSelectionServiceError>;

impl PluginSelectionService {
    pub fn new(sources: PluginSelectionSources) -> Self {
        Self { sources }
    }

    pub fn resolve_scope(
        &self,
        game_id: GameId,
        profile_id: ProfileId,
        mod_id: ModId,
        requested_revision: Option<ModRevisionId>,
    ) -> Result<PluginSelectionScope> {
        let manifest = self.manifest(&profile_id)?;
        let revision_id = if let Some(revision) = requested_revision {
            revision
        } else if let Some(installed) = manifest
            .as_ref()
            .and_then(|manifest| manifest.entries.iter().find(|entry| entry.mod_id == mod_id))
        {
            match &installed.revision_id {
                Some(revision) => revision.clone(),
                None => {
                    let revisions = self
                        .sources
                        .catalog
                        .list_revisions(&mod_id)
                        .map_err(|_| PluginSelectionServiceError::Unavailable)?;
                    let [revision] = revisions.as_slice() else {
                        return Err(PluginSelectionServiceError::ManifestUnverified);
                    };
                    revision.revision_id.clone()
                }
            }
        } else {
            self.sources
                .catalog
                .get_mod(&mod_id)
                .map_err(|_| PluginSelectionServiceError::Unavailable)?
                .ok_or(PluginSelectionServiceError::SourceUnavailable)?
                .display_revision_id
        };
        let scope = PluginSelectionScope {
            game_id,
            profile_id,
            mod_id,
            revision_id,
        };
        self.revision(&scope)?;
        Ok(scope)
    }

    pub fn inventory(&self, scope: &PluginSelectionScope) -> Result<Option<PluginInventory>> {
        let Some(policy) = self
            .sources
            .policies
            .iter()
            .find(|policy| policy.game_id() == scope.game_id)
        else {
            return Ok(None);
        };
        let revision = self.revision(scope)?;
        let root = self
            .sources
            .sandboxes
            .sandbox_root_for_package(&revision.package_id)
            .map_err(|_| PluginSelectionServiceError::SourceUnavailable)?;
        let scanned = self
            .sources
            .scanner
            .scan_install_files(ModPackageInstallFileScanRequest {
                package_id: &revision.package_id,
                sandbox_root: &root,
            })
            .map_err(|_| PluginSelectionServiceError::SourceUnavailable)?;
        let excluded = self
            .sources
            .package_selection
            .load_excluded_files(&revision.package_id)
            .map_err(|_| PluginSelectionServiceError::Unavailable)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let manifest = self.manifest(&scope.profile_id)?;
        let installed_revision = manifest.as_ref().and_then(|manifest| {
            manifest
                .entries
                .iter()
                .find(|entry| entry.mod_id == scope.mod_id)
        });
        let same_revision = installed_revision.is_some_and(|entry| {
            entry
                .revision_id
                .as_ref()
                .is_none_or(|revision| revision == &scope.revision_id)
        });
        let applied = manifest.as_ref().and_then(|manifest| {
            manifest
                .plugin_selections
                .iter()
                .find(|selection| selection.scope().mod_id == scope.mod_id)
        });
        let mut files = Vec::new();
        let mut candidates = Vec::new();
        for item in scanned {
            let target_root = item.target_path.split('/').next().unwrap_or_default();
            let path = InstallTargetPath::parse(&item.target_path, [target_root])
                .map_err(|_| PluginSelectionServiceError::SourceUnavailable)?;
            let plugin_candidate = policy.is_candidate(&path);
            if !plugin_candidate && !policy.is_excluded_attachment(&path) {
                continue;
            }
            let id = PackageFileId::new(item.package_file_id);
            let bytes = self
                .sources
                .reader
                .read_install_file(ModPackageInstallFileReadRequest {
                    package_id: &revision.package_id,
                    sandbox_root: &root,
                    package_file_id: &id,
                    max_bytes: u64::MAX,
                })
                .map_err(|_| PluginSelectionServiceError::SourceUnavailable)?;
            let summary = crate::reinstall::summarize(&bytes);
            let check = if plugin_candidate {
                policy.inspect(&bytes)
            } else {
                PluginFileCheck::PolicyExcluded
            };
            let existing = manifest.as_ref().and_then(|manifest| {
                manifest.entries.iter().find(|entry| {
                    entry.mod_id == scope.mod_id
                        && entry.package_file_id == id
                        && entry
                            .target_path
                            .as_str()
                            .eq_ignore_ascii_case(path.as_str())
                })
            });
            let trusted_existing = same_revision
                && existing.is_some_and(|entry| {
                    !entry.adopted && entry.installed_file.as_ref() == Some(&summary)
                });
            let excluded_by_package = excluded.contains(id.as_str());
            let selectable =
                !excluded_by_package && (check == PluginFileCheck::Supported || trusted_existing);
            let previous = applied.and_then(|selection| {
                selection.files().iter().find(|file| {
                    file.target_path
                        .as_str()
                        .eq_ignore_ascii_case(path.as_str())
                })
            });
            let selected = selectable
                && previous.map_or_else(
                    || {
                        if same_revision {
                            trusted_existing
                        } else {
                            check == PluginFileCheck::Supported
                        }
                    },
                    |file| file.choice.is_included(),
                );
            let retain_only = check != PluginFileCheck::Supported && trusted_existing;
            files.push(PluginFileChoice {
                package_file_id: id.clone(),
                target_path: path.clone(),
                source_file: summary,
                choice: choice_kind(selected, retain_only),
                excluded_by_package_selection: excluded_by_package,
            });
            candidates.push(PluginCandidate {
                package_file_id: id,
                target_path: path,
                size_bytes: bytes.len() as u64,
                check,
                selected,
                selectable,
                installed: existing.is_some(),
                retain_only,
                excluded_by_package,
            });
        }
        if same_revision {
            for entry in manifest
                .iter()
                .flat_map(|manifest| &manifest.entries)
                .filter(|entry| entry.mod_id == scope.mod_id)
            {
                let policy_owned = policy.is_candidate(&entry.target_path)
                    || policy.is_excluded_attachment(&entry.target_path)
                    || files
                        .iter()
                        .any(|file| file.package_file_id == entry.package_file_id);
                if policy_owned
                    && (entry.adopted
                        || !files.iter().any(|file| {
                            file.package_file_id == entry.package_file_id
                                && file.target_path.windows_key() == entry.target_path.windows_key()
                                && entry.installed_file.as_ref() == Some(&file.source_file)
                        }))
                {
                    // Excluding an unverified legacy attachment must not turn a blocked
                    // target switch into permission to delete that player's file.
                    return Err(PluginSelectionServiceError::ManifestUnverified);
                }
            }
        }
        if files.is_empty() {
            return Ok(None);
        }
        let base = PluginSelectionSnapshot::new(
            scope.clone(),
            policy.policy_id(),
            policy.policy_version(),
            files,
        )
        .map_err(|_| PluginSelectionServiceError::InvalidSelection)?;
        let pending = self
            .sources
            .selections
            .load_selection(scope)
            .map_err(|_| PluginSelectionServiceError::Unavailable)?;
        let matching = pending
            .as_ref()
            .filter(|selection| selection.inventory_id() == base.inventory_id());
        let mut files = base.files().to_vec();
        for candidate in &mut candidates {
            if let Some(previous) = matching.and_then(|selection| {
                selection
                    .files()
                    .iter()
                    .find(|file| file.package_file_id == candidate.package_file_id)
            }) {
                candidate.selected = candidate.selectable && previous.choice.is_included();
            }
            let file = files
                .iter_mut()
                .find(|file| file.package_file_id == candidate.package_file_id)
                .expect("candidate inventory is complete");
            file.choice = choice_kind(candidate.selected, candidate.retain_only);
        }
        let selection = PluginSelectionSnapshot::new(
            scope.clone(),
            policy.policy_id(),
            policy.policy_version(),
            files,
        )
        .map_err(|_| PluginSelectionServiceError::InvalidSelection)?;
        let confirmation_required = matching.is_none()
            && candidates.iter().any(|candidate| {
                let file = selection
                    .files()
                    .iter()
                    .find(|file| file.package_file_id == candidate.package_file_id)
                    .expect("candidate fact");
                let unchanged_managed = manifest.as_ref().is_some_and(|manifest| {
                    manifest.entries.iter().any(|entry| {
                        entry.mod_id == scope.mod_id
                            && entry.package_file_id == file.package_file_id
                            && !entry.adopted
                            && entry
                                .revision_id
                                .as_ref()
                                .is_none_or(|revision| revision == &scope.revision_id)
                            && entry
                                .target_path
                                .as_str()
                                .eq_ignore_ascii_case(file.target_path.as_str())
                            && entry.installed_file.as_ref() == Some(&file.source_file)
                    })
                });
                (candidate.selected && !unchanged_managed)
                    || (!candidate.selected && candidate.installed)
            });
        Ok(Some(PluginInventory {
            selection,
            candidates,
            confirmation_required,
        }))
    }

    pub fn select(
        &self,
        scope: &PluginSelectionScope,
        inventory_id: &str,
        included: &[PackageFileId],
    ) -> Result<PluginInventory> {
        let current = self
            .inventory(scope)?
            .ok_or(PluginSelectionServiceError::InventoryChanged)?;
        if current.selection.inventory_id() != inventory_id {
            return Err(PluginSelectionServiceError::InventoryChanged);
        }
        let chosen = included.iter().collect::<BTreeSet<_>>();
        if chosen.len() != included.len()
            || chosen.iter().any(|id| {
                !current
                    .candidates
                    .iter()
                    .any(|candidate| &candidate.package_file_id == *id && candidate.selectable)
            })
        {
            return Err(PluginSelectionServiceError::InvalidSelection);
        }
        let mut files = current.selection.files().to_vec();
        for file in &mut files {
            let candidate = current
                .candidates
                .iter()
                .find(|candidate| candidate.package_file_id == file.package_file_id)
                .expect("candidate fact");
            file.choice = choice_kind(
                chosen.contains(&file.package_file_id),
                candidate.retain_only,
            );
        }
        let selected = PluginSelectionSnapshot::new(
            scope.clone(),
            current.selection.policy_id(),
            current.selection.policy_version(),
            files,
        )
        .map_err(|_| PluginSelectionServiceError::InvalidSelection)?;
        self.sources
            .selections
            .save_selection(&selected)
            .map_err(|_| PluginSelectionServiceError::Unavailable)?;
        self.inventory(scope)?
            .ok_or(PluginSelectionServiceError::InventoryChanged)
    }

    pub fn ensure_confirmed(&self, expected: &PluginSelectionSnapshot) -> Result<()> {
        // 提交时只重验小型配置／清单事实；源字节由已有提交读取器复核，不在写锁内重新扫描整包。
        let manifest = self.manifest(&expected.scope().profile_id)?;
        let pending = self
            .sources
            .selections
            .load_selection(expected.scope())
            .map_err(|_| PluginSelectionServiceError::Unavailable)?;
        let owned = |file: &PluginFileChoice| {
            manifest.as_ref().and_then(|manifest| {
                manifest.entries.iter().find(|entry| {
                    entry.mod_id == expected.scope().mod_id
                        && entry.package_file_id == file.package_file_id
                        && entry
                            .target_path
                            .as_str()
                            .eq_ignore_ascii_case(file.target_path.as_str())
                })
            })
        };
        if let Some(pending) =
            pending.filter(|selection| selection.inventory_id() == expected.inventory_id())
        {
            let agrees = pending
                .files()
                .iter()
                .zip(expected.files())
                .all(|(previous, current)| {
                    previous.choice == current.choice
                        || (previous.choice == PluginFileChoiceKind::RetainInstalled
                            && current.choice == PluginFileChoiceKind::Exclude
                            && owned(current).is_none())
                });
            return if agrees {
                Ok(())
            } else {
                Err(PluginSelectionServiceError::InventoryChanged)
            };
        }
        for file in expected.files() {
            if file.choice.is_included() {
                let unchanged = owned(file).is_some_and(|entry| {
                    !entry.adopted
                        && entry
                            .revision_id
                            .as_ref()
                            .is_none_or(|revision| revision == &expected.scope().revision_id)
                        && entry.installed_file.as_ref() == Some(&file.source_file)
                });
                if !unchanged {
                    return Err(PluginSelectionServiceError::ConfirmationRequired);
                }
            } else if owned(file).is_some() {
                return Err(PluginSelectionServiceError::ConfirmationRequired);
            }
        }
        Ok(())
    }

    /// 仅由已经验证生命周期／批量 token 的执行入口调用，记录被该完整计划批准的选择。
    pub fn record_approved_selections(&self, selections: &[PluginSelectionSnapshot]) -> Result<()> {
        for selection in selections {
            self.sources
                .selections
                .save_selection(selection)
                .map_err(|_| PluginSelectionServiceError::Unavailable)?;
        }
        Ok(())
    }

    fn revision(&self, scope: &PluginSelectionScope) -> Result<hmm_ports::StoredModRevision> {
        self.sources
            .catalog
            .get_revision(&scope.revision_id)
            .map_err(|_| PluginSelectionServiceError::Unavailable)?
            .filter(|revision| revision.mod_id == scope.mod_id)
            .ok_or(PluginSelectionServiceError::SourceUnavailable)
    }

    fn manifest(&self, profile: &ProfileId) -> Result<Option<InstallManifest>> {
        let manifest = self
            .sources
            .manifests
            .load_manifest(profile)
            .map_err(|_| PluginSelectionServiceError::ManifestUnverified)?;
        if manifest.as_ref().is_some_and(|manifest| {
            manifest.profile_id != *profile
                || manifest.validate().is_err()
                || manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries
        }) {
            return Err(PluginSelectionServiceError::ManifestUnverified);
        }
        Ok(manifest)
    }
}

fn choice_kind(selected: bool, retain_only: bool) -> PluginFileChoiceKind {
    if !selected {
        PluginFileChoiceKind::Exclude
    } else if retain_only {
        PluginFileChoiceKind::RetainInstalled
    } else {
        PluginFileChoiceKind::Include
    }
}
