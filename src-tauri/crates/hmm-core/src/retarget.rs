use crate::{
    ContentTransformInvocation, GameId, InstallTargetPath, PackageFileId, ReplacementAdapterFacts,
    ReplacementBinding, ReplacementBindingId, ReplacementSourceId, ReplacementTargetKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetargetError {
    #[error("replacement source internal id cannot be empty")]
    EmptySourceInternalId,
    #[error("replacement source path family cannot be empty")]
    EmptySourcePathFamily,
    #[error(
        "replacement source {source_id} belongs to game {actual_game_id}, expected {expected_game_id}"
    )]
    SourceGameMismatch {
        source_id: String,
        expected_game_id: String,
        actual_game_id: String,
    },
    #[error("replacement analysis contains a duplicate source id: {source_id}")]
    DuplicateSourceId { source_id: String },
    #[error("replacement analysis matched asset count is smaller than its source count")]
    MatchedAssetCountTooSmall,
    #[error("retarget action package file id cannot be empty")]
    EmptyPackageFileId,
    #[error("retarget action source internal id cannot be empty")]
    EmptyActionSourceInternalId,
    #[error("retarget action target internal id cannot be empty")]
    EmptyActionTargetInternalId,
    #[error("retarget action source path family cannot be empty")]
    EmptyActionSourcePathFamily,
    #[error("retarget action target path family cannot be empty")]
    EmptyActionTargetPathFamily,
    #[error("retarget plan cannot be empty")]
    EmptyRetargetPlan,
    #[error("retarget plan source is not supported")]
    UnsupportedPlanSource,
    #[error("retarget plan binding does not reference its source")]
    BindingSourceMismatch,
    #[error("retarget action does not reference the plan source")]
    ActionSourceMismatch,
    #[error("retarget plan actions do not share one target")]
    InconsistentRetargetTarget,
    #[error("retarget plan contains a duplicate package file id: {package_file_id}")]
    DuplicateRetargetPackageFile { package_file_id: String },
    #[error("retarget plan contains a duplicate target path: {target_path}")]
    DuplicateRetargetTargetPath { target_path: String },
    #[error("retarget plan transform facts are missing")]
    MissingTransformFacts,
    #[error("retarget plan transform facts do not match its actions")]
    TransformFactsMismatch,
    #[error("retarget source routing maps package file {package_file_id} to two bindings")]
    AmbiguousSourceRouting { package_file_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplacementSource {
    id: ReplacementSourceId,
    game_id: GameId,
    source_type: ReplacementTargetKind,
    internal_id: String,
    path_family: String,
    supported: bool,
}

impl ReplacementSource {
    pub fn new(
        id: ReplacementSourceId,
        game_id: GameId,
        source_type: ReplacementTargetKind,
        internal_id: impl Into<String>,
        path_family: impl Into<String>,
        supported: bool,
    ) -> Result<Self, RetargetError> {
        let internal_id = internal_id.into();
        let internal_id = internal_id.trim();
        if internal_id.is_empty() {
            return Err(RetargetError::EmptySourceInternalId);
        }

        let path_family = path_family.into();
        let path_family = path_family.trim();
        if path_family.is_empty() {
            return Err(RetargetError::EmptySourcePathFamily);
        }

        Ok(Self {
            id,
            game_id,
            source_type,
            internal_id: internal_id.to_owned(),
            path_family: path_family.to_owned(),
            supported,
        })
    }

    pub fn id(&self) -> &ReplacementSourceId {
        &self.id
    }

    pub fn game_id(&self) -> &GameId {
        &self.game_id
    }

    pub fn source_type(&self) -> &ReplacementTargetKind {
        &self.source_type
    }

    pub fn internal_id(&self) -> &str {
        &self.internal_id
    }

    pub fn path_family(&self) -> &str {
        &self.path_family
    }

    pub fn is_supported(&self) -> bool {
        self.supported
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplacementWarning {
    NoSupportedAssets,
    MultipleSources,
    UnsupportedSource,
    SourceMatchesTarget,
    WeaponPartialPartSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplacementAnalysis {
    game_id: GameId,
    sources: Vec<ReplacementSource>,
    matched_asset_count: usize,
    warnings: Vec<ReplacementWarning>,
}

impl ReplacementAnalysis {
    pub fn new(
        game_id: GameId,
        sources: Vec<ReplacementSource>,
        matched_asset_count: usize,
        warnings: Vec<ReplacementWarning>,
    ) -> Result<Self, RetargetError> {
        let mut source_ids = BTreeSet::new();
        for source in &sources {
            if source.game_id() != &game_id {
                return Err(RetargetError::SourceGameMismatch {
                    source_id: source.id().as_str().to_owned(),
                    expected_game_id: game_id.as_str().to_owned(),
                    actual_game_id: source.game_id().as_str().to_owned(),
                });
            }
            if !source_ids.insert(source.id().clone()) {
                return Err(RetargetError::DuplicateSourceId {
                    source_id: source.id().as_str().to_owned(),
                });
            }
        }
        if matched_asset_count < sources.len() {
            return Err(RetargetError::MatchedAssetCountTooSmall);
        }

        Ok(Self {
            game_id,
            sources,
            matched_asset_count,
            warnings,
        })
    }

    pub fn game_id(&self) -> &GameId {
        &self.game_id
    }

    pub fn sources(&self) -> &[ReplacementSource] {
        &self.sources
    }

    pub fn matched_asset_count(&self) -> usize {
        self.matched_asset_count
    }

    pub fn warnings(&self) -> &[ReplacementWarning] {
        &self.warnings
    }

    pub fn is_retargetable(&self) -> bool {
        self.sources.len() == 1 && self.sources[0].is_supported()
    }

    pub fn single_source(&self) -> Option<&ReplacementSource> {
        self.is_retargetable().then(|| &self.sources[0])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetargetAction {
    package_file_id: PackageFileId,
    source_relative_path: InstallTargetPath,
    target_relative_path: InstallTargetPath,
    source_id: ReplacementSourceId,
    source_internal_id: String,
    target_internal_id: String,
    source_path_family: String,
    target_path_family: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_transform: Option<ContentTransformInvocation>,
}

impl RetargetAction {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        package_file_id: PackageFileId,
        source_relative_path: InstallTargetPath,
        target_relative_path: InstallTargetPath,
        source_id: ReplacementSourceId,
        source_internal_id: impl Into<String>,
        target_internal_id: impl Into<String>,
        source_path_family: impl Into<String>,
        target_path_family: impl Into<String>,
    ) -> Result<Self, RetargetError> {
        if package_file_id.as_str().trim().is_empty() {
            return Err(RetargetError::EmptyPackageFileId);
        }
        let source_internal_id = required_action_field(
            source_internal_id.into(),
            RetargetError::EmptyActionSourceInternalId,
        )?;
        let target_internal_id = required_action_field(
            target_internal_id.into(),
            RetargetError::EmptyActionTargetInternalId,
        )?;
        let source_path_family = required_action_field(
            source_path_family.into(),
            RetargetError::EmptyActionSourcePathFamily,
        )?;
        let target_path_family = required_action_field(
            target_path_family.into(),
            RetargetError::EmptyActionTargetPathFamily,
        )?;

        Ok(Self {
            package_file_id,
            source_relative_path,
            target_relative_path,
            source_id,
            source_internal_id,
            target_internal_id,
            source_path_family,
            target_path_family,
            content_transform: None,
        })
    }

    pub fn with_content_transform(mut self, invocation: ContentTransformInvocation) -> Self {
        self.content_transform = Some(invocation);
        self
    }

    pub fn package_file_id(&self) -> &PackageFileId {
        &self.package_file_id
    }

    pub fn source_relative_path(&self) -> &InstallTargetPath {
        &self.source_relative_path
    }

    pub fn target_relative_path(&self) -> &InstallTargetPath {
        &self.target_relative_path
    }

    pub fn source_id(&self) -> &ReplacementSourceId {
        &self.source_id
    }

    pub fn source_internal_id(&self) -> &str {
        &self.source_internal_id
    }

    pub fn target_internal_id(&self) -> &str {
        &self.target_internal_id
    }

    pub fn source_path_family(&self) -> &str {
        &self.source_path_family
    }

    pub fn target_path_family(&self) -> &str {
        &self.target_path_family
    }

    pub fn content_transform(&self) -> Option<&ContentTransformInvocation> {
        self.content_transform.as_ref()
    }
}

fn required_action_field(value: String, error: RetargetError) -> Result<String, RetargetError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(error);
    }
    Ok(value.to_owned())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetargetPlan {
    binding: ReplacementBinding,
    source: ReplacementSource,
    actions: Vec<RetargetAction>,
    warnings: Vec<ReplacementWarning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    adapter_facts: Option<ReplacementAdapterFacts>,
}

impl RetargetPlan {
    pub fn new(
        binding: ReplacementBinding,
        source: ReplacementSource,
        actions: Vec<RetargetAction>,
        warnings: Vec<ReplacementWarning>,
    ) -> Result<Self, RetargetError> {
        if actions.is_empty() {
            return Err(RetargetError::EmptyRetargetPlan);
        }
        if !source.is_supported() {
            return Err(RetargetError::UnsupportedPlanSource);
        }
        if binding.source_id() != source.id() {
            return Err(RetargetError::BindingSourceMismatch);
        }

        let expected_target_internal_id = actions[0].target_internal_id();
        let expected_target_path_family = actions[0].target_path_family();
        let mut package_file_ids = BTreeSet::new();
        let mut target_paths = BTreeSet::new();
        for action in &actions {
            if action.source_id() != source.id()
                || action.source_internal_id() != source.internal_id()
                || action.source_path_family() != source.path_family()
            {
                return Err(RetargetError::ActionSourceMismatch);
            }
            if action.target_internal_id() != expected_target_internal_id
                || action.target_path_family() != expected_target_path_family
            {
                return Err(RetargetError::InconsistentRetargetTarget);
            }
            if !package_file_ids.insert(action.package_file_id().clone()) {
                return Err(RetargetError::DuplicateRetargetPackageFile {
                    package_file_id: action.package_file_id().as_str().to_owned(),
                });
            }
            if !target_paths.insert(action.target_relative_path().clone()) {
                return Err(RetargetError::DuplicateRetargetTargetPath {
                    target_path: action.target_relative_path().as_str().to_owned(),
                });
            }
        }

        Ok(Self {
            binding,
            source,
            actions,
            warnings,
            adapter_facts: None,
        })
    }

    pub fn with_adapter_facts(
        mut self,
        adapter_facts: ReplacementAdapterFacts,
    ) -> Result<Self, RetargetError> {
        self.adapter_facts = Some(adapter_facts);
        self.validate_transform_facts()?;
        Ok(self)
    }

    pub fn validate_transform_facts(&self) -> Result<(), RetargetError> {
        let has_transforms = self
            .actions
            .iter()
            .any(|action| action.content_transform().is_some());
        let Some(adapter_facts) = &self.adapter_facts else {
            return if has_transforms {
                Err(RetargetError::MissingTransformFacts)
            } else {
                Ok(())
            };
        };
        if adapter_facts.transform_set_sha256() != self.content_transform_set_sha256() {
            return Err(RetargetError::TransformFactsMismatch);
        }
        if has_transforms
            && (adapter_facts.transformer_identities()
                != self.content_transformer_identities().as_slice()
                || adapter_facts.file_count() != self.actions.len() as u32
                || adapter_facts.part_count() == 0)
        {
            return Err(RetargetError::TransformFactsMismatch);
        }
        Ok(())
    }

    pub fn content_transformer_identities(&self) -> Vec<crate::ContentTransformerIdentity> {
        self.actions
            .iter()
            .filter_map(|action| action.content_transform())
            .map(|invocation| invocation.transformer_identity())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn content_transform_set_sha256(&self) -> String {
        let mut transformed_actions = self
            .actions
            .iter()
            .filter_map(|action| {
                action
                    .content_transform()
                    .map(|invocation| (action, invocation))
            })
            .collect::<Vec<_>>();
        transformed_actions.sort_by(|(left, _), (right, _)| {
            left.package_file_id()
                .cmp(right.package_file_id())
                .then_with(|| {
                    left.target_relative_path()
                        .cmp(right.target_relative_path())
                })
        });

        let mut hasher = Sha256::new();
        hash_transform_field(&mut hasher, "hmm-content-transform-set-v1");
        hasher.update((transformed_actions.len() as u64).to_be_bytes());
        for (action, invocation) in transformed_actions {
            hash_transform_field(&mut hasher, action.package_file_id().as_str());
            hash_transform_field(&mut hasher, action.target_relative_path().as_str());
            hasher.update(invocation.schema_version().to_be_bytes());
            hash_transform_field(&mut hasher, invocation.transformer_id());
            hasher.update(invocation.transformer_version().to_be_bytes());
            hash_transform_field(&mut hasher, invocation.source_content_sha256());
            hash_transform_field(&mut hasher, invocation.output_content_sha256());
            hash_transform_field(&mut hasher, invocation.canonical_mapping_sha256());
            hasher.update((invocation.dependencies().len() as u64).to_be_bytes());
            for (package_file_id, digest) in invocation.dependencies() {
                hash_transform_field(&mut hasher, package_file_id.as_str());
                hash_transform_field(&mut hasher, digest);
            }
            hasher.update((invocation.parameters().len() as u64).to_be_bytes());
            for (key, value) in invocation.parameters() {
                hash_transform_field(&mut hasher, key);
                hash_transform_field(&mut hasher, value);
            }
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn binding(&self) -> &ReplacementBinding {
        &self.binding
    }

    pub fn source(&self) -> &ReplacementSource {
        &self.source
    }

    pub fn actions(&self) -> &[RetargetAction] {
        &self.actions
    }

    pub fn warnings(&self) -> &[ReplacementWarning] {
        &self.warnings
    }

    pub fn adapter_facts(&self) -> Option<&ReplacementAdapterFacts> {
        self.adapter_facts.as_ref()
    }

    /// 这个计划的产出落在哪个绑定的 staging 根下——`RetargetSourceRouting` 的单绑定片段。
    /// 多绑定提交把每个计划的片段合并起来（`RetargetSourceRouting::merge`）。
    pub fn source_routing(&self) -> RetargetSourceRouting {
        let mut routing = RetargetSourceRouting::empty();
        for action in &self.actions {
            // `RetargetPlan::new` 已拒绝重复 `package_file_id`，这里不会冲突。
            let _ = routing.stage(action.package_file_id().clone(), self.binding.id().clone());
        }
        routing
    }
}

/// 一个 `package_file_id` 的字节从哪儿来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetargetSourceOrigin {
    /// 某个绑定的 staging 根：重定向产出，字节已经改写过。
    Staged(ReplacementBindingId),
    /// 沙箱原包：「保持原位」的槽位与包级随行文件，字节原样安装。
    ImportedPackage,
}

/// 「某个 `package_file_id` 的字节该从哪儿读」。
///
/// `#349` 切片③b：一个安装计划可以同时包含多个绑定的重定向产出（各自一个 staging 根）
/// 与「保持原位」的原包文件（直接读沙箱）。这份归属信息**故意不在 `InstallPlan` 里**——
/// `InstallAction` 没有绑定字段，而 `hmm-install-plan-v1` 段对 action **逐条哈希**，
/// 追加字段会静默改掉所有既有 `plan_hash`（`#286` 踩过这个坑）。归属只在**组装计划的
/// 那一刻**可知，所以由组装方给出这份路由、随提交请求一起传递，不落进计划、不参与任何摘要。
///
/// **非空路由是全映射，不是「只记 staging 的部分映射」。** 最初的版本只记受重定向的文件、
/// 把「不在其中」当作「回落读沙箱」，那让「组装方漏记一个文件」与「这个文件本来就该读原包」
/// 变成同一件事——漏记的文件会拿未重定向的原包字节写进重定向后的目标路径，装上去了、
/// 不报错，内容是错的。现在两种来源都显式记录，提交侧因此能要求**计划里每个动作都被覆盖**
/// （见 `ConfiguredInstallCommitter`）。
///
/// 空路由仍是有意义的一档：整个计划都不涉及 staging（未重定向的标准安装）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetargetSourceRouting {
    origins_by_package_file: BTreeMap<PackageFileId, RetargetSourceOrigin>,
}

impl RetargetSourceRouting {
    pub fn empty() -> Self {
        Self::default()
    }

    /// 记录一个受重定向文件的归属。同一个 `package_file_id` 被声明两次是组装错误：
    /// 谁的来源都可能是对的，拿错了就静默装错内容，所以在这里就拒绝。
    pub fn stage(
        &mut self,
        package_file_id: PackageFileId,
        binding_id: ReplacementBindingId,
    ) -> Result<(), RetargetError> {
        self.insert(package_file_id, RetargetSourceOrigin::Staged(binding_id))
    }

    /// 记录一个原样安装的文件：从沙箱原包读。
    pub fn read_from_package(
        &mut self,
        package_file_id: PackageFileId,
    ) -> Result<(), RetargetError> {
        self.insert(package_file_id, RetargetSourceOrigin::ImportedPackage)
    }

    fn insert(
        &mut self,
        package_file_id: PackageFileId,
        origin: RetargetSourceOrigin,
    ) -> Result<(), RetargetError> {
        match self.origins_by_package_file.entry(package_file_id) {
            Entry::Vacant(entry) => {
                entry.insert(origin);
                Ok(())
            }
            Entry::Occupied(entry) => Err(RetargetError::AmbiguousSourceRouting {
                package_file_id: entry.key().as_str().to_owned(),
            }),
        }
    }

    pub fn merge(&mut self, other: Self) -> Result<(), RetargetError> {
        for (package_file_id, origin) in other.origins_by_package_file {
            self.insert(package_file_id, origin)?;
        }
        Ok(())
    }

    pub fn origin_for(&self, package_file_id: &PackageFileId) -> Option<&RetargetSourceOrigin> {
        self.origins_by_package_file.get(package_file_id)
    }

    /// 这个文件走哪个绑定的 staging 根。原样安装的文件返回 `None`。
    pub fn staged_binding_for(
        &self,
        package_file_id: &PackageFileId,
    ) -> Option<&ReplacementBindingId> {
        match self.origins_by_package_file.get(package_file_id) {
            Some(RetargetSourceOrigin::Staged(binding_id)) => Some(binding_id),
            Some(RetargetSourceOrigin::ImportedPackage) | None => None,
        }
    }

    /// 路由是否给这个文件指定了来源。提交侧用它逐动作核对覆盖面。
    pub fn covers(&self, package_file_id: &PackageFileId) -> bool {
        self.origins_by_package_file.contains_key(package_file_id)
    }

    pub fn is_empty(&self) -> bool {
        self.origins_by_package_file.is_empty()
    }

    pub fn len(&self) -> usize {
        self.origins_by_package_file.len()
    }

    pub fn entries(&self) -> impl Iterator<Item = (&PackageFileId, &RetargetSourceOrigin)> {
        self.origins_by_package_file.iter()
    }

    /// 只有走 staging 的那些文件及其绑定。
    pub fn staged_entries(&self) -> impl Iterator<Item = (&PackageFileId, &ReplacementBindingId)> {
        self.origins_by_package_file
            .iter()
            .filter_map(|(package_file_id, origin)| match origin {
                RetargetSourceOrigin::Staged(binding_id) => Some((package_file_id, binding_id)),
                RetargetSourceOrigin::ImportedPackage => None,
            })
    }

    /// 路由里是否有任何文件要读沙箱原包——提交侧据此决定是否构造沙箱读取器。
    pub fn reads_imported_package(&self) -> bool {
        self.origins_by_package_file
            .values()
            .any(|origin| matches!(origin, RetargetSourceOrigin::ImportedPackage))
    }

    /// 涉及的绑定集合——提交后按它清理 staging 目录。
    pub fn binding_ids(&self) -> BTreeSet<ReplacementBindingId> {
        self.staged_entries()
            .map(|(_, binding_id)| binding_id.clone())
            .collect()
    }
}

fn hash_transform_field(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}
