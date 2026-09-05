//! 包内容的只读查询（`#354` 切片 D1）。
//!
//! 玩家要能决定「这个包里哪些文件装、内容根在哪」，前提是先**看得见整包**。既有的
//! [`crate::InstallPlanningService`] 走的是 `scan_install_files`，它只列内容根之下的文件，
//! 而且多个 `nativePC` 时直接失败——那正是要玩家决定的状态，却什么也拿不到。
//!
//! # 报事实，不报合并结论
//!
//! 每个文件带三条**互相独立**的事实：能不能算出目标路径、目标路径可不可安装、有没有命中
//! 本游戏的「绝不安装」清单。**不合并成一个 `will_install`**，因为合并会说谎：
//!
//! MHW 的可执行/脚本拒绝清单目前只作用在**重定向计划**产出的文件上，普通安装链路尚未套用
//! （缺口记在 `hmm-games-mhw` 的 `executable_reject_list.rs` 模块头，`#336` 当时明确划在
//! 范围外）。于是同一个 `.exe` 在两条链路上的归宿不同。合并成单一结论，就必然在其中一条
//! 链路上给出与实际相反的答案；分开报事实，UI 才能如实呈现，也才能在缺口补上后自然收敛。

use hmm_core::{GameId, InstallTargetPath, ModId};
use hmm_ports::{
    GameAdapter, ModImportResultRepository, ModImportSandboxLocator, ModPackageContentRoot,
    ModPackageContentRootRepository, ModPackageContentScanRequest, ModPackageContentScanner,
    ModPackageInstallFileScanError,
};
use std::sync::Arc;
use thiserror::Error;

/// 包内一个文件的分档事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageContentEntry {
    /// 沙箱根相对路径。同时是读取该文件所用的 `package_file_id`。
    pub package_file_id: String,
    pub size_bytes: u64,
    /// 相对**内容根**的路径——也就是它会被装到游戏目录下的哪里。
    ///
    /// `None` 有两种成因，UI 要分开呈现：文件落在内容根之外（包装目录同级的 readme 等），
    /// 或内容根本身还没定（多个 `nativePC`，等玩家挑）。后者靠
    /// [`PackageContentRoot::Ambiguous`] 区分。
    pub target_path: Option<String>,
    /// [`Self::target_path`] 能否落进本游戏允许的安装根。
    pub installable: bool,
    /// 文件名命中本游戏的「绝不安装」清单。
    ///
    /// 这是**事实**不是结论：见模块头，它当前只在重定向链路上被强制执行。
    pub rejected_by_game: bool,
}

