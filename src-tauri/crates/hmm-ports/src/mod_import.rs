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

/// 包内元数据分析产物。
///
/// `metadata` 保持既有语义：`display_name` = manifest 声明 ?? readme 首行，
/// 是"作者是否声明过名称"的判定依据（revision 继承分支依赖它，见导入服务的
/// catalog 保存分支），调用方不得把派生名称（文件名等）回填进去。
/// `manifest_display_name` 单独携带 manifest 显式声明的展示名，供上层把压缩包
/// 文件名插到 readme 之前：文件名是导入者导入前唯一亲自确认过的名称，而
/// readme 首行可能是教程、致谢或广告，只配作展示名的末端来源。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModPackageMetadataAnalysis {
    pub metadata: ModPackageMetadata,
    pub manifest_display_name: Option<String>,
}

pub trait ModPackageMetadataAnalyzer: Send + Sync {
    fn analyze_metadata(
        &self,
        package_id: &str,
        sandbox_root: &Path,
    ) -> Result<ModPackageMetadataAnalysis>;
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

/// 扫描安装文件失败的原因。
///
/// 刻意做成枚举而不是 `anyhow::Error`：调用方需要**区分**原因，才能把
/// 「包内有多个 nativePC」这类可操作的失败如实告诉玩家，而不是笼统地
/// 报一句「无法读取导入文件」（#284 review 发现的 R1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModPackageInstallFileScanError {
    /// 沙箱无法读取（IO 失败、路径不是目录等）。
    Unavailable,
    /// 沙箱里出现不受支持的条目，例如符号链接或目录联接。
    UnsupportedEntry,
    /// 目录层级超过扫描深度上限。
    DepthLimitExceeded,
    /// 包内有多个 `nativePC`（合集包）**且玩家还没做出选择**。
    ///
    /// 这不是「坏包」，而是**需要玩家自己做决定**——静默挑一个会写入他没预期的
    /// 文件。调用方应当把它呈现成可操作的提示，而不是当成错误。
    ///
    /// `#354` 切片 D2 起，玩家选定之后（[`ModPackageContentRootRepository`]）这一档就不再
    /// 产生：选择被记录下来，扫描按选定的根算目标路径。
    AmbiguousContentRoot,
    /// 记录在案的内容根**已经不是这个包的合法候选**。
    ///
    /// 失败关闭而不是退回自动解析：退回等于「玩家选了 A，我们装到 B」，而这类错误装完不报错、
    /// 只是文件落在别处，属于最难发现的一类。调用方应当提示玩家重新选择。
    StaleContentRootChoice,
}

impl std::fmt::Display for ModPackageInstallFileScanError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "imported mod sandbox files are unavailable",
            Self::UnsupportedEntry => "imported mod sandbox contains an unsupported entry",
            Self::DepthLimitExceeded => {
                "imported mod sandbox exceeds the install file scan depth limit"
            }
            Self::AmbiguousContentRoot => {
                "imported mod package contains more than one nativePC directory"
            }
            Self::StaleContentRootChoice => {
                "the recorded content root is no longer a candidate of this package"
            }
        })
    }
}

impl std::error::Error for ModPackageInstallFileScanError {}

impl ModPackageInstallFileScanError {
    /// 稳定错误码，供审计与前端取词使用；不含路径等敏感信息。
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unavailable => "imported_mod_file_scan_unavailable",
            Self::UnsupportedEntry => "imported_mod_file_scan_unsupported_entry",
            Self::DepthLimitExceeded => "imported_mod_file_scan_depth_limit_exceeded",
            Self::AmbiguousContentRoot => "imported_mod_file_scan_ambiguous_content_root",
            Self::StaleContentRootChoice => "imported_mod_file_scan_stale_content_root_choice",
        }
    }
}

