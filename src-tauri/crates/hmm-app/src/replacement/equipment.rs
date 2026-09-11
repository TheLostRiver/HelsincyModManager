use super::*;
use crate::InstallManifestQueryService;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct EquipmentSourceConfiguration {
    pub source: hmm_core::ReplacementSource,
    pub display_names: BTreeMap<String, String>,
    pub original_target_id: Option<ReplacementTargetId>,
    pub targets: Vec<ReplacementTarget>,
}

#[derive(Debug, Clone)]
pub struct EquipmentRetargetConfiguration {
    pub game_id: GameId,
    pub mod_id: ModId,
    pub analysis: ReplacementAnalysis,
    pub sources: Vec<EquipmentSourceConfiguration>,
    /// None 表示未选择配置档或安装事实无法确认；不是“没有安装”。
    pub installed_targets: Option<BTreeMap<ReplacementSourceId, ReplacementTargetId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquipmentRetargetReinstallRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub slots: Vec<InitialRetargetSlotIntent>,
    pub layer: FileLayer,
}

#[derive(Debug, Clone)]
pub struct PreviewEquipmentRetargetReinstallRequest {
    pub selection: EquipmentRetargetReinstallRequest,
    pub installed_revision_id: ModRevisionId,
    pub installed_bindings: Vec<ReplacementBindingSnapshot>,
}

impl ReplacementWorkflowService {
    pub fn equipment_configuration(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        profile_id: Option<&ProfileId>,
    ) -> Result<EquipmentRetargetConfiguration, ReplacementWorkflowError> {
        let bindings = profile_id.and_then(|profile| {
            InstallManifestQueryService::new(Arc::clone(&self.install_manifests))
                .query_installed_replacement_bindings_for_display(profile, mod_id)
                .ok()
        });
        let revisions = bindings
            .as_ref()
            .into_iter()
            .flatten()
            .filter_map(|binding| binding.revision_id().cloned())
            .collect::<BTreeSet<_>>();
        let resolved = if revisions.len() == 1 {
            self.resolve_imported_revision(
                game_id,
                mod_id,
                revisions.first().expect("one revision"),
            )?
        } else {
            self.resolve_imported_replacement(game_id, mod_id)?
        };
        let catalog = self.catalog_for(game_id)?;
        let all_targets = self.list_targets(game_id, None)?;
        let names = self
            .describe_sources(&resolved.analysis)
            .into_iter()
            .map(|source| (source.id.clone(), source.display_names))
            .collect::<BTreeMap<_, _>>();
        let sources = resolved
            .analysis
            .sources()
            .iter()
            .map(|source| EquipmentSourceConfiguration {
                source: source.clone(),
                display_names: names.get(source.id().as_str()).cloned().unwrap_or_default(),
                original_target_id: self
                    .self_target_for(game_id, source)
                    .ok()
                    .map(|target| target.id().clone()),
                targets: all_targets
                    .iter()
                    .filter(|target| {
                        target.target_type() == source.source_type()
                            && target
                                .metadata()
                                .get("path_family")
                                .and_then(serde_json::Value::as_str)
                                == Some(source.path_family())
                    })
                    .cloned()
                    .collect(),
            })
            .collect();
        let installed_targets = bindings.and_then(|bindings| {
            let mut targets = BTreeMap::new();
            for binding in bindings {
                let source = resolved
                    .analysis
                    .sources()
                    .iter()
                    .find(|source| source.id() == binding.binding().source_id())?;
                if source.internal_id() != binding.source_internal_id()
                    || source.path_family() != binding.source_path_family()
                    || source.source_type() != binding.retarget_kind()
                {
                    return None;
                }
                let target = catalog
                    .find_replacement_target(binding.binding().target_id())
                    .or_else(|error| {
                        catalog
                            .original_target_for_source(source)
                            .and_then(|target| {
                                if target.id() == binding.binding().target_id() {
                                    Ok(target)
                                } else {
                                    Err(error)
                                }
                            })
                    })
                    .ok()?;
                if target.internal_id() != binding.target_internal_id()
                    || target.target_type() != binding.retarget_kind()
                    || target
                        .metadata()
                        .get("path_family")
                        .and_then(serde_json::Value::as_str)
                        != Some(binding.target_path_family())
                {
                    return None;
                }
                if targets
                    .insert(source.id().clone(), target.id().clone())
                    .is_some()
                {
                    return None;
                }
            }
            Some(targets)
        });
        Ok(EquipmentRetargetConfiguration {
            game_id: game_id.clone(),
            mod_id: mod_id.clone(),
            analysis: resolved.analysis,
            sources,
            installed_targets,
        })
    }

