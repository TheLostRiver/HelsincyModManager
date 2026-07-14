use anyhow::Result;
use hmm_core::{ModId, ModRevisionId, PreviewImageRejectionReason};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedModPackage {
    pub package_id: String,
    pub sandbox_root: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModPackageMetadata {
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub dependencies: Vec<String>,
}

pub struct ModImportPackagePrepareRequest<'a> {
    pub task_id: &'a str,
    pub archive_path: &'a Path,
    pub cancellation_token: &'a dyn crate::CancellationToken,
}

pub trait ModImportPackagePreparer: Send + Sync {
    fn prepare_package(
        &self,
        request: ModImportPackagePrepareRequest<'_>,
    ) -> Result<PreparedModPackage>;
}

pub trait ModPackageMetadataAnalyzer: Send + Sync {
    fn analyze_metadata(&self, package_id: &str, sandbox_root: &Path)
        -> Result<ModPackageMetadata>;
}

pub trait ModImportSandboxLocator: Send + Sync {
    fn sandbox_root_for_package(&self, package_id: &str) -> Result<PathBuf>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModPackageInstallFile {
    pub package_file_id: String,
    pub target_path: String,
}

pub struct ModPackageInstallFileScanRequest<'a> {
    pub package_id: &'a str,
    pub sandbox_root: &'a Path,
}

