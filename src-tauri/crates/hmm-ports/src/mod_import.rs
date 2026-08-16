use anyhow::Result;
use hmm_core::{
    ExternalImportProvenance, ModId, ModRevisionId, PackageFileId, PreviewImageRejectionReason,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

pub const MOD_IMPORT_UPSERT_CHUNK_SIZE: usize = 200;
pub const MOD_IMPORT_UPSERT_MAX_ENTRIES: usize = 10_000;

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

pub trait ModImportArchiveReader: Read + Seek {}

impl<T> ModImportArchiveReader for T where T: Read + Seek + ?Sized {}

/// Uses an already-open archive handle. This lets an infrastructure adapter retain a no-follow
/// capability chain when the archive was generated in an app-private temporary directory.
pub struct ModImportPackagePrepareReaderRequest<'a> {
    pub task_id: &'a str,
    pub archive: &'a mut dyn ModImportArchiveReader,
    pub cancellation_token: &'a dyn crate::CancellationToken,
}

pub trait ModImportPackagePreparer: Send + Sync {
    fn prepare_package(
        &self,
        request: ModImportPackagePrepareRequest<'_>,
    ) -> Result<PreparedModPackage>;

    fn prepare_package_from_reader(
        &self,
        _request: ModImportPackagePrepareReaderRequest<'_>,
    ) -> Result<PreparedModPackage> {
        anyhow::bail!("preparing an already-open Mod import archive is not supported")
    }
}

pub trait ModPackageMetadataAnalyzer: Send + Sync {
    fn analyze_metadata(&self, package_id: &str, sandbox_root: &Path)
        -> Result<ModPackageMetadata>;
}

pub trait ModImportSandboxLocator: Send + Sync {
    fn sandbox_root_for_package(&self, package_id: &str) -> Result<PathBuf>;

    /// Removes an unpersisted task-scoped sandbox by its opaque package identity. Implementations
    /// must keep the operation inside their controlled sandbox root.
    fn cleanup_sandbox_for_package(&self, _package_id: &str) -> Result<()> {
        anyhow::bail!("sandbox cleanup is unavailable")
    }
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

pub struct ModPackageInstallFileReadRequest<'a> {
    pub package_id: &'a str,
    pub sandbox_root: &'a Path,
    pub package_file_id: &'a PackageFileId,
    pub max_bytes: u64,
}

pub trait ModPackageInstallFileReader: Send + Sync {
    fn read_install_file(&self, request: ModPackageInstallFileReadRequest<'_>) -> Result<Vec<u8>>;
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
    ExternalImport {
        provenance: ExternalImportProvenance,
    },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportCatalogUpsert {
    pub logical_mod: StoredLogicalMod,
    pub revision: StoredModRevision,
}

/// Captures the authority-side decision required before an external import may reuse a display
/// name already owned by another logical Mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModImportExternalDisplayNameAdmission {
    RequireUnique,
    AllowExisting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModImportExternalCatalogUpsert {
    pub upsert: ModImportCatalogUpsert,
    pub display_name_admission: ModImportExternalDisplayNameAdmission,
}

/// A single authoritative catalog read for callers that need both logical Mod provenance and
/// display revisions. Implementations should avoid per-entry reloads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModImportCatalogSnapshot {
    pub logical_mods: Vec<StoredLogicalMod>,
    pub revisions: Vec<StoredModRevision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModImportExternalCatalogAdmissionError {
    ContentAlreadyImported {
        content_fingerprint: String,
        existing_mod_id: ModId,
    },
    DisplayNameCollision {
        display_name: String,
    },
}

impl std::fmt::Display for ModImportExternalCatalogAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContentAlreadyImported { .. } => {
                formatter.write_str("external import content is already present")
            }
            Self::DisplayNameCollision { .. } => {
                formatter.write_str("external import display name is already present")
            }
        }
    }
}

impl std::error::Error for ModImportExternalCatalogAdmissionError {}

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

    /// Upserts a bounded batch without guaranteeing call-wide atomicity.
    ///
    /// Implementations may persist chunks independently. If a later chunk fails,
    /// earlier successful chunks remain durable; callers must not assume
    /// all-or-nothing behavior. Callers must support idempotent retries and mark
    /// dependent projections dirty until they can be rebuilt from authoritative
    /// repository state. The default implementation accepts only an empty batch
    /// and fails closed for non-empty input.
    fn upsert_many(&self, upserts: &[ModImportCatalogUpsert]) -> Result<()> {
        if upserts.is_empty() {
            return Ok(());
        }
        anyhow::bail!("batch Mod import upsert is not supported by this repository")
    }

    /// Persists external-import entries after authority-side content and display-name admission.
    /// Generic repositories retain compatibility by delegating to `upsert_many`; the production
    /// JSON authority overrides this method while holding its catalog lock.
    fn upsert_external_import_many(
        &self,
        upserts: &[ModImportExternalCatalogUpsert],
    ) -> Result<()> {
        let plain_upserts = upserts
            .iter()
            .map(|upsert| upsert.upsert.clone())
            .collect::<Vec<_>>();
        self.upsert_many(&plain_upserts)
    }

    /// Returns a consistent catalog snapshot. Implementations with a single catalog backing
    /// should override this rather than composing repeated point reads.
    fn catalog_snapshot(&self) -> Result<ModImportCatalogSnapshot> {
        let logical_mods = self.list_mods()?;
        let mut revisions = Vec::with_capacity(logical_mods.len());
        for logical_mod in &logical_mods {
            if let Some(revision) = self.get_revision(&logical_mod.display_revision_id)? {
                revisions.push(revision);
            }
        }
        Ok(ModImportCatalogSnapshot {
            logical_mods,
            revisions,
        })
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

    struct CompatibilityOnlyRepository;

    impl ModImportResultRepository for CompatibilityOnlyRepository {
        fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> Result<()> {
            Ok(())
        }

        fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
            Ok(Vec::new())
        }

        fn get_analysis(&self, _mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
            Ok(None)
        }
    }

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

    #[test]
    fn compatibility_repository_rejects_non_empty_batch_upsert() {
        let repository = CompatibilityOnlyRepository;
        let revision_id = ModRevisionId::new("revision-v1");
        let error = repository
            .upsert_many(&[ModImportCatalogUpsert {
                logical_mod: StoredLogicalMod {
                    mod_id: ModId::new("mod-a"),
                    origin_revision_id: revision_id.clone(),
                    display_revision_id: revision_id.clone(),
                    origin_provenance: StoredModOriginProvenance::Imported,
                },
                revision: StoredModRevision {
                    revision_id,
                    mod_id: ModId::new("mod-a"),
                    import_task_id: "task-v1".to_owned(),
                    package_id: "package-v1".to_owned(),
                    display_name: "Mod A".to_owned(),
                    metadata: StoredModPackageMetadata::default(),
                    preview_image: default_preview_image(),
                },
            }])
            .expect_err("compatibility repository must fail closed");

        assert!(error.to_string().contains("not supported"));
        repository
            .upsert_many(&[])
            .expect("empty batch is always a no-op");
    }
}