    /// 显式的整组目标切换。旧单源入口仍保留它自己的单绑定拒绝条件。
    pub fn preview_equipment_reinstall(
        &self,
        request: PreviewEquipmentRetargetReinstallRequest,
    ) -> Result<PlannedInitialRetargetInstall, ReplacementWorkflowError> {
        let selection = request.selection;
        let resolved = self.resolve_imported_revision(
            &selection.game_id,
            &selection.mod_id,
            &request.installed_revision_id,
        )?;
        if selection.slots.len() != resolved.analysis.sources().len() || selection.slots.is_empty()
        {
            return Err(ReplacementWorkflowError::SourceNotRetargetable);
        }
        let mut installed = BTreeMap::new();
        for binding in request.installed_bindings {
            let source = resolved
                .analysis
                .sources()
                .iter()
                .find(|source| source.id() == binding.binding().source_id())
                .ok_or(ReplacementWorkflowError::InstalledBindingUnavailable)?;
            if binding.mod_id() != &selection.mod_id
                || binding.profile_id() != &selection.profile_id
                || binding
                    .revision_id()
                    .is_some_and(|revision| revision != &request.installed_revision_id)
                || binding.source_internal_id() != source.internal_id()
                || binding.source_path_family() != source.path_family()
                || binding.target_path_family() != source.path_family()
                || binding.retarget_kind() != source.source_type()
            {
                return Err(ReplacementWorkflowError::InstalledBindingUnavailable);
            }
            if installed.insert(source.id().clone(), binding).is_some() {
                return Err(ReplacementWorkflowError::InstalledBindingUnavailable);
            }
        }
        if installed.is_empty() {
            return Err(ReplacementWorkflowError::InstalledBindingUnavailable);
        }
        let catalog = self.catalog_for(&selection.game_id)?;
        let reader = ImportedReplacementContentReader {
            reader: self.file_reader.as_ref(),
            package_id: &resolved.package_id,
            sandbox_root: &resolved.sandbox_root,
        };
        let mut seen_sources = BTreeSet::new();
        let mut targets = Vec::new();
        let mut plans = Vec::new();
        let mut changed = false;
        for (index, intent) in selection.slots.iter().enumerate() {
            if !seen_sources.insert(intent.source_id().clone()) {
                return Err(ReplacementWorkflowError::DuplicateSlotIntent);
            }
            let source = resolved
                .analysis
                .sources()
                .iter()
                .find(|source| source.id() == intent.source_id())
                .ok_or(ReplacementWorkflowError::SourceNotRetargetable)?;
            let target = match intent {
                InitialRetargetSlotIntent::Retarget { target_id, .. } => catalog
                    .find_replacement_target(target_id)
                    .map_err(map_catalog_error)?,
                InitialRetargetSlotIntent::KeepInPlace { .. } => {
                    self.self_target_for(&selection.game_id, source)?
                }
            };
            let previous = installed.get(source.id());
            changed |=
                previous.is_none_or(|binding| binding.target_internal_id() != target.internal_id());
            let binding_id = match previous {
                Some(binding) => binding.binding_id().clone(),
                None => canonical_source_binding_id(
                    &selection.game_id,
                    &selection.profile_id,
                    &selection.mod_id,
                    source.id(),
                    target.id(),
                )?,
            };
            let binding = ReplacementBinding::new(
                binding_id,
                selection.mod_id.clone(),
                selection.profile_id.clone(),
                source.id().clone(),
                target.id().clone(),
                previous.map_or(0, |binding| binding.binding().created_at_unix_millis()),
            )
            .map_err(|_| ReplacementWorkflowError::BindingUnavailable)?;
            let plan = self
                .replacement
                .build_retarget_plan_with_content(
                    RetargetPlanRequest {
                        game_id: selection.game_id.clone(),
                        binding,
                        assets: resolved.assets.clone(),
                        carries_package_companions: index == 0,
                    },
                    &reader,
                )
                .map_err(ReplacementWorkflowError::Analysis)?;
            targets.push(target);
            plans.push(plan);
        }
        if !changed {
            return Err(ReplacementWorkflowError::TargetAlreadySelected);
        }
        let install_plan = self
            .replacement
            .build_retarget_install_plan_for_all(
                &plans,
                selection.layer.clone(),
                Some(request.installed_revision_id.clone()),
            )
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
        let install_plan =
            self.append_cross_mod_target_conflicts(install_plan, &selection.profile_id)?;
        Ok(PlannedInitialRetargetInstall {
            package_id: resolved.package_id,
            revision_id: request.installed_revision_id,
            layer: selection.layer,
            analysis: resolved.analysis,
            targets,
            retarget_plans: plans,
            install_plan,
        })
    }

    /// 重装已有完整候选集时使用一处 staging；每个 PackageFileId 仍只能对应一个产出。
    pub fn materialize_equipment_reinstall(
        &self,
        staging: &dyn RetargetStagingMaterializer,
        planned: PlannedInitialRetargetInstall,
    ) -> Result<InstallPlan, ReplacementWorkflowError> {
        let files = planned
            .retarget_plans
            .iter()
            .flat_map(retarget_staging_files)
            .collect::<Vec<_>>();
        staging
            .materialize(&files)
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
        let mut install_plan = self
            .replacement
            .build_retarget_install_plan_for_all(
                &planned.retarget_plans,
                planned.layer,
                Some(planned.revision_id),
            )
            .map_err(|_| ReplacementWorkflowError::PlanUnavailable)?;
        install_plan.conflicts = planned.install_plan.conflicts;
        Ok(install_plan)
    }
}
