use hmm_core::{BatchAttemptStatus, BatchExecutionPolicy, BatchItemStatus, BatchOperation};
use serde::{Deserialize, Serialize};

/// Input request shared by `preview_batch_mod_lifecycle` and `seal_batch_mod_lifecycle`.
///
/// The frontend only submits controlled ids (`gameId`/`profileId`/`modId`/revision ids) and
/// selection facts. It never computes install paths, target claims, digests, tokens or
/// replacement binding snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchModLifecycleRequestDto {
    pub schema_version: u32,
    pub operation: BatchModLifecycleOperationDto,
    pub game_id: String,
    pub profile_id: String,
    pub execution_policy: BatchModLifecycleExecutionPolicyDto,
    pub items: Vec<BatchModLifecycleItemInputDto>,
    /// Same-revision reinstall target switches. The backend resolves the binding snapshot from
    /// these controlled ids at seal time; the frontend never submits binding internals.
    #[serde(default)]
    pub replacement_targets: Vec<BatchModLifecycleReplacementTargetDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchModLifecycleOperationDto {
    Install,
    Uninstall,
    Reinstall,
}

impl From<BatchModLifecycleOperationDto> for BatchOperation {
    fn from(value: BatchModLifecycleOperationDto) -> Self {
        match value {
            BatchModLifecycleOperationDto::Install => Self::Install,
            BatchModLifecycleOperationDto::Uninstall => Self::Uninstall,
            BatchModLifecycleOperationDto::Reinstall => Self::Reinstall,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchModLifecycleExecutionPolicyDto {
    StopOnFailure,
    ContinueOnItemFailure,
}

impl From<BatchModLifecycleExecutionPolicyDto> for BatchExecutionPolicy {
    fn from(value: BatchModLifecycleExecutionPolicyDto) -> Self {
        match value {
            BatchModLifecycleExecutionPolicyDto::StopOnFailure => Self::StopOnFailure,
            BatchModLifecycleExecutionPolicyDto::ContinueOnItemFailure => {
                Self::ContinueOnItemFailure
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchModLifecycleReplacementTargetDto {
    pub mod_id: String,
    pub target_id: String,
}

/// Operation-tagged item input. One request allows exactly one operation; the command layer
/// rejects items whose tag differs from the request operation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum BatchModLifecycleItemInputDto {
    Install {
        mod_id: String,
        revision_id: String,
        layer: BatchModLifecycleLayerDto,
    },
    Uninstall {
        mod_id: String,
        expected_installed_revision_id: String,
    },
    Reinstall {
        mod_id: String,
        installed_revision_id: String,
        candidate_revision_id: String,
        layer: BatchModLifecycleLayerDto,
    },
}

impl BatchModLifecycleItemInputDto {
    pub fn operation(&self) -> BatchModLifecycleOperationDto {
        match self {
            Self::Install { .. } => BatchModLifecycleOperationDto::Install,
            Self::Uninstall { .. } => BatchModLifecycleOperationDto::Uninstall,
            Self::Reinstall { .. } => BatchModLifecycleOperationDto::Reinstall,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchModLifecycleLayerDto {
    pub name: String,
    pub priority: i32,
}

// ===== Preview =====

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchModLifecyclePreviewStatusDto {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchModLifecycleReasonSummaryDto {
    pub code: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchModLifecycleActionSummaryDto {
    pub actions: usize,
    pub retained: usize,
    pub replaced: usize,
    pub added: usize,
    pub stale: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchModLifecyclePreviewDto {
    pub status: BatchModLifecyclePreviewStatusDto,
    pub operation: BatchOperation,
    pub execution_policy: BatchExecutionPolicy,
    /// Aggregated item-level blocking reasons across all items.
    pub item_reasons: Vec<BatchModLifecycleReasonSummaryDto>,
    /// Global plan-level blocking reasons.
    pub global_reasons: Vec<BatchModLifecycleReasonSummaryDto>,
    /// Sum of per-item action summaries.
    pub action_summary: BatchModLifecycleActionSummaryDto,
    pub ready_item_count: usize,
    pub blocked_item_count: usize,
    /// Opaque, short-lived preview token; `null` when the plan is blocked.
    pub preview_token: Option<String>,
}

// ===== Seal =====

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchModLifecycleSealStatusDto {
    Sealed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchModLifecycleSealDto {
    pub batch_id: String,
    pub status: BatchModLifecycleSealStatusDto,
    pub operation: BatchOperation,
    pub execution_policy: BatchExecutionPolicy,
    /// Execution validity deadline of `planToken`, not a journal retention bound.
    pub expires_at_unix_millis: u64,
    /// Opaque execution token; hold in memory only and discard after `start`.
    pub plan_token: String,
}

// ===== Started =====

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchModLifecycleStartedDto {
    pub task: crate::dto::TaskStartedDto,
    pub batch_id: String,
    pub attempt_number: u32,
}

// ===== Result page =====

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchModLifecycleResultSummaryDto {
    pub item_count: usize,
    pub succeeded_count: usize,
    pub blocked_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
    pub skipped_count: usize,
    pub recovery_required_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchModLifecycleResultItemDto {
    pub item_id: String,
    pub ordinal: usize,
    pub mod_id: String,
    pub status: BatchItemStatus,
    pub reason_code: Option<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchModLifecycleResultPageDto {
    pub batch_id: String,
    pub attempt_number: u32,
    /// Attempt status uses the full stable batch attempt vocabulary; terminal values are
    /// `completed`, `completed_with_errors`, `blocked`, `cancelled`, `recovery_required`,
    /// `interrupted` and `failed`.
    pub status: BatchAttemptStatus,
    pub task_id: Option<String>,
    pub evidence_health_degraded: bool,
    pub summary: BatchModLifecycleResultSummaryDto,
    /// Page items sorted by ordinal; `nextCursor` is only valid for this exact attempt.
    pub items: Vec<BatchModLifecycleResultItemDto>,
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_request() -> BatchModLifecycleRequestDto {
        serde_json::from_value(json!({
            "schemaVersion": 1,
            "operation": "install",
            "gameId": "mhw",
            "profileId": "default",
            "executionPolicy": "stop_on_failure",
            "items": [{
                "operation": "install",
                "modId": "mod-a",
                "revisionId": "rev-1",
                "layer": { "name": "base", "priority": 0 }
            }]
        }))
        .expect("sample request deserializes")
    }

    #[test]
    fn request_deserializes_with_camel_case_field_names() {
        let request = sample_request();
        assert_eq!(request.schema_version, 1);
        assert_eq!(request.operation, BatchModLifecycleOperationDto::Install);
        assert_eq!(request.game_id, "mhw");
        assert_eq!(request.profile_id, "default");
        assert_eq!(
            request.execution_policy,
            BatchModLifecycleExecutionPolicyDto::StopOnFailure
        );
        assert_eq!(request.items.len(), 1);
        assert!(request.replacement_targets.is_empty());
    }

    #[test]
    fn request_rejects_unknown_top_level_fields() {
        let error = serde_json::from_value::<BatchModLifecycleRequestDto>(json!({
            "schemaVersion": 1,
            "operation": "install",
            "gameId": "mhw",
            "profileId": "default",
            "executionPolicy": "stop_on_failure",
            "items": [],
            "previewToken": "forged"
        }))
        .expect_err("unknown field is rejected");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn request_rejects_unknown_item_and_layer_fields() {
        let error = serde_json::from_value::<BatchModLifecycleRequestDto>(json!({
            "schemaVersion": 1,
            "operation": "install",
            "gameId": "mhw",
            "profileId": "default",
            "executionPolicy": "stop_on_failure",
            "items": [{
                "operation": "install",
                "modId": "mod-a",
                "revisionId": "rev-1",
                "layer": { "name": "base", "priority": 0, "path": "C:/forged" }
            }]
        }))
        .expect_err("unknown layer field is rejected");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn request_rejects_unknown_operation_values() {
        let error = serde_json::from_value::<BatchModLifecycleRequestDto>(json!({
            "schemaVersion": 1,
            "operation": "downgrade",
            "gameId": "mhw",
            "profileId": "default",
            "executionPolicy": "stop_on_failure",
            "items": []
        }))
        .expect_err("unknown operation is rejected");

        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn request_accepts_uninstall_and_reinstall_item_shapes() {
        let request = serde_json::from_value::<BatchModLifecycleRequestDto>(json!({
            "schemaVersion": 1,
            "operation": "reinstall",
            "gameId": "mhw",
            "profileId": "default",
            "executionPolicy": "continue_on_item_failure",
            "items": [{
                "operation": "reinstall",
                "modId": "mod-a",
                "installedRevisionId": "rev-1",
                "candidateRevisionId": "rev-2",
                "layer": { "name": "base", "priority": 0 }
            }],
            "replacementTargets": [{ "modId": "mod-a", "targetId": "target-1" }]
        }))
        .expect("reinstall request deserializes");

        assert_eq!(request.operation, BatchModLifecycleOperationDto::Reinstall);
        assert_eq!(request.replacement_targets.len(), 1);
        assert_eq!(request.replacement_targets[0].mod_id, "mod-a");
    }

    #[test]
    fn started_dto_serializes_camel_case_with_nested_task() {
        let dto = BatchModLifecycleStartedDto {
            task: crate::dto::TaskStartedDto {
                task_id: "batch-task-1".to_owned(),
                kind: crate::dto::TaskKindDto::Install,
                status: crate::dto::TaskStatusDto::Completed,
            },
            batch_id: "batch-1".to_owned(),
            attempt_number: 0,
        };
        let value = serde_json::to_value(&dto).expect("serialize started dto");

        assert_eq!(value["task"]["taskId"], "batch-task-1");
        assert_eq!(value["task"]["kind"], "install");
        assert_eq!(value["task"]["status"], "completed");
        assert_eq!(value["batchId"], "batch-1");
        assert_eq!(value["attemptNumber"], 0);
    }

    #[test]
    fn preview_dto_serializes_stable_status_and_summaries() {
        let dto = BatchModLifecyclePreviewDto {
            status: BatchModLifecyclePreviewStatusDto::Blocked,
            operation: BatchOperation::Install,
            execution_policy: BatchExecutionPolicy::StopOnFailure,
            item_reasons: vec![BatchModLifecycleReasonSummaryDto {
                code: "install_plan_conflict".to_owned(),
                count: 2,
            }],
            global_reasons: vec![BatchModLifecycleReasonSummaryDto {
                code: "batch_global_target_conflict".to_owned(),
                count: 1,
            }],
            action_summary: BatchModLifecycleActionSummaryDto {
                actions: 3,
                retained: 0,
                replaced: 0,
                added: 3,
                stale: 0,
            },
            ready_item_count: 0,
            blocked_item_count: 2,
            preview_token: None,
        };
        let value = serde_json::to_value(&dto).expect("serialize preview dto");

        assert_eq!(value["status"], "blocked");
        assert_eq!(value["operation"], "install");
        assert_eq!(value["executionPolicy"], "stop_on_failure");
        assert_eq!(value["itemReasons"][0]["code"], "install_plan_conflict");
        assert_eq!(value["globalReasons"][0]["count"], 1);
        assert_eq!(value["actionSummary"]["added"], 3);
        assert_eq!(value["readyItemCount"], 0);
        assert_eq!(value["blockedItemCount"], 2);
        assert!(value["previewToken"].is_null());
    }

    #[test]
    fn result_page_serializes_attempt_binding_and_counts() {
        let dto = BatchModLifecycleResultPageDto {
            batch_id: "batch-1".to_owned(),
            attempt_number: 1,
            status: BatchAttemptStatus::CompletedWithErrors,
            task_id: Some("batch-task-1".to_owned()),
            evidence_health_degraded: false,
            summary: BatchModLifecycleResultSummaryDto {
                item_count: 2,
                succeeded_count: 1,
                blocked_count: 0,
                failed_count: 1,
                cancelled_count: 0,
                skipped_count: 0,
                recovery_required_count: 0,
            },
            items: vec![BatchModLifecycleResultItemDto {
                item_id: "item-1".to_owned(),
                ordinal: 0,
                mod_id: "mod-a".to_owned(),
                status: BatchItemStatus::Succeeded,
                reason_code: None,
                retryable: false,
            }],
            next_cursor: None,
        };
        let value = serde_json::to_value(&dto).expect("serialize result page dto");

        assert_eq!(value["attemptNumber"], 1);
        assert_eq!(value["status"], "completed_with_errors");
        assert_eq!(value["taskId"], "batch-task-1");
        assert_eq!(value["summary"]["succeededCount"], 1);
        assert_eq!(value["items"][0]["itemId"], "item-1");
        assert_eq!(value["items"][0]["status"], "succeeded");
        assert_eq!(value["items"][0]["retryable"], false);
        assert!(value["nextCursor"].is_null());
    }
}