/// 内容根解析结果。路径是沙箱根相对的正斜杠字符串，空串表示沙箱根本身。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageContentRoot {
    Fallback,
    Single(String),
    /// 多个 `nativePC`：候选如实列出，等玩家挑（`#354` D2）。
    Ambiguous(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageContents {
    pub entries: Vec<PackageContentEntry>,
    /// 当前**实际生效**的内容根。
    pub content_root: PackageContentRoot,
    /// 允许被选作内容根的全部目录，与当前选了哪个无关——玩家要能改主意（`#354` D2）。
    pub candidates: Vec<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PackageContentsQueryError {
    #[error("imported mod sources are unavailable")]
    SourcesUnavailable,
    #[error("imported mod analysis is unavailable")]
    AnalysisUnavailable,
    #[error("imported mod was not found")]
    ModNotFound { mod_id: ModId },
    #[error("imported mod sandbox is unavailable")]
    SandboxUnavailable,
    #[error("game adapter was not found")]
    GameAdapterNotFound { game_id: GameId },
    #[error("imported mod package contents could not be scanned")]
    ScanFailed(ModPackageInstallFileScanError),
    /// 要设置的内容根不在这个包的候选清单里。
    ///
    /// 在**设置**这一步就拦下，而不是存进去等扫描时才失败关闭：否则玩家会以为选好了，
    /// 直到下一次安装才发现不对。
    #[error("the requested content root is not a candidate of this package")]
    ContentRootNotACandidate,
    #[error("the content root choice could not be persisted")]
    ContentRootChoiceUnavailable,
}

impl PackageContentsQueryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SourcesUnavailable => "package_contents_sources_unavailable",
            Self::AnalysisUnavailable => "package_contents_analysis_unavailable",
            Self::ModNotFound { .. } => "package_contents_mod_not_found",
            Self::SandboxUnavailable => "package_contents_sandbox_unavailable",
            Self::GameAdapterNotFound { .. } => "package_contents_game_adapter_not_found",
            // 扫描侧已有稳定错误码（符号链接、深度上限等），原样透出，不在这里重新发明。
            Self::ScanFailed(error) => error.code(),
            Self::ContentRootNotACandidate => "package_contents_content_root_not_a_candidate",
            Self::ContentRootChoiceUnavailable => {
                "package_contents_content_root_choice_unavailable"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageContentsQueryRequest {
    pub game_id: GameId,
    pub mod_id: ModId,
}

#[derive(Clone)]
pub struct PackageContentsQueryService {
    sources: Option<Arc<PackageContentsSources>>,
}

struct PackageContentsSources {
    result_repository: Arc<dyn ModImportResultRepository>,
    sandbox_locator: Arc<dyn ModImportSandboxLocator>,
    content_scanner: Arc<dyn ModPackageContentScanner>,
    content_root_choices: Arc<dyn ModPackageContentRootRepository>,
    game_adapters: Vec<Arc<dyn GameAdapter>>,
}

impl Default for PackageContentsQueryService {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageContentsQueryService {
    pub fn new() -> Self {
        Self { sources: None }
    }

    pub fn with_imported_mod_sources(
        result_repository: Arc<dyn ModImportResultRepository>,
        sandbox_locator: Arc<dyn ModImportSandboxLocator>,
        content_scanner: Arc<dyn ModPackageContentScanner>,
        content_root_choices: Arc<dyn ModPackageContentRootRepository>,
        game_adapters: Vec<Arc<dyn GameAdapter>>,
    ) -> Self {
        Self {
            sources: Some(Arc::new(PackageContentsSources {
                result_repository,
                sandbox_locator,
                content_scanner,
                content_root_choices,
                game_adapters,
            })),
        }
    }

    pub fn query(
        &self,
        request: PackageContentsQueryRequest,
    ) -> Result<PackageContents, PackageContentsQueryError> {
        let sources = self
            .sources
            .as_ref()
            .ok_or(PackageContentsQueryError::SourcesUnavailable)?;

        let adapter = sources
            .game_adapters
            .iter()
            .find(|adapter| adapter.game_id() == request.game_id)
            .ok_or_else(|| PackageContentsQueryError::GameAdapterNotFound {
                game_id: request.game_id.clone(),
            })?;

        // 与 `build_plan_from_imported_mod` 同一条解析链：mod → analysis → package → sandbox。
        let analysis = sources
            .result_repository
            .get_analysis(request.mod_id.as_str())
            .map_err(|_| PackageContentsQueryError::AnalysisUnavailable)?
            .ok_or_else(|| PackageContentsQueryError::ModNotFound {
                mod_id: request.mod_id.clone(),
            })?;
        let sandbox_root = sources
            .sandbox_locator
            .sandbox_root_for_package(&analysis.package_id)
            .map_err(|_| PackageContentsQueryError::SandboxUnavailable)?;

        let contents = sources
            .content_scanner
            .scan_package_contents(ModPackageContentScanRequest {
                package_id: &analysis.package_id,
                sandbox_root: &sandbox_root,
            })
            .map_err(PackageContentsQueryError::ScanFailed)?;

        let allowed_target_roots = adapter.allowed_install_roots();
        let content_root_prefix = match &contents.content_root {
            ModPackageContentRoot::Fallback => Some(String::new()),
            ModPackageContentRoot::Single(root) => Some(root.clone()),
            // 内容根未定就算不出目标路径。这不是失败——玩家挑完（D2）自然就有了。
            ModPackageContentRoot::Ambiguous(_) => None,
        };

        let entries = contents
            .entries
            .into_iter()
            .map(|entry| {
                let target_path = content_root_prefix
                    .as_deref()
                    .and_then(|prefix| strip_content_root(&entry.package_file_id, prefix));
                let installable = target_path.as_deref().is_some_and(|path| {
                    InstallTargetPath::parse(path, allowed_target_roots.iter()).is_ok()
                });
                let rejected_by_game = entry
                    .package_file_id
                    .rsplit('/')
                    .next()
                    .is_some_and(|file_name| adapter.is_rejected_install_file_name(file_name));

                PackageContentEntry {
                    package_file_id: entry.package_file_id,
                    size_bytes: entry.size_bytes,
                    target_path,
                    installable,
                    rejected_by_game,
                }
            })
            .collect();

        Ok(PackageContents {
            entries,
            content_root: match contents.content_root {
                ModPackageContentRoot::Fallback => PackageContentRoot::Fallback,
                ModPackageContentRoot::Single(root) => PackageContentRoot::Single(root),
                ModPackageContentRoot::Ambiguous(roots) => PackageContentRoot::Ambiguous(roots),
            },
            candidates: contents.candidates,
        })
    }

    /// 记下玩家为这个包选定的内容根（`#354` 切片 D2）。
    ///
    /// **在这里就校验白名单**，而不是等扫描时失败关闭：存下一个非候选的值，玩家会以为
    /// 选好了，直到下一次安装才报错。校验用的候选与扫描认的出自同一处
    /// （`scan_package_contents` 的 `candidates`）。
    pub fn choose_content_root(
        &self,
        request: PackageContentsQueryRequest,
        content_root: &str,
    ) -> Result<(), PackageContentsQueryError> {
        let sources = self
            .sources
            .as_ref()
            .ok_or(PackageContentsQueryError::SourcesUnavailable)?;
        let package_id = self.package_id_for(sources, &request.mod_id)?;

        let contents = self.query(request)?;
        if !contents
            .candidates
            .iter()
            .any(|candidate| candidate == content_root)
        {
            return Err(PackageContentsQueryError::ContentRootNotACandidate);
        }

        sources
            .content_root_choices
            .save_content_root(&package_id, content_root)
            .map_err(|_| PackageContentsQueryError::ContentRootChoiceUnavailable)
    }

    /// 撤销选择，回到自动解析。合集包会重新变成「等玩家决定」。
    pub fn clear_content_root(
        &self,
        request: PackageContentsQueryRequest,
    ) -> Result<(), PackageContentsQueryError> {
        let sources = self
            .sources
            .as_ref()
            .ok_or(PackageContentsQueryError::SourcesUnavailable)?;
        let package_id = self.package_id_for(sources, &request.mod_id)?;
        sources
            .content_root_choices
            .clear_content_root(&package_id)
            .map_err(|_| PackageContentsQueryError::ContentRootChoiceUnavailable)
    }

    fn package_id_for(
        &self,
        sources: &PackageContentsSources,
        mod_id: &ModId,
    ) -> Result<String, PackageContentsQueryError> {
        Ok(sources
            .result_repository
            .get_analysis(mod_id.as_str())
            .map_err(|_| PackageContentsQueryError::AnalysisUnavailable)?
            .ok_or_else(|| PackageContentsQueryError::ModNotFound {
                mod_id: mod_id.clone(),
            })?
            .package_id)
    }
}

/// 把沙箱相对路径改写成内容根相对路径；不在内容根之下则返回 `None`。
///
/// **按段比较，不是按字符串前缀。** 内容根 `皮肤` 不该吞掉 `皮肤2/x.tex`——补上分隔符再比，
/// 这个坑在路径处理里是经典的，而它的表现是「装到别的目录去」而不是报错。
fn strip_content_root(package_file_id: &str, content_root: &str) -> Option<String> {
    if content_root.is_empty() {
        return Some(package_file_id.to_owned());
    }
    package_file_id
        .strip_prefix(content_root)?
        .strip_prefix('/')
        .map(str::to_owned)
}

#[cfg(test)]
#[path = "package_contents_query_tests.rs"]
mod package_contents_query_tests;
