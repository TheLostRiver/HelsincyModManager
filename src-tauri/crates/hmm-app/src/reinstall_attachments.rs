use super::*;
use hmm_core::RetargetPolicyExcludedFile;

pub(super) struct AttachmentRetention {
    pub counts: ReinstallAttachmentCounts,
    pub retained_targets: Vec<InstallTargetPath>,
}

/// 同 revision 的完整盘点必须说明每个既有文件的去向。原包文件消失不能被当作政策删除。
pub(super) fn verify_retarget_inventory(
    mod_id: &ModId,
    manifest: &InstallManifest,
    plan: &InstallPlan,
    exclusions: &[RetargetPolicyExcludedFile],
) -> Result<(), ReinstallBlockingReason> {
    let ids = plan
        .actions
        .iter()
        .map(|action| &action.provider.package_file_id)
        .chain(
            exclusions
                .iter()
                .map(RetargetPolicyExcludedFile::package_file_id),
        )
        .collect::<BTreeSet<_>>();
    if manifest
        .entries
        .iter()
        .filter(|entry| &entry.mod_id == mod_id)
        .any(|entry| !ids.contains(&entry.package_file_id))
    {
        return Err(ReinstallBlockingReason::SourceUnavailable);
    }
    Ok(())
}

impl ReinstallPreviewService {
    /// 只补足 adapter 明确排除、当前安装已经可信拥有的附件；游戏字节由统一 preflight 复核。
    pub(super) fn retain_installed_attachments(
        &self,
        request: &ReinstallPreviewRequest,
        candidate: &StoredModRevision,
        manifest: &InstallManifest,
        plan: &mut InstallPlan,
        exclusions: &[RetargetPolicyExcludedFile],
        verified_original: bool,
    ) -> Result<AttachmentRetention, ReinstallBlockingReason> {
        let mut counts = ReinstallAttachmentCounts::default();
        let mut retained_targets = Vec::new();
        let mut ids = plan
            .actions
            .iter()
            .map(|action| action.provider.package_file_id.clone())
            .collect::<BTreeSet<_>>();
        let mut paths = BTreeSet::new();
        for file in exclusions {
            match file.reason() {
                hmm_core::RetargetExclusionReason::ExecutableOrScript => {}
            }
            if !ids.insert(file.package_file_id().clone())
                || !paths.insert(file.original_path().windows_key())
            {
                return Err(ReinstallBlockingReason::CandidateNotReady);
            }
            let entries = manifest
                .entries
                .iter()
                .filter(|entry| {
                    entry.mod_id == request.mod_id
                        && (entry.package_file_id == *file.package_file_id()
                            || entry.target_path.windows_key()
                                == file.original_path().windows_key())
                })
                .collect::<Vec<_>>();
            if entries.is_empty() {
                counts.excluded = counts
                    .excluded
                    .checked_add(1)
                    .ok_or(ReinstallBlockingReason::CandidateNotReady)?;
                continue;
            }
            let [entry] = entries.as_slice() else {
                return Err(ReinstallBlockingReason::InstalledAttachmentUnverified);
            };
            if entry.adopted
                || entry.package_file_id != *file.package_file_id()
                || !entry
                    .target_path
                    .as_str()
                    .eq_ignore_ascii_case(file.original_path().as_str())
                || (entry.revision_id.as_ref() != Some(&candidate.revision_id)
                    && !(entry.revision_id.is_none() && verified_original))
            {
                return Err(ReinstallBlockingReason::InstalledAttachmentUnverified);
            }
            if manifest.entries.iter().any(|other| {
                other.mod_id != request.mod_id
                    && other.target_path.windows_key() == entry.target_path.windows_key()
            }) {
                return Err(ReinstallBlockingReason::CrossModTargetConflict);
            }
            if plan
                .actions
                .iter()
                .any(|action| action.target_path.windows_key() == entry.target_path.windows_key())
            {
                return Err(ReinstallBlockingReason::PlanConflict);
            }
            let expected = entry
                .installed_file
                .as_ref()
                .ok_or(ReinstallBlockingReason::InstalledAttachmentUnverified)?;
            let bytes = self
                .original_source
                .read_candidate_source_file(candidate, &entry.package_file_id)
                .map_err(|_| ReinstallBlockingReason::SourceUnavailable)?;
            if &summarize(&bytes) != expected {
                return Err(ReinstallBlockingReason::InstalledAttachmentUnverified);
            }
            let retained = InstallPlan::from_providers([InstallFileProvider::new(
                entry.mod_id.clone(),
                entry.package_file_id.clone(),
                entry.target_path.clone(),
                entry.layer.clone(),
            )]);
            plan.actions.extend(retained.actions);
            retained_targets.push(entry.target_path.clone());
            counts.retained = counts
                .retained
                .checked_add(1)
                .ok_or(ReinstallBlockingReason::CandidateNotReady)?;
        }
        Ok(AttachmentRetention {
            counts,
            retained_targets,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::RetargetExclusionReason;

    #[test]
    fn source_inventory_cannot_discard_a_missing_attachment_or_model() {
        let mod_id = ModId::new("mod");
        let entry = |name: &str| InstallManifestEntry {
            target_path: InstallTargetPath::parse(format!("content/{name}"), ["content"]).unwrap(),
            mod_id: mod_id.clone(),
            revision_id: Some(ModRevisionId::new("revision")),
            package_file_id: PackageFileId::new(name),
            layer: FileLayer::new("base", 0),
            backup_ref: None,
            installed_file: None,
            adopted: false,
        };
        let model = entry("model.bin");
        let attachment = entry("helper.dll");
        let manifest = InstallManifest::completed(
            ProfileId::new("profile"),
            vec![model.clone(), attachment.clone()],
        );
        let plan = InstallPlan::from_providers([InstallFileProvider::new(
            mod_id.clone(),
            model.package_file_id.clone(),
            model.target_path.clone(),
            model.layer.clone(),
        )]);
        assert_eq!(
            verify_retarget_inventory(&mod_id, &manifest, &plan, &[]),
            Err(ReinstallBlockingReason::SourceUnavailable)
        );
        let excluded = RetargetPolicyExcludedFile::new(
            attachment.package_file_id,
            attachment.target_path,
            RetargetExclusionReason::ExecutableOrScript,
        )
        .unwrap();
        verify_retarget_inventory(&mod_id, &manifest, &plan, std::slice::from_ref(&excluded))
            .unwrap();
        assert_eq!(
            verify_retarget_inventory(
                &mod_id,
                &manifest,
                &InstallPlan::from_providers([]),
                &[excluded]
            ),
            Err(ReinstallBlockingReason::SourceUnavailable)
        );
    }
}