pub trait ModPackageInstallFileScanner: Send + Sync {
    fn scan_install_files(
        &self,
        request: ModPackageInstallFileScanRequest<'_>,
    ) -> Result<Vec<ModPackageInstallFile>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredModImportAnalysis {
    pub mod_id: String,
    pub task_id: String,
    pub package_id: String,
    pub display_name: String,
    #[serde(default)]
    pub metadata: StoredModPackageMetadata,
    #[serde(default = "default_preview_image")]
    pub preview_image: StoredImportPreviewImage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredLogicalMod {
    pub mod_id: ModId,
    pub origin_revision_id: ModRevisionId,
    pub display_revision_id: ModRevisionId,
    pub origin_provenance: StoredModOriginProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "snake_case"
)]
pub enum StoredModOriginProvenance {
    Imported,
    MigratedV1 {
        legacy_mod_id: String,
        legacy_package_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredModRevision {
    pub revision_id: ModRevisionId,
    pub mod_id: ModId,
    pub import_task_id: String,
    pub package_id: String,
    pub display_name: String,
    #[serde(default)]
    pub metadata: StoredModPackageMetadata,
    #[serde(default = "default_preview_image")]
    pub preview_image: StoredImportPreviewImage,
}

impl StoredModRevision {
    pub fn as_analysis(&self) -> StoredModImportAnalysis {
        StoredModImportAnalysis {
            mod_id: self.mod_id.as_str().to_owned(),
            task_id: self.import_task_id.clone(),
            package_id: self.package_id.clone(),
            display_name: self.display_name.clone(),
            metadata: self.metadata.clone(),
            preview_image: self.preview_image.clone(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredModPackageMetadata {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum StoredImportPreviewImage {
    Thumbnail {
        thumbnail_url: String,
        width: u32,
        height: u32,
        content_hash: String,
        #[serde(default = "default_preview_thumbnail_variant")]
        variant: String,
    },
    Fallback {
        reason: PreviewImageRejectionReason,
    },
}

fn default_preview_thumbnail_variant() -> String {
    "preview-768".to_owned()
}

fn default_preview_image() -> StoredImportPreviewImage {
    StoredImportPreviewImage::Fallback {
        reason: PreviewImageRejectionReason::Missing,
    }
}

pub trait ModImportResultRepository: Send + Sync {
    fn save_new_mod(
        &self,
        logical_mod: &StoredLogicalMod,
        revision: &StoredModRevision,
    ) -> Result<()> {
        anyhow::ensure!(
            logical_mod.mod_id == revision.mod_id
                && logical_mod.origin_revision_id == revision.revision_id
                && logical_mod.display_revision_id == revision.revision_id,
            "logical Mod and origin revision do not match"
        );
        self.save_analysis(&revision.as_analysis())
    }

    fn append_revision(&self, _revision: &StoredModRevision) -> Result<()> {
        anyhow::bail!("revision append is not supported by this repository")
    }

    fn get_mod(&self, mod_id: &ModId) -> Result<Option<StoredLogicalMod>> {
        Ok(self.get_analysis(mod_id.as_str())?.map(|analysis| {
            let revision_id = ModRevisionId::new(analysis.package_id);
            StoredLogicalMod {
                mod_id: ModId::new(analysis.mod_id),
                origin_revision_id: revision_id.clone(),
                display_revision_id: revision_id,
                origin_provenance: StoredModOriginProvenance::Imported,
            }
        }))
    }

    fn list_mods(&self) -> Result<Vec<StoredLogicalMod>> {
        Ok(self
            .list_analysis()?
            .into_iter()
            .map(|analysis| {
                let revision_id = ModRevisionId::new(analysis.package_id);
                StoredLogicalMod {
                    mod_id: ModId::new(analysis.mod_id),
                    origin_revision_id: revision_id.clone(),
                    display_revision_id: revision_id,
                    origin_provenance: StoredModOriginProvenance::Imported,
                }
            })
            .collect())
    }

    fn get_revision(&self, revision_id: &ModRevisionId) -> Result<Option<StoredModRevision>> {
        Ok(self
            .list_analysis()?
            .into_iter()
            .find(|analysis| analysis.package_id == revision_id.as_str())
            .map(|analysis| StoredModRevision {
                revision_id: revision_id.clone(),
                mod_id: ModId::new(analysis.mod_id),
                import_task_id: analysis.task_id,
                package_id: analysis.package_id,
                display_name: analysis.display_name,
                metadata: analysis.metadata,
                preview_image: analysis.preview_image,
            }))
    }

    fn list_revisions(&self, mod_id: &ModId) -> Result<Vec<StoredModRevision>> {
        Ok(self
            .get_analysis(mod_id.as_str())?
            .into_iter()
            .map(|analysis| StoredModRevision {
                revision_id: ModRevisionId::new(&analysis.package_id),
                mod_id: ModId::new(analysis.mod_id),
                import_task_id: analysis.task_id,
                package_id: analysis.package_id,
                display_name: analysis.display_name,
                metadata: analysis.metadata,
                preview_image: analysis.preview_image,
            })
            .collect())
    }

    // Compatibility projection for existing library, install and preview-image consumers.
    fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> Result<()>;
    fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>>;
    fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>>;
}

pub struct DiagnosticPackageEntry<'a> {
    pub name: &'a str,
    pub bytes: &'a [u8],
}

pub struct DiagnosticPackageExportRequest<'a> {
    pub file_name: &'a str,
    pub entries: &'a [DiagnosticPackageEntry<'a>],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticPackageExportResult {
    pub export_id: String,
    pub file_name: String,
    pub size_bytes: u64,
}

pub trait DiagnosticPackageExporter: Send + Sync {
    fn export_package(
        &self,
        request: DiagnosticPackageExportRequest<'_>,
    ) -> Result<DiagnosticPackageExportResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revision_catalog_records_serialize_distinct_identity_and_provenance() {
        let logical_mod = StoredLogicalMod {
            mod_id: ModId::new("mod-a"),
            origin_revision_id: ModRevisionId::new("revision-v1"),
            display_revision_id: ModRevisionId::new("revision-v2"),
            origin_provenance: StoredModOriginProvenance::MigratedV1 {
                legacy_mod_id: "legacy-mod".to_owned(),
                legacy_package_id: "legacy-package".to_owned(),
            },
        };
        let revision = StoredModRevision {
            revision_id: ModRevisionId::new("revision-v2"),
            mod_id: ModId::new("mod-a"),
            import_task_id: "task-v2".to_owned(),
            package_id: "package-v2".to_owned(),
            display_name: "Candidate".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: default_preview_image(),
        };

        assert_eq!(
            serde_json::to_value(&logical_mod).expect("serialize logical Mod"),
            serde_json::json!({
                "mod_id": "mod-a",
                "origin_revision_id": "revision-v1",
                "display_revision_id": "revision-v2",
                "origin_provenance": {
                    "kind": "migrated_v1",
                    "legacy_mod_id": "legacy-mod",
                    "legacy_package_id": "legacy-package"
                }
            })
        );
        assert_eq!(
            serde_json::to_value(&revision).expect("serialize revision"),
            serde_json::json!({
                "revision_id": "revision-v2",
                "mod_id": "mod-a",
                "import_task_id": "task-v2",
                "package_id": "package-v2",
                "display_name": "Candidate",
                "metadata": {
                    "version": null,
                    "author": null,
                    "category": null,
                    "tags": [],
                    "dependencies": []
                },
                "preview_image": {
                    "kind": "fallback",
                    "reason": "missing"
                }
            })
        );
    }
}
