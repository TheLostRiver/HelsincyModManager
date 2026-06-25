use hmm_core::{
    FileLayer, InstallFileProvider, InstallPlan, InstallTargetPath, InstallTargetPathError, ModId,
    PackageFileId,
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInstallPlanRequest {
    pub allowed_target_roots: Vec<String>,
    pub files: Vec<InstallPlanFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlanFile {
    pub mod_id: ModId,
    pub package_file_id: PackageFileId,
    pub target_path: String,
    pub layer: FileLayer,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallPlanningError {
    #[error("invalid install target path")]
    InvalidTargetPath {
        package_file_id: PackageFileId,
        source: InstallTargetPathError,
    },
}

#[derive(Debug, Default, Clone)]
pub struct InstallPlanningService;

impl InstallPlanningService {
    pub fn new() -> Self {
        Self
    }

    pub fn build_plan(
        &self,
        request: BuildInstallPlanRequest,
    ) -> Result<InstallPlan, InstallPlanningError> {
        let mut providers = Vec::with_capacity(request.files.len());

        for file in request.files {
            let target_path =
                InstallTargetPath::parse(file.target_path, request.allowed_target_roots.iter())
                    .map_err(|source| InstallPlanningError::InvalidTargetPath {
                        package_file_id: file.package_file_id.clone(),
                        source,
                    })?;

            providers.push(InstallFileProvider::new(
                file.mod_id,
                file.package_file_id,
                target_path,
                file.layer,
            ));
        }

        Ok(InstallPlan::from_providers(providers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{FileLayer, InstallTargetPathError, ModId, PackageFileId};

    fn install_file(
        mod_id: &str,
        package_file_id: &str,
        target_path: &str,
        priority: i32,
    ) -> InstallPlanFile {
        InstallPlanFile {
            mod_id: ModId::new(mod_id),
            package_file_id: PackageFileId::new(package_file_id),
            target_path: target_path.to_owned(),
            layer: FileLayer::new("test", priority),
        }
    }

    #[test]
    fn build_plan_parses_allowed_target_paths_into_core_plan() {
        let service = InstallPlanningService::new();
        let request = BuildInstallPlanRequest {
            allowed_target_roots: vec!["content".to_owned()],
            files: vec![install_file(
                "mod-a",
                "file-a",
                "content/models/player.mod3",
                0,
            )],
        };

        let plan = service
            .build_plan(request)
            .expect("valid request should build an install plan");

        assert!(!plan.has_blocking_conflicts());
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(
            plan.actions[0].target_path.as_str(),
            "content/models/player.mod3"
        );
        assert_eq!(plan.actions[0].provider.mod_id.as_str(), "mod-a");
    }

    #[test]
    fn build_plan_reports_package_file_for_invalid_target_path() {
        let service = InstallPlanningService::new();
        let request = BuildInstallPlanRequest {
            allowed_target_roots: vec!["content".to_owned()],
            files: vec![install_file("mod-a", "file-a", "../outside.bin", 0)],
        };

        let error = service
            .build_plan(request)
            .expect_err("invalid target path should fail planning");

        assert_eq!(
            error,
            InstallPlanningError::InvalidTargetPath {
                package_file_id: PackageFileId::new("file-a"),
                source: InstallTargetPathError::ParentTraversal,
            }
        );
    }

    #[test]
    fn build_plan_preserves_core_conflicts() {
        let service = InstallPlanningService::new();
        let request = BuildInstallPlanRequest {
            allowed_target_roots: vec!["content".to_owned()],
            files: vec![
                install_file("mod-a", "file-a", "content/models/player.mod3", 0),
                install_file("mod-b", "file-b", "content/models/player.mod3", 0),
            ],
        };

        let plan = service
            .build_plan(request)
            .expect("valid paths should build a plan even when conflicts exist");

        assert!(plan.has_blocking_conflicts());
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].providers.len(), 2);
    }
}