/// 玩家为某个**包**选定的内容根（`#354` 切片 D2）。
///
/// 按 `package_id` 而不是 `mod_id` / `profile_id` 键：内容根是这个解压包的**物理属性**
/// ——同一份沙箱内容只有一种布局，与哪个 profile 要装它无关。
///
/// 值是**沙箱根相对**的正斜杠路径，空串表示沙箱根本身；与
/// [`ModPackageContentRoot`] 的表示一致。
pub trait ModPackageContentRootRepository: Send + Sync {
    fn load_content_root(&self, package_id: &str) -> Result<Option<String>>;
    fn save_content_root(&self, package_id: &str, content_root: &str) -> Result<()>;
    fn clear_content_root(&self, package_id: &str) -> Result<()>;
}

/// 没有任何记录的仓储。给不需要这项能力的装配点用（例如只读的外部扫描工具链），
/// 行为与 D2 之前完全一致。
pub struct NoStoredContentRoot;

impl ModPackageContentRootRepository for NoStoredContentRoot {
    fn load_content_root(&self, _package_id: &str) -> Result<Option<String>> {
        Ok(None)
    }

    fn save_content_root(&self, _package_id: &str, _content_root: &str) -> Result<()> {
        anyhow::bail!("content root selection is not supported by this repository")
    }

    fn clear_content_root(&self, _package_id: &str) -> Result<()> {
        anyhow::bail!("content root selection is not supported by this repository")
    }
}

/// 玩家在某个**包**里勾掉的文件（`#354` 切片 D3）。
///
/// # 为什么存「排除集合」而不是「包含集合」
///
/// 两个理由，都不是风格问题：
///
/// 1. **默认全选必须是空集合。** 没有记录 ⇒ 排除集合为空 ⇒ 计划一个字节不变。用包含集合的话
///    「没有记录」与「一个都不装」在表示上撞车，得再造一个哨兵值来区分。
/// 2. **包重新解压出新文件时要优雅降级。** 排除集合让没见过的新文件**照常安装**（作者补了个
///    贴图，玩家自然想要）；包含集合会让它们**静默不装**——而「少装了一个文件」装完不报错，
///    正是最难发现的那一类。
///
/// 元素是 `package_file_id`（沙箱根相对路径）。集合里有而包里没有的条目是**无害的**：
/// 它只是不再命中任何文件。这一点与内容根不同——那边的陈旧值会让路径从错误的根起算，
/// 所以必须失败关闭。
pub trait ModPackageFileSelectionRepository: Send + Sync {
    fn load_excluded_files(&self, package_id: &str) -> Result<Vec<String>>;
    fn save_excluded_files(&self, package_id: &str, excluded: &[String]) -> Result<()>;
    fn clear_excluded_files(&self, package_id: &str) -> Result<()>;
}

/// 不记录任何勾选的仓储：行为与 D3 之前完全一致。
pub struct NoStoredFileSelection;

impl ModPackageFileSelectionRepository for NoStoredFileSelection {
    fn load_excluded_files(&self, _package_id: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn save_excluded_files(&self, _package_id: &str, _excluded: &[String]) -> Result<()> {
        anyhow::bail!("file selection is not supported by this repository")
    }

    fn clear_excluded_files(&self, _package_id: &str) -> Result<()> {
        anyhow::bail!("file selection is not supported by this repository")
    }
}

pub trait ModPackageInstallFileScanner: Send + Sync {
    fn scan_install_files(
        &self,
        request: ModPackageInstallFileScanRequest<'_>,
    ) -> Result<Vec<ModPackageInstallFile>, ModPackageInstallFileScanError>;
}

/// 包内容树的一条目：沙箱里的一个真实文件。
///
/// 与 [`ModPackageInstallFile`] 的区别是**覆盖面**，不是格式：那一条只列内容根之下的文件，
/// 而且要求内容根唯一——多个 `nativePC` 时 [`ModPackageInstallFileScanner`] 直接返回
/// [`ModPackageInstallFileScanError::AmbiguousContentRoot`]，一个文件都拿不到。
///
/// 玩家要在界面里挑内容根、挑装哪些文件，前提是先能**看见整包**。所以这里列沙箱内全部
/// 文件、不做任何按内容根的取舍，并把内容根解析结果如实带出去，分档交给上层。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModPackageContentEntry {
    /// 沙箱根相对路径，正斜杠分隔。与 [`ModPackageInstallFile::package_file_id`] 同源同形，
    /// 因此可以直接用作读取文件的 `package_file_id`。
    pub package_file_id: String,
    pub size_bytes: u64,
}

