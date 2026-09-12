use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetargetFilePreviewDto {
    pub file_id: String,
    pub source_id: Option<String>,
    pub source_path: String,
    pub installed_path: Option<String>,
    pub target_path: Option<String>,
    pub disposition: hmm_core::RetargetFileDisposition,
    pub reason: hmm_core::RetargetFileReason,
    pub change: Option<hmm_core::ReinstallTargetClass>,
}

impl From<hmm_app::RetargetFilePreview> for RetargetFilePreviewDto {
    fn from(file: hmm_app::RetargetFilePreview) -> Self {
        Self {
            file_id: file.effect.package_file_id.as_str().to_owned(),
            source_id: file.effect.source_id.map(|id| id.as_str().to_owned()),
            source_path: file.effect.source_path.as_str().to_owned(),
            installed_path: file.installed_path.map(|path| path.as_str().to_owned()),
            target_path: file.effect.target_path.map(|path| path.as_str().to_owned()),
            disposition: file.effect.disposition,
            reason: file.effect.reason,
            change: file.change,
        }
    }
}

impl From<hmm_core::RetargetFileEffect> for RetargetFilePreviewDto {
    fn from(effect: hmm_core::RetargetFileEffect) -> Self {
        hmm_app::RetargetFilePreview::from(effect).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        InstallTargetPath, PackageFileId, ReinstallTargetClass, RetargetFileDisposition,
        RetargetFileEffect, RetargetFileReason,
    };

    #[test]
    fn file_preview_projects_controlled_paths_and_stable_dispositions() {
        let path = InstallTargetPath::parse("nativePC/plugins/fixture.dll", ["nativePC"]).unwrap();
        let dto = RetargetFilePreviewDto::from(hmm_app::RetargetFilePreview {
            effect: RetargetFileEffect {
                package_file_id: PackageFileId::new("fixture-file"),
                source_id: None,
                source_path: path.clone(),
                target_path: Some(path.clone()),
                disposition: RetargetFileDisposition::InstalledAttachmentRetained,
                reason: RetargetFileReason::InstalledAttachment,
            },
            installed_path: Some(path),
            change: Some(ReinstallTargetClass::Retained),
        });
        let value = serde_json::to_value(dto).unwrap();
        assert_eq!(value["fileId"], "fixture-file");
        assert_eq!(value["sourcePath"], "nativePC/plugins/fixture.dll");
        assert_eq!(value["installedPath"], value["targetPath"]);
        assert_eq!(value["disposition"], "installed_attachment_retained");
        assert_eq!(value["change"], "retained");
        assert!(value.get("gameRoot").is_none());
        assert!(value.get("stagingPath").is_none());
    }
}
