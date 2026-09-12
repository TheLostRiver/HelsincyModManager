use crate::{InstallTargetPath, PackageFileId, RetargetError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetargetExclusionReason {
    ExecutableOrScript,
}

/// Adapter 产生的政策排除事实，不是安装或接管授权，不向 transport 序列化路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetargetPolicyExcludedFile {
    package_file_id: PackageFileId,
    original_path: InstallTargetPath,
    reason: RetargetExclusionReason,
}

impl RetargetPolicyExcludedFile {
    pub fn new(
        package_file_id: PackageFileId,
        original_path: InstallTargetPath,
        reason: RetargetExclusionReason,
    ) -> Result<Self, RetargetError> {
        if package_file_id.as_str().trim().is_empty() {
            return Err(RetargetError::EmptyPackageFileId);
        }
        Ok(Self {
            package_file_id,
            original_path,
            reason,
        })
    }

    pub fn package_file_id(&self) -> &PackageFileId {
        &self.package_file_id
    }

    pub fn original_path(&self) -> &InstallTargetPath {
        &self.original_path
    }

    pub fn reason(&self) -> RetargetExclusionReason {
        self.reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GameId, ModId, ProfileId, ReplacementBinding, ReplacementBindingId, ReplacementSource,
        ReplacementSourceId, ReplacementTargetId, ReplacementTargetKind, RetargetAction,
        RetargetPlan,
    };

    fn path(value: &str) -> InstallTargetPath {
        InstallTargetPath::parse(value, ["content"]).unwrap()
    }

    fn plan() -> RetargetPlan {
        let source_id = ReplacementSourceId::parse("source").unwrap();
        let source = ReplacementSource::new(
            source_id.clone(),
            GameId::mhw(),
            ReplacementTargetKind::parse("weapon").unwrap(),
            "source",
            "family",
            true,
        )
        .unwrap();
        let binding = ReplacementBinding::new(
            ReplacementBindingId::parse("binding").unwrap(),
            ModId::new("mod"),
            ProfileId::new("profile"),
            source_id.clone(),
            ReplacementTargetId::parse("target").unwrap(),
            0,
        )
        .unwrap();
        let action = RetargetAction::new(
            PackageFileId::new("model"),
            path("content/source.model"),
            path("content/target.model"),
            source_id,
            "source",
            "target",
            "family",
            "family",
        )
        .unwrap();
        RetargetPlan::new(binding, source, vec![action], Vec::new()).unwrap()
    }

    #[test]
    fn exclusion_facts_reject_overlapping_files_and_are_not_serialized() {
        let file = |id: &str, value: &str| {
            RetargetPolicyExcludedFile::new(
                PackageFileId::new(id),
                path(value),
                RetargetExclusionReason::ExecutableOrScript,
            )
            .unwrap()
        };
        assert!(RetargetPolicyExcludedFile::new(
            PackageFileId::new(""),
            path("content/helper.dll"),
            RetargetExclusionReason::ExecutableOrScript
        )
        .is_err());
        assert!(plan()
            .with_policy_exclusions(vec![file("model", "content/helper.dll")])
            .is_err());
        assert!(plan()
            .with_policy_exclusions(vec![file("helper", "content/SOURCE.model")])
            .is_err());
        assert!(plan()
            .with_policy_exclusions(vec![
                file("a", "content/helper.dll"),
                file("b", "content/HELPER.dll")
            ])
            .is_err());
        let plan = plan()
            .with_policy_exclusions(vec![file("helper", "content/helper.dll")])
            .unwrap();
        assert!(plan.has_complete_policy_inventory());
        assert_eq!(plan.policy_exclusions().len(), 1);
        assert_eq!(plan.actions().len(), 1);
        let json = serde_json::to_string(&plan).unwrap();
        assert!(!json.contains("helper.dll"));
        assert!(!json.contains("policy_exclusions"));
        assert!(!json.contains("policy_inventory_complete"));
    }
}