/// 内容根解析结果的端口形态。
///
/// 路径一律是**沙箱根相对**的正斜杠字符串，空串表示沙箱根本身——这份结果要原样穿到前端，
/// 用 `PathBuf` 会把宿主的绝对路径与分隔符带出去。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModPackageContentRoot {
    /// 沙箱内没有 `nativePC`：回退为沙箱根。
    Fallback,
    /// 恰好一个 `nativePC`：内容根是它**所在的目录**。
    Single(String),
    /// 多个 `nativePC`（合集包）。
    ///
    /// **不替玩家挑一个**——静默挑会写入他没预期的文件。候选如实列出，由玩家决定。
    Ambiguous(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModPackageContents {
    pub entries: Vec<ModPackageContentEntry>,
    /// 本次扫描**实际生效**的内容根。
    pub content_root: ModPackageContentRoot,
    /// 这个包**允许**被选作内容根的全部目录，与当前选了哪个无关。
    ///
    /// 与 [`Self::content_root`] 分开的理由：玩家选定之后 `content_root` 会收敛成
    /// `Single(选中的那个)`，若候选也跟着消失，他就**改不了主意**了。这份清单同时是
    /// 设置选择时的白名单——界面能选的与扫描认的出自同一处，不会分叉。
    pub candidates: Vec<String>,
    /// 玩家勾掉的 `package_file_id`（`#354` 切片 D3）。
    ///
    /// **整包仍然逐条列在 [`Self::entries`] 里**——勾掉不等于看不见，否则玩家没法勾回来。
    pub excluded_files: Vec<String>,
}

pub struct ModPackageContentScanRequest<'a> {
    pub package_id: &'a str,
    pub sandbox_root: &'a Path,
}

pub trait ModPackageContentScanner: Send + Sync {
    /// 列出整包内容。
    ///
    /// 复用 [`ModPackageInstallFileScanError`] 是因为两个方法共用同一套沙箱防御
    /// （符号链接、深度上限、不可读），错误码也该一致。但本方法**永远不会返回**
    /// [`ModPackageInstallFileScanError::AmbiguousContentRoot`]：内容根有歧义正是它要
    /// 如实报告的状态之一，而不是失败。
    fn scan_package_contents(
        &self,
        request: ModPackageContentScanRequest<'_>,
    ) -> Result<ModPackageContents, ModPackageInstallFileScanError>;
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

    /// 删除 logical Mod 与其全部 revision，返回被删 revision 的 package_id 列表，
    /// 供调用方回收 per-package 存储（提取沙盒内容、缩略图）。
    ///
    /// 实现必须在一次一致的变更里删除 logical Mod 行与全部 `mod_id` 匹配的
    /// revision 行——只删 mod 行会让目录校验（每个 revision 必须解析到既有
    /// logical Mod）失败。
    fn remove_mod_with_revisions(&self, mod_id: &ModId) -> Result<Vec<String>> {
        let revisions = self.list_revisions(mod_id)?;
        let package_ids = revisions
            .iter()
            .map(|revision| revision.package_id.clone())
            .collect();
        self.remove_analysis(mod_id.as_str())?;
        Ok(package_ids)
    }

    /// 删除一个 logical Mod 的分析行（及其投影出的全部 revision）。
    /// 默认不支持：不能安全删除的仓储必须 fail closed。
    fn remove_analysis(&self, _mod_id: &str) -> Result<()> {
        anyhow::bail!("mod removal is not supported by this repository")
    }
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
