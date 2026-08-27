use crate::ModId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const EXTERNAL_IMPORT_SELECTION_MUTATION_MAX_ITEMS: usize = 200;
pub const EXTERNAL_IMPORT_SELECTION_MAX_ITEMS: usize = 10_000;

pub const DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_CANDIDATES: u64 =
    EXTERNAL_IMPORT_SELECTION_MAX_ITEMS as u64;
pub const DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_FILES: u64 = 1_000_000;
pub const DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024 * 1024;
pub const DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_MATERIALIZATION_BYTES: u64 = 64 * 1024 * 1024 * 1024;
// 单项物化预算。2026-08-27 依真实狩技盒子库实测放宽(见设计文档 Slice 1 定稿):
// 一个普通 Mod 就用掉了旧上限 16,384 的 44.8%(7,339 文件),而同一候选的字节只占旧
// 4 GiB 总量的 5.9%——文件数比字节紧约 7.6 倍,大型材质整合包会先撞文件数。
// 这三个值必须与 hmm-infra 的 DEFAULT_ZIP_MAX_* 保持一致:同一个 Mod 不能走迁移能进、
// 打包成 zip 反而被拒。
pub const DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_FILES: u64 = 64 * 1024;
pub const DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_SINGLE_FILE_BYTES: u64 =
    4 * 1024 * 1024 * 1024;
pub const DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub const DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_DEPTH: u32 = 64;

/// 已执行过导入的批次(终态 import_status)**永不清理**:从第三方盒子迁移是低频动作
/// (一个玩家一生可能只做几次),而「导入了哪些、成功/失败了哪些」是长期可追溯事实。
/// 它们的体量也小——每个批次只有实际候选数量的行。2026-08-27 从「保留最近 50 个」
/// 改为永久保留,因此不再有对应常量:清理逻辑直接跳过这一类。
///
/// 只扫描过、从未启动导入的批次是另一回事:它们不含任何导入事实,却可能各带最多
/// 10,000 行候选(打开弹窗扫一次就产生一个)。这类仍按数量封顶,但不再按时间过期
/// ——记录因为「过了 7 天」凭空消失比没有记录更伤信任。
pub const EXTERNAL_IMPORT_HISTORY_MAX_SCAN_ONLY_BATCHES: usize = 50;

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

