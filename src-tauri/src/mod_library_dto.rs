use crate::dto::{CategoryLabelDto, InstallManifestStatusDto, PreviewImageDto};
use hmm_app::{
    InstallManifestStatus, InstallManifestStatusSummary, ModLibraryItem, ModLibraryPage,
    ModLibraryPageItem, ModLibraryStatus,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLibraryItemDto {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_label: Option<String>,
    pub size_label: String,
    pub status: ModInstallStatusDto,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_summary: Option<ModLibraryInstallSummaryDto>,
    pub category_labels: Vec<CategoryLabelDto>,
    pub preview_image: PreviewImageDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLibraryInstallSummaryDto {
    pub status: InstallManifestStatusDto,
    pub managed_file_count: usize,
    pub backup_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModInstallStatusDto {
    Disabled,
    NotInstalled,
    Installed,
    CommittedCleanupPending,
    CleanupPending,
    RollbackRequired,
    RepairRequired,
    Unknown,
}

impl From<ModLibraryItem> for ModLibraryItemDto {
    fn from(item: ModLibraryItem) -> Self {
        Self {
            id: item.id,
            name: item.name,
            author: item.author,
            version_label: item.version_label,
            size_label: item.size_label,
            status: item.status.into(),
            install_summary: None,
            category_labels: item
                .category_labels
                .into_iter()
                .map(|label| CategoryLabelDto {
                    name: label.name,
                    color: label.color,
                })
                .collect(),
            preview_image: item.preview_image.into(),
        }
    }
}

impl From<ModLibraryPageItem> for ModLibraryItemDto {
    fn from(page_item: ModLibraryPageItem) -> Self {
        let mut item: Self = page_item.item.into();
        if let Some(summary) = page_item.install_summary {
            item.status = summary.status.into();
            item.install_summary = Some(summary.into());
        }
        item
    }
}

impl From<InstallManifestStatusSummary> for ModLibraryInstallSummaryDto {
    fn from(summary: InstallManifestStatusSummary) -> Self {
        Self {
            status: summary.status.into(),
            managed_file_count: summary.managed_file_count,
            backup_count: summary.backup_count,
        }
    }
}

impl From<ModLibraryStatus> for ModInstallStatusDto {
    fn from(status: ModLibraryStatus) -> Self {
        match status {
            ModLibraryStatus::Disabled => Self::Disabled,
        }
    }
}

impl From<InstallManifestStatus> for ModInstallStatusDto {
    fn from(status: InstallManifestStatus) -> Self {
        match status {
            InstallManifestStatus::NotInstalled => Self::NotInstalled,
            InstallManifestStatus::Installed => Self::Installed,
            InstallManifestStatus::CommittedCleanupPending => Self::CommittedCleanupPending,
            InstallManifestStatus::CleanupPending => Self::CleanupPending,
            InstallManifestStatus::RollbackRequired => Self::RollbackRequired,
            InstallManifestStatus::RepairRequired => Self::RepairRequired,
            InstallManifestStatus::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryModLibraryRequestDto {
    #[serde(default)]
    pub profile_context: Option<ModLibraryProfileContextDto>,
    pub search: String,
    pub filter: ModLibraryFilterDto,
    pub sort: String,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModLibraryProfileContextDto {
    pub game_id: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModLibraryFilterDto {
    pub kind: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLibraryPageDto {
    pub items: Vec<ModLibraryItemDto>,
    pub page: u64,
    pub page_size: u32,
    pub library_total: usize,
    pub matching_total: usize,
}

impl From<ModLibraryPage> for ModLibraryPageDto {
    fn from(page: ModLibraryPage) -> Self {
        Self {
            items: page.items.into_iter().map(Into::into).collect(),
            page: page.page,
            page_size: page.page_size,
            library_total: page.library_total,
            matching_total: page.matching_total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_app::ImportPreviewImage;
    use hmm_core::{ModId, PreviewImageRejectionReason, ProfileId};
    use std::collections::BTreeSet;

    #[test]
    fn request_deserializes_camel_case_query_without_paths() {
        let request: QueryModLibraryRequestDto = serde_json::from_value(serde_json::json!({
            "profileContext": {
                "gameId": "mhw",
                "profileId": "default"
            },
            "search": "fatalis",
            "filter": {
                "kind": "status",
                "status": "installed"
            },
            "sort": "name_asc",
            "page": 2,
            "pageSize": 24
        }))
        .expect("deserialize query request");

        assert_eq!(
            request.profile_context.expect("profile context").game_id,
            "mhw"
        );
        assert_eq!(request.search, "fatalis");
        assert_eq!(request.filter.kind, "status");
        assert_eq!(request.filter.status.as_deref(), Some("installed"));
        assert_eq!(request.page, 2);
        assert_eq!(request.page_size, 24);
    }

    #[test]
    fn request_rejects_unreviewed_path_fields() {
        let requests = [
            serde_json::json!({
                "search": "",
                "filter": { "kind": "all" },
                "sort": "name_asc",
                "page": 1,
                "pageSize": 24,
                "gamePath": "C:/Games/MHW"
            }),
            serde_json::json!({
                "profileContext": {
                    "gameId": "mhw",
                    "profileId": "default",
                    "manifestPath": "manifest.json"
                },
                "search": "",
                "filter": { "kind": "all" },
                "sort": "name_asc",
                "page": 1,
                "pageSize": 24
            }),
            serde_json::json!({
                "search": "",
                "filter": {
                    "kind": "all",
                    "cachePath": "cache/mods"
                },
                "sort": "name_asc",
                "page": 1,
                "pageSize": 24
            }),
        ];

        for request in requests {
            let error = serde_json::from_value::<QueryModLibraryRequestDto>(request)
                .expect_err("unknown path field must be rejected");
            assert!(error.to_string().contains("unknown field"));
        }
    }

    #[test]
    fn item_omits_absent_optional_metadata() {
        let item = ModLibraryItemDto::from(ModLibraryItem {
            id: "mod-a".to_owned(),
            name: "Armor A".to_owned(),
            author: None,
            version_label: None,
            size_label: "导入完成".to_owned(),
            status: ModLibraryStatus::Disabled,
            category_labels: Vec::new(),
            preview_image: ImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
        });

        let value = serde_json::to_value(item).expect("serialize mod library item");
        let object = value.as_object().expect("item object");

        assert!(!object.contains_key("author"));
        assert!(!object.contains_key("versionLabel"));
    }

    #[test]
    fn page_serializes_install_summary_without_path_fields() {
        let page: ModLibraryPageDto = ModLibraryPage {
            items: vec![ModLibraryPageItem {
                item: ModLibraryItem {
                    id: "mod-a".to_owned(),
                    name: "Armor A".to_owned(),
                    author: Some("Hunter".to_owned()),
                    version_label: Some("1.0.0".to_owned()),
                    size_label: "导入完成".to_owned(),
                    status: ModLibraryStatus::Disabled,
                    category_labels: Vec::new(),
                    preview_image: ImportPreviewImage::Fallback {
                        reason: PreviewImageRejectionReason::Missing,
                    },
                },
                install_summary: Some(InstallManifestStatusSummary {
                    profile_id: ProfileId::new("default"),
                    mod_id: ModId::new("mod-a"),
                    status: InstallManifestStatus::Installed,
                    managed_file_count: 2,
                    backup_count: 1,
                    installed_revision_id: None,
                }),
            }],
            page: 1,
            page_size: 24,
            library_total: 1,
            matching_total: 1,
        }
        .into();

        let value = serde_json::to_value(page).expect("serialize mod library page");

        assert_eq!(value["items"][0]["status"], "installed");
        assert_eq!(value["items"][0]["installSummary"]["status"], "installed");
        assert_eq!(value["items"][0]["installSummary"]["managedFileCount"], 2);
        assert_eq!(value["items"][0]["installSummary"]["backupCount"], 1);
        assert_eq!(value["pageSize"], 24);
        assert_eq!(value["libraryTotal"], 1);
        assert_eq!(value["matchingTotal"], 1);

        assert_eq!(
            value
                .as_object()
                .expect("page object")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["items", "libraryTotal", "matchingTotal", "page", "pageSize",])
        );
        assert_eq!(
            value["items"][0]
                .as_object()
                .expect("item object")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "author",
                "categoryLabels",
                "id",
                "installSummary",
                "name",
                "previewImage",
                "sizeLabel",
                "status",
                "versionLabel",
            ])
        );
        assert_eq!(
            value["items"][0]["installSummary"]
                .as_object()
                .expect("install summary object")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["backupCount", "managedFileCount", "status"])
        );

        let serialized = value.to_string().to_lowercase();
        for forbidden in [
            "archivepath",
            "sandboxpath",
            "cachepath",
            "manifestpath",
            "gamepath",
            "backupref",
            "rawpath",
        ] {
            assert!(!serialized.contains(forbidden), "contains {forbidden}");
        }
    }
}
