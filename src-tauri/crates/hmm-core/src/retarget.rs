use crate::{
    ContentTransformInvocation, GameId, InstallTargetPath, PackageFileId, ReplacementAdapterFacts,
    ReplacementBinding, ReplacementSourceId, ReplacementTargetKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
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
}

fn hash_transform_field(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}
