use crate::{
    FileLayer, GameId, InstallTargetPath, ModId, ModRevisionId, ProfileId,
    ReplacementBindingSnapshot,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const BATCH_PLAN_SCHEMA_VERSION: u32 = 1;
pub const BATCH_RESOURCE_LIMITS_VERSION: u32 = 1;
pub const DEFAULT_BATCH_MAX_ITEMS: usize = 100;
pub const DEFAULT_BATCH_MAX_TARGET_ACTIONS: usize = 50_000;
pub const DEFAULT_BATCH_MAX_CANONICAL_BYTES: usize = 16 * 1024 * 1024;
pub const DEFAULT_BATCH_PREVIEW_TOKEN_TTL_MILLIS: u128 = 30 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchOperation {
    Install,
    Uninstall,
    Reinstall,
}

impl BatchOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Uninstall => "uninstall",
            Self::Reinstall => "reinstall",
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum BatchExecutionPolicy {
    #[default]
    StopOnFailure,
    ContinueOnItemFailure,
}

impl BatchExecutionPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StopOnFailure => "stop_on_failure",
            Self::ContinueOnItemFailure => "continue_on_item_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallBatchItemInput {
    pub mod_id: ModId,
    pub revision_id: ModRevisionId,
    pub layer: FileLayer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_binding_snapshot: Option<ReplacementBindingSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UninstallBatchItemInput {
    pub mod_id: ModId,
    pub expected_installed_revision_id: ModRevisionId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReinstallBatchItemInput {
    pub mod_id: ModId,
    pub installed_revision_id: ModRevisionId,
    pub candidate_revision_id: ModRevisionId,
    pub layer: FileLayer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_binding_snapshot: Option<ReplacementBindingSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "snake_case",
    rename_all_fields = "snake_case"
)]
pub enum BatchItemInput {
    Install(InstallBatchItemInput),
    Uninstall(UninstallBatchItemInput),
    Reinstall(ReinstallBatchItemInput),
}

impl BatchItemInput {
    pub fn operation(&self) -> BatchOperation {
        match self {
            Self::Install(_) => BatchOperation::Install,
            Self::Uninstall(_) => BatchOperation::Uninstall,
            Self::Reinstall(_) => BatchOperation::Reinstall,
        }
    }

    pub fn mod_id(&self) -> &ModId {
        match self {
            Self::Install(input) => &input.mod_id,
            Self::Uninstall(input) => &input.mod_id,
            Self::Reinstall(input) => &input.mod_id,
        }
    }

    pub fn stable_key(&self) -> String {
        format!("{}\0{}", self.operation().as_str(), self.mod_id().as_str())
    }

    fn validate(
        &self,
        request_operation: BatchOperation,
        profile_id: &ProfileId,
    ) -> Result<(), BatchPlanError> {
        if self.operation() != request_operation {
            return Err(BatchPlanError::InputOperationMismatch);
        }
        if self.mod_id().as_str().trim().is_empty() {
            return Err(BatchPlanError::InvalidInput);
        }

        match self {
            Self::Install(input) => {
                if input.revision_id.as_str().trim().is_empty()
                    || input.layer.name.trim().is_empty()
                {
                    return Err(BatchPlanError::InvalidInput);
                }
                validate_binding(
                    input.replacement_binding_snapshot.as_ref(),
                    &input.mod_id,
                    profile_id,
                )?;
            }
            Self::Uninstall(input) => {
                if input
                    .expected_installed_revision_id
                    .as_str()
                    .trim()
                    .is_empty()
                {
                    return Err(BatchPlanError::InvalidInput);
                }
            }
            Self::Reinstall(input) => {
                if input.installed_revision_id.as_str().trim().is_empty()
                    || input.candidate_revision_id.as_str().trim().is_empty()
                    || input.layer.name.trim().is_empty()
                {
                    return Err(BatchPlanError::InvalidInput);
                }
                validate_binding(
                    input.replacement_binding_snapshot.as_ref(),
                    &input.mod_id,
                    profile_id,
                )?;
            }
        }
        Ok(())
    }
}

fn validate_binding(
    binding: Option<&ReplacementBindingSnapshot>,
    mod_id: &ModId,
    profile_id: &ProfileId,
) -> Result<(), BatchPlanError> {
    if let Some(binding) = binding {
        if binding.mod_id() != mod_id || binding.profile_id() != profile_id {
            return Err(BatchPlanError::InvalidInput);
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchPlanRequest {
    pub schema_version: u32,
    pub operation: BatchOperation,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub execution_policy: BatchExecutionPolicy,
    pub items: Vec<BatchItemInput>,
}

impl BatchPlanRequest {
    pub fn normalize(self) -> Result<NormalizedBatchPlanRequest, BatchPlanError> {
        if self.schema_version != BATCH_PLAN_SCHEMA_VERSION
            || self.items.is_empty()
            || self.profile_id.as_str().trim().is_empty()
        {
            return Err(BatchPlanError::InvalidInput);
        }
        if self.items.len() > DEFAULT_BATCH_MAX_ITEMS {
            return Err(BatchPlanError::ResourceLimitExceeded {
                resource: BatchResource::Items,
            });
        }

        let mut items = self.items;
        for item in &items {
            item.validate(self.operation, &self.profile_id)?;
        }
        items.sort_by_key(BatchItemInput::stable_key);
        let mut seen = BTreeSet::new();
        if items.iter().any(|item| !seen.insert(item.mod_id().clone())) {
            return Err(BatchPlanError::DuplicateItem);
        }

        Ok(NormalizedBatchPlanRequest {
            schema_version: self.schema_version,
            operation: self.operation,
            game_id: self.game_id,
            profile_id: self.profile_id,
            execution_policy: self.execution_policy,
            items,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedBatchPlanRequest {
    pub schema_version: u32,
    pub operation: BatchOperation,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub execution_policy: BatchExecutionPolicy,
    pub items: Vec<BatchItemInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchTargetWriteKind {
    Install,
    Remove,
    Restore,
}

impl BatchTargetWriteKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Remove => "remove",
            Self::Restore => "restore",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchTargetClaim {
    pub target_path: InstallTargetPath,
    pub kind: BatchTargetWriteKind,
}

impl BatchTargetClaim {
    pub fn windows_key(&self) -> String {
        self.target_path.as_str().to_ascii_lowercase()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchPreflightStatus {
    Ready,
    Warning,
    Blocked,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchPreflightDecision {
    pub status: BatchPreflightStatus,
    pub rules_version: Option<u32>,
    #[serde(default)]
    pub codes: Vec<String>,
}

impl BatchPreflightDecision {
    pub fn is_blocked(&self) -> bool {
        matches!(
            self.status,
            BatchPreflightStatus::Blocked | BatchPreflightStatus::Unverified
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchActionSummary {
    pub actions: usize,
    pub retained: usize,
    pub replaced: usize,
    pub added: usize,
    pub stale: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchItemFacts {
    pub mod_id: ModId,
    pub source_revision_id: Option<ModRevisionId>,
    pub installed_revision_id: Option<ModRevisionId>,
    pub fact_digest: String,
    pub single_plan_digest: String,
    pub target_claims: Vec<BatchTargetClaim>,
    pub action_summary: BatchActionSummary,
    pub prerequisite: BatchPreflightDecision,
    #[serde(default)]
    pub blocking_reasons: Vec<String>,
    #[serde(default)]
    pub warning_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchPlanFacts {
    pub environment_digest: String,
    pub prerequisite_rules_version: Option<u32>,
    pub items: Vec<BatchItemFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchResourceLimits {
    pub version: u32,
    pub max_items: usize,
    pub max_target_actions: usize,
    pub max_canonical_bytes: usize,
}

impl Default for BatchResourceLimits {
    fn default() -> Self {
        Self {
            version: BATCH_RESOURCE_LIMITS_VERSION,
            max_items: DEFAULT_BATCH_MAX_ITEMS,
            max_target_actions: DEFAULT_BATCH_MAX_TARGET_ACTIONS,
            max_canonical_bytes: DEFAULT_BATCH_MAX_CANONICAL_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchResourceUsage {
    pub item_count: usize,
    pub target_action_count: usize,
    pub canonical_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchReasonSummary {
    pub code: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchItemPlan {
    pub ordinal: usize,
    pub input_snapshot: BatchItemInput,
    pub source_revision_id: Option<ModRevisionId>,
    pub installed_revision_id: Option<ModRevisionId>,
    pub fact_digest: String,
    pub single_plan_digest: String,
    pub prerequisite: BatchPreflightDecision,
    pub target_claims: Vec<BatchTargetClaim>,
    pub action_summary: BatchActionSummary,
    #[serde(default)]
    pub blocking_reasons: Vec<String>,
    #[serde(default)]
    pub warning_codes: Vec<String>,
}

impl BatchItemPlan {
    pub fn is_ready(&self) -> bool {
        self.blocking_reasons.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchPlan {
    pub plan_schema_version: u32,
    pub operation: BatchOperation,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub execution_policy: BatchExecutionPolicy,
    pub items: Vec<BatchItemPlan>,
    pub environment_digest: String,
    pub prerequisite_rules_version: Option<u32>,
    pub resource_limits: BatchResourceLimits,
    pub resource_usage: BatchResourceUsage,
    pub global_target_claims_digest: String,
    pub batch_digest: String,
    #[serde(default)]
    pub global_blocking_reasons: Vec<BatchReasonSummary>,
    #[serde(default)]
    pub warning_codes: Vec<BatchReasonSummary>,
}

impl BatchPlan {
    pub fn status(&self) -> BatchPlanStatus {
        if !self.global_blocking_reasons.is_empty() {
            return BatchPlanStatus::Blocked;
        }
        let ready = self.items.iter().filter(|item| item.is_ready()).count();
        let blocked = self.items.len().saturating_sub(ready);
        match self.execution_policy {
            BatchExecutionPolicy::StopOnFailure if blocked > 0 => BatchPlanStatus::Blocked,
            BatchExecutionPolicy::ContinueOnItemFailure if ready == 0 => BatchPlanStatus::Blocked,
            _ => BatchPlanStatus::Ready,
        }
    }

    pub fn is_ready(&self) -> bool {
        self.status() == BatchPlanStatus::Ready
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchPlanStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BatchPlanError {
    #[error("batch input is invalid")]
    InvalidInput,
    #[error("batch input operation does not match request operation")]
    InputOperationMismatch,
    #[error("batch contains a duplicate Mod item")]
    DuplicateItem,
    #[error("batch facts do not match normalized request")]
    FactsMismatch,
    #[error("batch facts are unavailable")]
    FactsUnavailable,
    #[error("batch resource limit exceeded: {resource:?}")]
    ResourceLimitExceeded { resource: BatchResource },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchResource {
    Items,
    TargetActions,
    CanonicalBytes,
}

#[derive(Serialize)]
struct CanonicalBatchPlan<'a> {
    plan_schema_version: u32,
    operation: BatchOperation,
    game_id: &'a GameId,
    profile_id: &'a ProfileId,
    execution_policy: BatchExecutionPolicy,
    environment_digest: &'a str,
    prerequisite_rules_version: Option<u32>,
    resource_limits_version: u32,
    global_target_claims_digest: &'a str,
    items: Vec<CanonicalBatchItem<'a>>,
}

#[derive(Serialize)]
struct CanonicalBatchItem<'a> {
    ordinal: usize,
    input: CanonicalBatchItemInput<'a>,
    source_revision_id: Option<&'a str>,
    installed_revision_id: Option<&'a str>,
    fact_digest: &'a str,
    single_plan_digest: &'a str,
    prerequisite: &'a BatchPreflightDecision,
    target_claims: Vec<CanonicalTargetClaim<'a>>,
    action_summary: &'a BatchActionSummary,
    blocking_reasons: &'a [String],
    warning_codes: &'a [String],
}

#[derive(Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
enum CanonicalBatchItemInput<'a> {
    Install {
        mod_id: &'a str,
        revision_id: &'a str,
        layer: CanonicalLayer<'a>,
        replacement_binding: Option<CanonicalBinding<'a>>,
    },
    Uninstall {
        mod_id: &'a str,
        expected_installed_revision_id: &'a str,
    },
    Reinstall {
        mod_id: &'a str,
        installed_revision_id: &'a str,
        candidate_revision_id: &'a str,
        layer: CanonicalLayer<'a>,
        replacement_binding: Option<CanonicalBinding<'a>>,
    },
}

#[derive(Serialize)]
struct CanonicalLayer<'a> {
    name: &'a str,
    priority: i32,
}

#[derive(Serialize)]
struct CanonicalBinding<'a> {
    binding_id: &'a str,
    mod_id: &'a str,
    profile_id: &'a str,
    source_id: &'a str,
    target_id: &'a str,
    revision_id: Option<&'a str>,
    source_internal_id: &'a str,
    target_internal_id: &'a str,
    source_path_family: &'a str,
    target_path_family: &'a str,
    retarget_kind: &'a crate::ReplacementTargetKind,
}

#[derive(Serialize)]
struct CanonicalTargetClaim<'a> {
    target: &'a str,
    kind: BatchTargetWriteKind,
}

pub fn build_batch_plan(
    request: NormalizedBatchPlanRequest,
    facts: BatchPlanFacts,
    resource_limits: BatchResourceLimits,
) -> Result<BatchPlan, BatchPlanError> {
    if request.items.len() > resource_limits.max_items {
        return Err(BatchPlanError::ResourceLimitExceeded {
            resource: BatchResource::Items,
        });
    }
    if facts.items.len() != request.items.len()
        || facts.items.iter().any(|fact| {
            request
                .items
                .iter()
                .all(|item| item.mod_id() != &fact.mod_id)
        })
    {
        return Err(BatchPlanError::FactsMismatch);
    }

    let facts_by_mod = facts
        .items
        .into_iter()
        .map(|fact| (fact.mod_id.clone(), fact))
        .collect::<BTreeMap<_, _>>();
    let mut items = Vec::with_capacity(request.items.len());
    let mut target_owners: BTreeMap<String, BTreeSet<ModId>> = BTreeMap::new();
    let mut target_claims = Vec::new();
    let mut total_actions = 0usize;

    for (ordinal, input) in request.items.iter().cloned().enumerate() {
        let fact = facts_by_mod
            .get(input.mod_id())
            .ok_or(BatchPlanError::FactsMismatch)?;
        let mut blocking_reasons = fact.blocking_reasons.clone();
        append_revision_mismatch_reason(&mut blocking_reasons, &input, fact);
        if fact.prerequisite.is_blocked() {
            blocking_reasons.push("prerequisite_blocked".to_owned());
        }
        deduplicate_strings(&mut blocking_reasons);
        let mut warning_codes = fact.warning_codes.clone();
        if fact.prerequisite.status == BatchPreflightStatus::Warning {
            warning_codes.extend(fact.prerequisite.codes.iter().cloned());
        }
        deduplicate_strings(&mut warning_codes);
        total_actions = total_actions.saturating_add(fact.action_summary.actions);
        let mut item_target_claims = fact.target_claims.clone();
        item_target_claims.sort_by(|left, right| {
            left.windows_key()
                .cmp(&right.windows_key())
                .then_with(|| left.kind.as_str().cmp(right.kind.as_str()))
        });
        for claim in &item_target_claims {
            target_owners
                .entry(claim.windows_key())
                .or_default()
                .insert(fact.mod_id.clone());
            target_claims.push(claim.clone());
        }
        items.push(BatchItemPlan {
            ordinal,
            input_snapshot: input,
            source_revision_id: fact.source_revision_id.clone(),
            installed_revision_id: fact.installed_revision_id.clone(),
            fact_digest: fact.fact_digest.clone(),
            single_plan_digest: fact.single_plan_digest.clone(),
            prerequisite: fact.prerequisite.clone(),
            target_claims: item_target_claims,
            action_summary: fact.action_summary.clone(),
            blocking_reasons,
            warning_codes,
        });
    }
    if total_actions > resource_limits.max_target_actions {
        return Err(BatchPlanError::ResourceLimitExceeded {
            resource: BatchResource::TargetActions,
        });
    }

    let conflict_count = target_owners
        .values()
        .filter(|owners| owners.len() > 1)
        .count();
    let mut global_blocking_reasons = Vec::new();
    if conflict_count > 0 {
        global_blocking_reasons.push(BatchReasonSummary {
            code: "batch_global_target_conflict".to_owned(),
            count: conflict_count,
        });
    }
    let global_target_claims_digest = digest_target_claims(&target_claims);
    let mut warning_counts = BTreeMap::<String, usize>::new();
    for item in &items {
        for code in &item.warning_codes {
            *warning_counts.entry(code.clone()).or_default() += 1;
        }
    }
    let warning_codes = warning_counts
        .into_iter()
        .map(|(code, count)| BatchReasonSummary { code, count })
        .collect::<Vec<_>>();
    let mut plan = BatchPlan {
        plan_schema_version: BATCH_PLAN_SCHEMA_VERSION,
        operation: request.operation,
        game_id: request.game_id,
        profile_id: request.profile_id,
        execution_policy: request.execution_policy,
        items,
        environment_digest: facts.environment_digest,
        prerequisite_rules_version: facts.prerequisite_rules_version,
        resource_limits,
        resource_usage: BatchResourceUsage {
            item_count: request.items.len(),
            target_action_count: total_actions,
            canonical_bytes: 0,
        },
        global_target_claims_digest,
        batch_digest: String::new(),
        global_blocking_reasons,
        warning_codes,
    };
    let canonical = canonical_bytes(&plan);
    if canonical.len() > plan.resource_limits.max_canonical_bytes {
        return Err(BatchPlanError::ResourceLimitExceeded {
            resource: BatchResource::CanonicalBytes,
        });
    }
    plan.resource_usage.canonical_bytes = canonical.len();
    plan.batch_digest = sha256_digest(&canonical);
    Ok(plan)
}

fn canonical_bytes(plan: &BatchPlan) -> Vec<u8> {
    serde_json::to_vec(&CanonicalBatchPlan {
        plan_schema_version: plan.plan_schema_version,
        operation: plan.operation,
        game_id: &plan.game_id,
        profile_id: &plan.profile_id,
        execution_policy: plan.execution_policy,
        environment_digest: &plan.environment_digest,
        prerequisite_rules_version: plan.prerequisite_rules_version,
        resource_limits_version: plan.resource_limits.version,
        global_target_claims_digest: &plan.global_target_claims_digest,
        items: plan.items.iter().map(canonical_item).collect::<Vec<_>>(),
    })
    .expect("batch canonical plan contains only serializable validated values")
}

fn canonical_item(item: &BatchItemPlan) -> CanonicalBatchItem<'_> {
    let input = match &item.input_snapshot {
        BatchItemInput::Install(input) => CanonicalBatchItemInput::Install {
            mod_id: input.mod_id.as_str(),
            revision_id: input.revision_id.as_str(),
            layer: CanonicalLayer {
                name: &input.layer.name,
                priority: input.layer.priority,
            },
            replacement_binding: input
                .replacement_binding_snapshot
                .as_ref()
                .map(canonical_binding),
        },
        BatchItemInput::Uninstall(input) => CanonicalBatchItemInput::Uninstall {
            mod_id: input.mod_id.as_str(),
            expected_installed_revision_id: input.expected_installed_revision_id.as_str(),
        },
        BatchItemInput::Reinstall(input) => CanonicalBatchItemInput::Reinstall {
            mod_id: input.mod_id.as_str(),
            installed_revision_id: input.installed_revision_id.as_str(),
            candidate_revision_id: input.candidate_revision_id.as_str(),
            layer: CanonicalLayer {
                name: &input.layer.name,
                priority: input.layer.priority,
            },
            replacement_binding: input
                .replacement_binding_snapshot
                .as_ref()
                .map(canonical_binding),
        },
    };
    CanonicalBatchItem {
        ordinal: item.ordinal,
        input,
        source_revision_id: item.source_revision_id.as_ref().map(ModRevisionId::as_str),
        installed_revision_id: item
            .installed_revision_id
            .as_ref()
            .map(ModRevisionId::as_str),
        fact_digest: &item.fact_digest,
        single_plan_digest: &item.single_plan_digest,
        prerequisite: &item.prerequisite,
        target_claims: item
            .target_claims
            .iter()
            .map(|claim| CanonicalTargetClaim {
                target: claim.target_path.as_str(),
                kind: claim.kind,
            })
            .collect(),
        action_summary: &item.action_summary,
        blocking_reasons: &item.blocking_reasons,
        warning_codes: &item.warning_codes,
    }
}

fn canonical_binding(binding: &ReplacementBindingSnapshot) -> CanonicalBinding<'_> {
    CanonicalBinding {
        binding_id: binding.binding_id().as_str(),
        mod_id: binding.mod_id().as_str(),
        profile_id: binding.profile_id().as_str(),
        source_id: binding.binding().source_id().as_str(),
        target_id: binding.binding().target_id().as_str(),
        revision_id: binding.revision_id().map(ModRevisionId::as_str),
        source_internal_id: binding.source_internal_id(),
        target_internal_id: binding.target_internal_id(),
        source_path_family: binding.source_path_family(),
        target_path_family: binding.target_path_family(),
        retarget_kind: binding.retarget_kind(),
    }
}

fn append_revision_mismatch_reason(
    blocking_reasons: &mut Vec<String>,
    input: &BatchItemInput,
    fact: &BatchItemFacts,
) {
    let matches = match input {
        BatchItemInput::Install(input) => {
            fact.source_revision_id.as_ref() == Some(&input.revision_id)
        }
        BatchItemInput::Uninstall(input) => {
            fact.installed_revision_id.as_ref() == Some(&input.expected_installed_revision_id)
        }
        BatchItemInput::Reinstall(input) => {
            fact.source_revision_id.as_ref() == Some(&input.candidate_revision_id)
                && fact.installed_revision_id.as_ref() == Some(&input.installed_revision_id)
        }
    };
    if !matches {
        blocking_reasons.push(
            match input {
                BatchItemInput::Install(_) => "source_revision_changed",
                BatchItemInput::Uninstall(_) => "manifest_changed",
                BatchItemInput::Reinstall(_) => "source_revision_changed",
            }
            .to_owned(),
        );
    }
}

fn digest_target_claims(claims: &[BatchTargetClaim]) -> String {
    let mut canonical = claims
        .iter()
        .map(|claim| (claim.windows_key(), claim.kind.as_str()))
        .collect::<Vec<_>>();
    canonical.sort();
    sha256_digest(&serde_json::to_vec(&canonical).expect("target claims are serializable"))
}

fn sha256_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn deduplicate_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(operation: BatchOperation, items: Vec<BatchItemInput>) -> BatchPlanRequest {
        BatchPlanRequest {
            schema_version: BATCH_PLAN_SCHEMA_VERSION,
            operation,
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            execution_policy: BatchExecutionPolicy::StopOnFailure,
            items,
        }
    }

    fn install_item(mod_id: &str, revision_id: &str) -> BatchItemInput {
        BatchItemInput::Install(InstallBatchItemInput {
            mod_id: ModId::new(mod_id),
            revision_id: ModRevisionId::new(revision_id),
            layer: FileLayer::new("default", 10),
            replacement_binding_snapshot: None,
        })
    }

    fn facts(mod_id: &str, target: &str) -> BatchItemFacts {
        facts_with_revision(mod_id, &format!("{mod_id}-revision"), target)
    }

    fn facts_with_revision(mod_id: &str, revision_id: &str, target: &str) -> BatchItemFacts {
        BatchItemFacts {
            mod_id: ModId::new(mod_id),
            source_revision_id: Some(ModRevisionId::new(revision_id)),
            installed_revision_id: None,
            fact_digest: format!("fact-{mod_id}"),
            single_plan_digest: format!("plan-{mod_id}"),
            target_claims: vec![BatchTargetClaim {
                target_path: InstallTargetPath::parse(target, ["nativepc"]).expect("target"),
                kind: BatchTargetWriteKind::Install,
            }],
            action_summary: BatchActionSummary {
                actions: 1,
                ..Default::default()
            },
            prerequisite: BatchPreflightDecision {
                status: BatchPreflightStatus::Ready,
                rules_version: Some(1),
                codes: Vec::new(),
            },
            blocking_reasons: Vec::new(),
            warning_codes: Vec::new(),
        }
    }

    #[test]
    fn normalizes_selection_order_and_digest_deterministically() {
        let first_request = request(
            BatchOperation::Install,
            vec![
                install_item("mod-b", "rev-b"),
                install_item("mod-a", "rev-a"),
            ],
        )
        .normalize()
        .expect("valid request");
        let second_request = request(
            BatchOperation::Install,
            vec![
                install_item("mod-a", "rev-a"),
                install_item("mod-b", "rev-b"),
            ],
        )
        .normalize()
        .expect("valid request");
        let first = build_batch_plan(
            first_request,
            BatchPlanFacts {
                environment_digest: "env".to_owned(),
                prerequisite_rules_version: Some(1),
                items: vec![facts("mod-a", "nativepc/a"), facts("mod-b", "nativepc/b")],
            },
            BatchResourceLimits::default(),
        )
        .expect("plan");
        let second = build_batch_plan(
            second_request,
            BatchPlanFacts {
                environment_digest: "env".to_owned(),
                prerequisite_rules_version: Some(1),
                items: vec![facts("mod-b", "nativepc/b"), facts("mod-a", "nativepc/a")],
            },
            BatchResourceLimits::default(),
        )
        .expect("plan");
        assert_eq!(first.batch_digest, second.batch_digest);
        assert_eq!(first.items[0].input_snapshot.mod_id(), &ModId::new("mod-a"));
    }

    #[test]
    fn duplicate_items_and_limits_are_rejected() {
        let duplicate = request(
            BatchOperation::Install,
            vec![install_item("same", "a"), install_item("same", "b")],
        )
        .normalize();
        assert_eq!(duplicate, Err(BatchPlanError::DuplicateItem));

        let too_many = request(
            BatchOperation::Install,
            (0..=DEFAULT_BATCH_MAX_ITEMS)
                .map(|index| install_item(&format!("mod-{index}"), "revision"))
                .collect(),
        )
        .normalize();
        assert!(matches!(
            too_many,
            Err(BatchPlanError::ResourceLimitExceeded {
                resource: BatchResource::Items
            })
        ));
    }

    #[test]
    fn target_action_and_canonical_byte_limits_are_rejected() {
        let normalized = request(BatchOperation::Install, vec![install_item("a", "a")])
            .normalize()
            .expect("request");
        let mut over_action_limit = facts_with_revision("a", "a", "nativepc/a");
        over_action_limit.action_summary.actions = DEFAULT_BATCH_MAX_TARGET_ACTIONS + 1;
        assert!(matches!(
            build_batch_plan(
                normalized.clone(),
                BatchPlanFacts {
                    environment_digest: "env".to_owned(),
                    prerequisite_rules_version: Some(1),
                    items: vec![over_action_limit],
                },
                BatchResourceLimits::default(),
            ),
            Err(BatchPlanError::ResourceLimitExceeded {
                resource: BatchResource::TargetActions
            })
        ));

        assert!(matches!(
            build_batch_plan(
                normalized,
                BatchPlanFacts {
                    environment_digest: "env".to_owned(),
                    prerequisite_rules_version: Some(1),
                    items: vec![facts_with_revision("a", "a", "nativepc/a")],
                },
                BatchResourceLimits {
                    max_canonical_bytes: 1,
                    ..Default::default()
                },
            ),
            Err(BatchPlanError::ResourceLimitExceeded {
                resource: BatchResource::CanonicalBytes
            })
        ));
    }

    #[test]
    fn target_conflicts_use_windows_semantics() {
        let normalized = request(
            BatchOperation::Install,
            vec![install_item("a", "a"), install_item("b", "b")],
        )
        .normalize()
        .expect("request");
        let plan = build_batch_plan(
            normalized,
            BatchPlanFacts {
                environment_digest: "env".to_owned(),
                prerequisite_rules_version: Some(1),
                items: vec![
                    facts("a", "nativepc/Foo\\Bar"),
                    facts("b", "nativepc/foo/bar"),
                ],
            },
            BatchResourceLimits::default(),
        )
        .expect("plan");
        assert_eq!(plan.status(), BatchPlanStatus::Blocked);
        assert_eq!(
            plan.global_blocking_reasons[0].code,
            "batch_global_target_conflict"
        );
    }

    #[test]
    fn continue_policy_can_be_ready_with_isolated_blocker() {
        let mut normalized = request(
            BatchOperation::Install,
            vec![install_item("a", "a"), install_item("b", "b")],
        )
        .normalize()
        .expect("request");
        normalized.execution_policy = BatchExecutionPolicy::ContinueOnItemFailure;
        let mut blocked = facts_with_revision("a", "a", "nativepc/a");
        blocked
            .blocking_reasons
            .push("source_revision_changed".to_owned());
        let plan = build_batch_plan(
            normalized,
            BatchPlanFacts {
                environment_digest: "env".to_owned(),
                prerequisite_rules_version: Some(1),
                items: vec![blocked, facts_with_revision("b", "b", "nativepc/b")],
            },
            BatchResourceLimits::default(),
        )
        .expect("plan");
        assert_eq!(plan.status(), BatchPlanStatus::Ready);
        assert_eq!(plan.items.iter().filter(|item| item.is_ready()).count(), 1);
    }
}