string_id!(ExternalImportAdapterId);
string_id!(ExternalImportSourceId);
string_id!(ExternalImportBatchId);
string_id!(ExternalImportCandidateId);
string_id!(ExternalImportSelectionId);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportSource {
    pub source_id: ExternalImportSourceId,
    pub adapter_id: ExternalImportAdapterId,
    pub display_label: String,
    pub expires_at_unix_millis: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportBatch {
    pub batch_id: ExternalImportBatchId,
    /// Ephemeral opaque source handle. It never contains a path and may become unavailable
    /// after the source registry expires it.
    #[serde(default)]
    pub source_id: Option<ExternalImportSourceId>,
    pub adapter_id: ExternalImportAdapterId,
    /// A keyed local digest. It is intentionally not a source path or ordinary path hash.
    pub source_fingerprint: String,
    pub scan_status: ExternalImportScanStatus,
    pub import_status: ExternalImportBatchImportStatus,
    pub created_at_unix_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportScanStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportBatchImportStatus {
    Pending,
    Running,
    Completed,
    CompletedWithErrors,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportMetadataHint {
    pub display_name: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub source_mod_type: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportResourceUsage {
    pub file_count: u64,
    pub source_bytes: u64,
    pub materialization_bytes: u64,
}

impl ExternalImportResourceUsage {
    fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            file_count: self.file_count.checked_add(other.file_count)?,
            source_bytes: self.source_bytes.checked_add(other.source_bytes)?,
            materialization_bytes: self
                .materialization_bytes
                .checked_add(other.materialization_bytes)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportMaterializationBudget {
    pub max_files: u64,
    pub max_single_file_bytes: u64,
    pub max_total_bytes: u64,
    pub max_directory_depth: u32,
}

impl Default for ExternalImportMaterializationBudget {
    fn default() -> Self {
        Self {
            max_files: DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_FILES,
            max_single_file_bytes: DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_SINGLE_FILE_BYTES,
            max_total_bytes: DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_TOTAL_BYTES,
            max_directory_depth: DEFAULT_EXTERNAL_IMPORT_MATERIALIZATION_MAX_DEPTH,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportResourceBudget {
    /// Bounds the number of root-level source entries the scanner may retain for one preview.
    /// This prevents empty or malformed directories from bypassing file and byte budgets.
    #[serde(default = "default_external_import_batch_max_candidates")]
    pub max_total_candidates: u64,
    pub max_total_files: u64,
    pub max_total_source_bytes: u64,
    pub max_total_materialization_bytes: u64,
    pub materialization: ExternalImportMaterializationBudget,
}

impl Default for ExternalImportResourceBudget {
    fn default() -> Self {
        Self {
            max_total_candidates: DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_CANDIDATES,
            max_total_files: DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_FILES,
            max_total_source_bytes: DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_SOURCE_BYTES,
            max_total_materialization_bytes:
                DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_MATERIALIZATION_BYTES,
            materialization: ExternalImportMaterializationBudget::default(),
        }
    }
}

fn default_external_import_batch_max_candidates() -> u64 {
    DEFAULT_EXTERNAL_IMPORT_BATCH_MAX_CANDIDATES
}

impl ExternalImportResourceBudget {
    pub fn permits(&self, usage: ExternalImportResourceUsage) -> bool {
        usage.file_count <= self.max_total_files
            && usage.source_bytes <= self.max_total_source_bytes
            && usage.materialization_bytes <= self.max_total_materialization_bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportCandidateStatus {
    Ready,
    AlreadyImported,
    DuplicateInBatch,
    NameCollision,
    StructureInvalid,
    MetadataInvalid,
    UnsupportedEntry,
    ResourceLimitExceeded,
    SourceUnreadable,
    /// 编号目录有元数据但没有 `files/` 载荷(狩技盒子「无操作」安装方式的残留)。
    /// 与 StructureInvalid 分开:预览要能带着 mod 名明确告诉玩家「载荷不在盒子库中」。
    PayloadMissing,
}

impl ExternalImportCandidateStatus {
    pub fn reason_code(self) -> Option<ExternalImportReasonCode> {
        match self {
            Self::Ready => None,
            Self::AlreadyImported => Some(ExternalImportReasonCode::AlreadyImported),
            Self::DuplicateInBatch => Some(ExternalImportReasonCode::DuplicateInBatch),
            Self::NameCollision => Some(ExternalImportReasonCode::NameCollision),
            Self::StructureInvalid => Some(ExternalImportReasonCode::StructureInvalid),
            Self::MetadataInvalid => Some(ExternalImportReasonCode::MetadataInvalid),
            Self::UnsupportedEntry => Some(ExternalImportReasonCode::UnsupportedEntry),
            Self::ResourceLimitExceeded => Some(ExternalImportReasonCode::ResourceLimitExceeded),
            Self::SourceUnreadable => Some(ExternalImportReasonCode::SourceUnreadable),
            Self::PayloadMissing => Some(ExternalImportReasonCode::PayloadMissing),
        }
    }

    fn may_be_selected(self) -> bool {
        matches!(
            self,
            Self::Ready | Self::NameCollision | Self::MetadataInvalid
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportConflictKind {
    None,
    ContentDuplicate,
    NameCollision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportCandidate {
    pub batch_id: ExternalImportBatchId,
    pub candidate_id: ExternalImportCandidateId,
    /// An opaque hash of the adapter-local item key. It never stores a source path.
    pub source_item_key_hash: String,
    pub content_fingerprint: String,
    pub metadata_hint: ExternalImportMetadataHint,
    pub resource_usage: ExternalImportResourceUsage,
    pub preview_status: ExternalImportCandidateStatus,
    pub conflict_kind: ExternalImportConflictKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportConflictResolution {
    KeepBoth,
    IgnoreInvalidMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportSelectionDecision {
    pub conflict_resolution: Option<ExternalImportConflictResolution>,
    /// Existing category identity only. Category existence is checked by the app use case later.
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportSelectionEntry {
    pub candidate_id: ExternalImportCandidateId,
    pub decision: Option<ExternalImportSelectionDecision>,
    pub updated_at_unix_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportSelectionStatus {
    Editing,
    Sealed,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportSelection {
    pub selection_id: ExternalImportSelectionId,
    pub batch_id: ExternalImportBatchId,
    pub revision: u64,
    pub status: ExternalImportSelectionStatus,
    pub entries: Vec<ExternalImportSelectionEntry>,
    pub selected_resource_usage: ExternalImportResourceUsage,
    pub expires_at_unix_millis: u64,
}

impl ExternalImportSelection {
    pub fn new(
        selection_id: ExternalImportSelectionId,
        batch_id: ExternalImportBatchId,
        expires_at_unix_millis: u64,
    ) -> Self {
        Self {
            selection_id,
            batch_id,
            revision: 0,
            status: ExternalImportSelectionStatus::Editing,
            entries: Vec::new(),
            selected_resource_usage: ExternalImportResourceUsage::default(),
            expires_at_unix_millis,
        }
    }

    pub fn selected_count(&self) -> usize {
        self.entries.len()
    }

    pub fn apply_mutation(
        &mut self,
        expected_revision: u64,
        mutations: &[ExternalImportSelectionMutation],
        candidates: &[ExternalImportCandidate],
        budget: &ExternalImportResourceBudget,
        now_unix_millis: u64,
    ) -> Result<ExternalImportSelectionMutationResult, ExternalImportSelectionError> {
        self.ensure_editable(expected_revision, now_unix_millis)?;
        if mutations.is_empty() {
            return Err(ExternalImportSelectionError::MutationEmpty);
        }
        if mutations.len() > EXTERNAL_IMPORT_SELECTION_MUTATION_MAX_ITEMS {
            return Err(ExternalImportSelectionError::MutationLimitExceeded);
        }

        let candidates_by_id = candidates_by_id(candidates)?;
        let mut next_entries = entries_by_candidate_id(&self.entries)?;
        let mut seen_mutations = BTreeSet::new();

        for mutation in mutations {
            if !seen_mutations.insert(&mutation.candidate_id) {
                return Err(ExternalImportSelectionError::CandidateInvalid);
            }
            let candidate = candidates_by_id
                .get(&mutation.candidate_id)
                .ok_or(ExternalImportSelectionError::CandidateInvalid)?;
            if candidate.batch_id != self.batch_id {
                return Err(ExternalImportSelectionError::CandidateInvalid);
            }

            if mutation.selected {
                if !candidate.preview_status.may_be_selected()
                    || !selection_decision_is_valid(candidate, mutation.decision.as_ref())
                {
                    return Err(ExternalImportSelectionError::CandidateInvalid);
                }
                next_entries.insert(
                    mutation.candidate_id.clone(),
                    ExternalImportSelectionEntry {
                        candidate_id: mutation.candidate_id.clone(),
                        decision: mutation.decision.clone(),
                        updated_at_unix_millis: now_unix_millis,
                    },
                );
            } else {
                if mutation.decision.is_some() {
                    return Err(ExternalImportSelectionError::CandidateInvalid);
                }
                next_entries.remove(&mutation.candidate_id);
            }
        }

        let (entries, usage) =
            validated_entries_and_usage(next_entries, &self.batch_id, &candidates_by_id, budget)?;
        self.entries = entries;
        self.selected_resource_usage = usage;
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(ExternalImportSelectionError::RevisionOverflow)?;

        Ok(ExternalImportSelectionMutationResult {
            revision: self.revision,
            selected_count: self.selected_count(),
            selected_resource_usage: self.selected_resource_usage,
        })
    }

    pub fn seal(
        &mut self,
        expected_revision: u64,
        candidates: &[ExternalImportCandidate],
        budget: &ExternalImportResourceBudget,
        now_unix_millis: u64,
    ) -> Result<ExternalImportSelectionMutationResult, ExternalImportSelectionError> {
        self.ensure_editable(expected_revision, now_unix_millis)?;
        if self.entries.is_empty() {
            return Err(ExternalImportSelectionError::Empty);
        }

        let candidates_by_id = candidates_by_id(candidates)?;
        let entries_by_id = entries_by_candidate_id(&self.entries)?;
        let (entries, usage) =
            validated_entries_and_usage(entries_by_id, &self.batch_id, &candidates_by_id, budget)?;
        self.entries = entries;
        self.selected_resource_usage = usage;
        self.status = ExternalImportSelectionStatus::Sealed;
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(ExternalImportSelectionError::RevisionOverflow)?;

        Ok(ExternalImportSelectionMutationResult {
            revision: self.revision,
            selected_count: self.selected_count(),
            selected_resource_usage: self.selected_resource_usage,
        })
    }

    /// Server-side select-all only adds candidates that are immediately selectable without an
    /// extra conflict decision. Existing explicit decisions remain part of the snapshot.
    pub fn select_all_ready(
        &mut self,
        expected_revision: u64,
        candidates: &[ExternalImportCandidate],
        budget: &ExternalImportResourceBudget,
        now_unix_millis: u64,
    ) -> Result<ExternalImportSelectionMutationResult, ExternalImportSelectionError> {
        self.ensure_editable(expected_revision, now_unix_millis)?;

        let candidates_by_id = candidates_by_id(candidates)?;
        let mut next_entries = entries_by_candidate_id(&self.entries)?;
        for candidate in candidates_by_id.values() {
            if candidate.batch_id != self.batch_id {
                return Err(ExternalImportSelectionError::CandidateInvalid);
            }
            if candidate.preview_status == ExternalImportCandidateStatus::Ready {
                next_entries
                    .entry(candidate.candidate_id.clone())
                    .or_insert_with(|| ExternalImportSelectionEntry {
                        candidate_id: candidate.candidate_id.clone(),
                        decision: None,
                        updated_at_unix_millis: now_unix_millis,
                    });
            }
        }

        let (entries, usage) =
            validated_entries_and_usage(next_entries, &self.batch_id, &candidates_by_id, budget)?;
        self.entries = entries;
        self.selected_resource_usage = usage;
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(ExternalImportSelectionError::RevisionOverflow)?;

        Ok(ExternalImportSelectionMutationResult {
            revision: self.revision,
            selected_count: self.selected_count(),
            selected_resource_usage: self.selected_resource_usage,
        })
    }

    fn ensure_editable(
        &self,
        expected_revision: u64,
        now_unix_millis: u64,
    ) -> Result<(), ExternalImportSelectionError> {
        if self.revision != expected_revision {
            return Err(ExternalImportSelectionError::RevisionConflict);
        }
        match self.status {
            ExternalImportSelectionStatus::Sealed => Err(ExternalImportSelectionError::Closed),
            ExternalImportSelectionStatus::Expired => Err(ExternalImportSelectionError::Expired),
            ExternalImportSelectionStatus::Editing
                if now_unix_millis >= self.expires_at_unix_millis =>
            {
                Err(ExternalImportSelectionError::Expired)
            }
            ExternalImportSelectionStatus::Editing => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportSelectionMutation {
    pub candidate_id: ExternalImportCandidateId,
    pub selected: bool,
    pub decision: Option<ExternalImportSelectionDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportSelectionMutationResult {
    pub revision: u64,
    pub selected_count: usize,
    pub selected_resource_usage: ExternalImportResourceUsage,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ExternalImportSelectionError {
    #[error("selection revision conflict")]
    RevisionConflict,
    #[error("selection is empty")]
    Empty,
    #[error("selection mutation is empty")]
    MutationEmpty,
    #[error("selection mutation limit exceeded")]
    MutationLimitExceeded,
    #[error("selection total limit exceeded")]
    TotalLimitExceeded,
    #[error("selection resource limit exceeded")]
    ResourceLimitExceeded,
    #[error("selection candidate is invalid")]
    CandidateInvalid,
    #[error("selection has expired")]
    Expired,
    #[error("selection is closed")]
    Closed,
    #[error("selection revision overflow")]
    RevisionOverflow,
}

impl ExternalImportSelectionError {
    pub fn reason_code(self) -> ExternalImportReasonCode {
        match self {
            Self::RevisionConflict => ExternalImportReasonCode::SelectionRevisionConflict,
            Self::Empty => ExternalImportReasonCode::SelectionEmpty,
            Self::MutationEmpty => ExternalImportReasonCode::SelectionMutationEmpty,
            Self::MutationLimitExceeded => ExternalImportReasonCode::SelectionMutationLimitExceeded,
            Self::TotalLimitExceeded => ExternalImportReasonCode::SelectionTotalLimitExceeded,
            Self::ResourceLimitExceeded => ExternalImportReasonCode::SelectionResourceLimitExceeded,
            Self::CandidateInvalid => ExternalImportReasonCode::SelectionCandidateInvalid,
            Self::Expired => ExternalImportReasonCode::SelectionExpired,
            Self::Closed => ExternalImportReasonCode::SelectionClosed,
            Self::RevisionOverflow => ExternalImportReasonCode::SelectionRevisionOverflow,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportItemStatus {
    Imported,
    AlreadyImported,
    Skipped,
    Blocked,
    Failed,
    Cancelled,
}

impl ExternalImportItemStatus {
    /// 与 serde 的 snake_case 序列化保持一字不差:SQL 派生列与 DTO 共用这套稳定字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Imported => "imported",
            Self::AlreadyImported => "already_imported",
            Self::Skipped => "skipped",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "imported" => Some(Self::Imported),
            "already_imported" => Some(Self::AlreadyImported),
            "skipped" => Some(Self::Skipped),
            "blocked" => Some(Self::Blocked),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportItemStatusCounts {
    pub imported: u64,
    pub already_imported: u64,
    pub skipped: u64,
    pub blocked: u64,
    pub failed: u64,
    pub cancelled: u64,
}

impl ExternalImportItemStatusCounts {
    pub fn add(&mut self, status: ExternalImportItemStatus, count: u64) -> Option<()> {
        let bucket = match status {
            ExternalImportItemStatus::Imported => &mut self.imported,
            ExternalImportItemStatus::AlreadyImported => &mut self.already_imported,
            ExternalImportItemStatus::Skipped => &mut self.skipped,
            ExternalImportItemStatus::Blocked => &mut self.blocked,
            ExternalImportItemStatus::Failed => &mut self.failed,
            ExternalImportItemStatus::Cancelled => &mut self.cancelled,
        };
        *bucket = bucket.checked_add(count)?;
        Some(())
    }

    /// 单批结果行数受 EXTERNAL_IMPORT_SELECTION_MAX_ITEMS 约束,饱和在实践中不可达;
    /// 写入侧 add 已做 checked 加法,前端守卫另行核对 total 与分项之和。
    pub fn total(self) -> u64 {
        self.imported
            .saturating_add(self.already_imported)
            .saturating_add(self.skipped)
            .saturating_add(self.blocked)
            .saturating_add(self.failed)
            .saturating_add(self.cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportItemResult {
    pub candidate_id: ExternalImportCandidateId,
    pub status: ExternalImportItemStatus,
    pub reason_code: Option<ExternalImportReasonCode>,
    pub imported_mod_id: Option<ModId>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalImportReasonCode {
    AlreadyImported,
    DuplicateInBatch,
    NameCollision,
    StructureInvalid,
    MetadataInvalid,
    UnsupportedEntry,
    ResourceLimitExceeded,
    SourceUnreadable,
    PayloadMissing,
    SourceChanged,
    SelectionRevisionConflict,
    SelectionEmpty,
    SelectionMutationEmpty,
    SelectionMutationLimitExceeded,
    SelectionTotalLimitExceeded,
    SelectionResourceLimitExceeded,
    SelectionCandidateInvalid,
    SelectionExpired,
    SelectionClosed,
    SelectionRevisionOverflow,
}

impl ExternalImportReasonCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyImported => "already_imported",
            Self::DuplicateInBatch => "duplicate_in_batch",
            Self::NameCollision => "name_collision",
            Self::StructureInvalid => "structure_invalid",
            Self::MetadataInvalid => "metadata_invalid",
            Self::UnsupportedEntry => "unsupported_entry",
            Self::ResourceLimitExceeded => "resource_limit_exceeded",
            Self::SourceUnreadable => "source_unreadable",
            Self::PayloadMissing => "payload_missing",
            Self::SourceChanged => "source_changed",
            Self::SelectionRevisionConflict => "selection_revision_conflict",
            Self::SelectionEmpty => "selection_empty",
            Self::SelectionMutationEmpty => "selection_mutation_empty",
            Self::SelectionMutationLimitExceeded => "selection_mutation_limit_exceeded",
            Self::SelectionTotalLimitExceeded => "selection_total_limit_exceeded",
            Self::SelectionResourceLimitExceeded => "selection_resource_limit_exceeded",
            Self::SelectionCandidateInvalid => "selection_candidate_invalid",
            Self::SelectionExpired => "selection_expired",
            Self::SelectionClosed => "selection_closed",
            Self::SelectionRevisionOverflow => "selection_revision_overflow",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalImportProvenance {
    pub adapter_id: ExternalImportAdapterId,
    pub batch_id: ExternalImportBatchId,
    pub source_item_key_hash: String,
    pub content_fingerprint: String,
    pub imported_at_unix_millis: u64,
}

impl ExternalImportProvenance {
    pub fn validate(&self) -> Result<(), ExternalImportProvenanceError> {
        for value in [
            self.adapter_id.as_str(),
            self.batch_id.as_str(),
            self.source_item_key_hash.as_str(),
        ] {
            if !is_valid_opaque_value(value) {
                return Err(ExternalImportProvenanceError::InvalidOpaqueValue);
            }
        }
        if !is_valid_opaque_value(&self.content_fingerprint)
            && !is_sha256_content_fingerprint(&self.content_fingerprint)
        {
            return Err(ExternalImportProvenanceError::InvalidOpaqueValue);
        }
        Ok(())
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ExternalImportProvenanceError {
    #[error("external import provenance contains an invalid opaque value")]
    InvalidOpaqueValue,
}

fn is_valid_opaque_value(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value.chars().all(|character| {
            !character.is_control()
                && !character.is_whitespace()
                && !matches!(character, '/' | '\\' | ':')
        })
}

fn is_sha256_content_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn candidates_by_id(
    candidates: &[ExternalImportCandidate],
) -> Result<
    BTreeMap<ExternalImportCandidateId, &ExternalImportCandidate>,
    ExternalImportSelectionError,
> {
    let mut candidates_by_id = BTreeMap::new();
    for candidate in candidates {
        if candidates_by_id
            .insert(candidate.candidate_id.clone(), candidate)
            .is_some()
        {
            return Err(ExternalImportSelectionError::CandidateInvalid);
        }
    }
    Ok(candidates_by_id)
}

fn entries_by_candidate_id(
    entries: &[ExternalImportSelectionEntry],
) -> Result<
    BTreeMap<ExternalImportCandidateId, ExternalImportSelectionEntry>,
    ExternalImportSelectionError,
> {
    let mut entries_by_id = BTreeMap::new();
    for entry in entries {
        if entries_by_id
            .insert(entry.candidate_id.clone(), entry.clone())
            .is_some()
        {
            return Err(ExternalImportSelectionError::CandidateInvalid);
        }
    }
    Ok(entries_by_id)
}

fn validated_entries_and_usage(
    entries_by_id: BTreeMap<ExternalImportCandidateId, ExternalImportSelectionEntry>,
    batch_id: &ExternalImportBatchId,
    candidates_by_id: &BTreeMap<ExternalImportCandidateId, &ExternalImportCandidate>,
    budget: &ExternalImportResourceBudget,
) -> Result<
    (
        Vec<ExternalImportSelectionEntry>,
        ExternalImportResourceUsage,
    ),
    ExternalImportSelectionError,
> {
    if entries_by_id.len() > EXTERNAL_IMPORT_SELECTION_MAX_ITEMS {
        return Err(ExternalImportSelectionError::TotalLimitExceeded);
    }

    let mut usage = ExternalImportResourceUsage::default();
    for entry in entries_by_id.values() {
        let candidate = candidates_by_id
            .get(&entry.candidate_id)
            .ok_or(ExternalImportSelectionError::CandidateInvalid)?;
        if candidate.batch_id != *batch_id
            || !candidate.preview_status.may_be_selected()
            || !selection_decision_is_valid(candidate, entry.decision.as_ref())
        {
            return Err(ExternalImportSelectionError::CandidateInvalid);
        }
        usage = usage
            .checked_add(candidate.resource_usage)
            .ok_or(ExternalImportSelectionError::ResourceLimitExceeded)?;
    }
    if !budget.permits(usage) {
        return Err(ExternalImportSelectionError::ResourceLimitExceeded);
    }

    Ok((entries_by_id.into_values().collect(), usage))
}

fn selection_decision_is_valid(
    candidate: &ExternalImportCandidate,
    decision: Option<&ExternalImportSelectionDecision>,
) -> bool {
    let category_id_is_valid = decision
        .and_then(|decision| decision.category_id.as_deref())
        .is_none_or(|category_id| !category_id.trim().is_empty());
    if !category_id_is_valid {
        return false;
    }

    let resolution = decision.and_then(|decision| decision.conflict_resolution);
    match candidate.preview_status {
        ExternalImportCandidateStatus::Ready => {
            resolution.is_none() || resolution == Some(ExternalImportConflictResolution::KeepBoth)
        }
        ExternalImportCandidateStatus::NameCollision => {
            resolution == Some(ExternalImportConflictResolution::KeepBoth)
        }
        ExternalImportCandidateStatus::MetadataInvalid => {
            resolution == Some(ExternalImportConflictResolution::IgnoreInvalidMetadata)
        }
        ExternalImportCandidateStatus::AlreadyImported
        | ExternalImportCandidateStatus::DuplicateInBatch
        | ExternalImportCandidateStatus::StructureInvalid
        | ExternalImportCandidateStatus::UnsupportedEntry
        | ExternalImportCandidateStatus::ResourceLimitExceeded
        | ExternalImportCandidateStatus::SourceUnreadable
        | ExternalImportCandidateStatus::PayloadMissing => false,
    }
}
