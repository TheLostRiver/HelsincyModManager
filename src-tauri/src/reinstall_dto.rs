use crate::dto::GamePrerequisiteDecisionDto;
use crate::retarget_file_dto::RetargetFilePreviewDto;
use hmm_app::{
    InstallManifestStatus, InstallRecoveryActionKind, InstallRecoveryStatus, ModRevisionList,
    ReinstallBlockingReason, ReinstallBlockingReasonSummary, ReinstallPlanPreview,
    ReinstallPreviewStatus, ReinstallRevisionSummary, ReinstallTargetCounts,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartImportModRevisionTaskRequestDto {
    pub archive_path: String,
    pub mod_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReinstallFileLayerDto {
    pub name: String,
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewReinstallPlanRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub candidate_revision_id: String,
    pub layer: ReinstallFileLayerDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartReinstallTaskRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub mod_id: String,
    pub candidate_revision_id: String,
    pub layer: ReinstallFileLayerDto,
    pub plan_token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRevisionSummaryDto {
    pub revision_id: String,
}

impl From<ReinstallRevisionSummary> for ModRevisionSummaryDto {
    fn from(summary: ReinstallRevisionSummary) -> Self {
        Self {
            revision_id: summary.revision_id.as_str().to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRevisionListDto {
    pub mod_id: String,
    pub origin_revision_id: String,
    pub display_revision_id: String,
    pub revisions: Vec<ModRevisionSummaryDto>,
}

impl From<ModRevisionList> for ModRevisionListDto {
    fn from(list: ModRevisionList) -> Self {
        Self {
            mod_id: list.mod_id.as_str().to_owned(),
            origin_revision_id: list.origin_revision_id.as_str().to_owned(),
            display_revision_id: list.display_revision_id.as_str().to_owned(),
            revisions: list
                .revision_ids
                .into_iter()
                .map(|revision_id| ModRevisionSummaryDto {
                    revision_id: revision_id.as_str().to_owned(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReinstallTargetCountsDto {
    pub retained: usize,
    pub replaced: usize,
    pub added: usize,
    pub stale: usize,
}

impl From<ReinstallTargetCounts> for ReinstallTargetCountsDto {
    fn from(counts: ReinstallTargetCounts) -> Self {
        Self {
            retained: counts.retained,
            replaced: counts.replaced,
            added: counts.added,
            stale: counts.stale,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReinstallBlockingReasonDto {
    PrerequisitesBlocked,
    NotInstalled,
    CandidateNotFound,
    CandidateNotReady,
    OriginalInstallUnverified,
    InstalledAttachmentUnverified,
    CandidateOwnerMismatch,
    CandidateAlreadyInstalled,
    ManifestStateUnsafe,
    InstalledRevisionUnknown,
    SourceUnavailable,
    TargetMissing,
    TargetChanged,
    TargetReadFailed,
    BackupMissing,
    BackupReadFailed,
    PlanConflict,
    CrossModTargetConflict,
    #[allow(
        dead_code,
        reason = "reserved by the stable Task 7 preview blocking-reason contract"
    )]
    PreviewStale,
}

impl From<ReinstallBlockingReason> for ReinstallBlockingReasonDto {
    fn from(reason: ReinstallBlockingReason) -> Self {
        match reason {
            ReinstallBlockingReason::PrerequisitesBlocked => Self::PrerequisitesBlocked,
            ReinstallBlockingReason::NotInstalled => Self::NotInstalled,
            ReinstallBlockingReason::CandidateNotFound => Self::CandidateNotFound,
            ReinstallBlockingReason::CandidateNotReady => Self::CandidateNotReady,
            ReinstallBlockingReason::OriginalInstallUnverified => Self::OriginalInstallUnverified,
            ReinstallBlockingReason::InstalledAttachmentUnverified => {
                Self::InstalledAttachmentUnverified
            }
            ReinstallBlockingReason::CandidateOwnerMismatch => Self::CandidateOwnerMismatch,
            ReinstallBlockingReason::CandidateAlreadyInstalled => Self::CandidateAlreadyInstalled,
            ReinstallBlockingReason::ManifestStateUnsafe => Self::ManifestStateUnsafe,
            ReinstallBlockingReason::InstalledRevisionUnknown => Self::InstalledRevisionUnknown,
            ReinstallBlockingReason::SourceUnavailable => Self::SourceUnavailable,
            ReinstallBlockingReason::TargetMissing => Self::TargetMissing,
            ReinstallBlockingReason::TargetChanged => Self::TargetChanged,
            ReinstallBlockingReason::TargetReadFailed => Self::TargetReadFailed,
            ReinstallBlockingReason::BackupMissing => Self::BackupMissing,
            ReinstallBlockingReason::BackupReadFailed => Self::BackupReadFailed,
            ReinstallBlockingReason::PlanConflict => Self::PlanConflict,
            ReinstallBlockingReason::CrossModTargetConflict => Self::CrossModTargetConflict,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReinstallBlockingReasonSummaryDto {
    pub code: ReinstallBlockingReasonDto,
    pub count: usize,
}

impl From<ReinstallBlockingReasonSummary> for ReinstallBlockingReasonSummaryDto {
    fn from(summary: ReinstallBlockingReasonSummary) -> Self {
        Self {
            code: summary.reason.into(),
            count: summary.count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReinstallAttachmentCountsDto {
    pub retained: u32,
    pub excluded: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ReinstallPlanPreviewDto {
    Ready {
        #[serde(skip_serializing_if = "Vec::is_empty")]
        file_effects: Vec<RetargetFilePreviewDto>,
        prerequisite_decision: GamePrerequisiteDecisionDto,
        plan_token: String,
        installed_revision: ModRevisionSummaryDto,
        candidate_revision: ModRevisionSummaryDto,
        counts: ReinstallTargetCountsDto,
        #[serde(skip_serializing_if = "Option::is_none")]
        attachment_counts: Option<ReinstallAttachmentCountsDto>,
        blocking_reasons: Vec<ReinstallBlockingReasonSummaryDto>,
    },
    Blocked {
        #[serde(skip_serializing_if = "Vec::is_empty")]
        file_effects: Vec<RetargetFilePreviewDto>,
        prerequisite_decision: GamePrerequisiteDecisionDto,
        plan_token: (),
        installed_revision: Option<ModRevisionSummaryDto>,
        candidate_revision: Option<ModRevisionSummaryDto>,
        counts: ReinstallTargetCountsDto,
        #[serde(skip_serializing_if = "Option::is_none")]
        attachment_counts: Option<ReinstallAttachmentCountsDto>,
        blocking_reasons: Vec<ReinstallBlockingReasonSummaryDto>,
    },
    NoChanges {
        prerequisite_decision: GamePrerequisiteDecisionDto,
        plan_token: (),
        installed_revision: ModRevisionSummaryDto,
        candidate_revision: ModRevisionSummaryDto,
        counts: ReinstallTargetCountsDto,
        #[serde(skip_serializing_if = "Option::is_none")]
        attachment_counts: Option<ReinstallAttachmentCountsDto>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        file_effects: Vec<RetargetFilePreviewDto>,
        blocking_reasons: Vec<ReinstallBlockingReasonSummaryDto>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReinstallDtoInvariantError {
    InvalidNoChanges,
    ReadyFieldsMissing,
    ReadyHasBlockingReasons,
    BlockedHasPlanToken,
    CandidateNotFoundHasCandidate,
}

impl TryFrom<ReinstallPlanPreview> for ReinstallPlanPreviewDto {
    type Error = ReinstallDtoInvariantError;

    fn try_from(preview: ReinstallPlanPreview) -> Result<Self, Self::Error> {
        let file_effects = preview.file_effects.into_iter().map(Into::into).collect();
        let prerequisite_decision = preview.prerequisite_decision.clone().into();
        let attachment_counts =
            (!preview.attachment_counts.is_empty()).then_some(ReinstallAttachmentCountsDto {
                retained: preview.attachment_counts.retained,
                excluded: preview.attachment_counts.excluded,
            });
        match preview.status {
            ReinstallPreviewStatus::Ready => {
                if !preview.blocking_reasons.is_empty() {
                    return Err(ReinstallDtoInvariantError::ReadyHasBlockingReasons);
                }
                let (Some(plan_token), Some(installed_revision), Some(candidate_revision)) = (
                    preview.plan_token,
                    preview.installed_revision,
                    preview.candidate_revision,
                ) else {
                    return Err(ReinstallDtoInvariantError::ReadyFieldsMissing);
                };
                if plan_token.trim().is_empty() {
                    return Err(ReinstallDtoInvariantError::ReadyFieldsMissing);
                }
                Ok(Self::Ready {
                    file_effects,
                    prerequisite_decision,
                    plan_token,
                    installed_revision: installed_revision.into(),
                    candidate_revision: candidate_revision.into(),
                    counts: preview.counts.into(),
                    attachment_counts,
                    blocking_reasons: Vec::new(),
                })
            }
            ReinstallPreviewStatus::Blocked => {
                if preview.plan_token.is_some() {
                    return Err(ReinstallDtoInvariantError::BlockedHasPlanToken);
                }
                if preview
                    .blocking_reasons
                    .iter()
                    .any(|summary| summary.reason == ReinstallBlockingReason::CandidateNotFound)
                    && preview.candidate_revision.is_some()
                {
                    return Err(ReinstallDtoInvariantError::CandidateNotFoundHasCandidate);
                }
                Ok(Self::Blocked {
                    file_effects,
                    prerequisite_decision,
                    plan_token: (),
                    installed_revision: preview.installed_revision.map(Into::into),
                    candidate_revision: preview.candidate_revision.map(Into::into),
                    counts: preview.counts.into(),
                    attachment_counts,
                    blocking_reasons: preview
                        .blocking_reasons
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                })
            }
            ReinstallPreviewStatus::NoChanges => {
                if preview.plan_token.is_some()
                    || !preview.blocking_reasons.is_empty()
                    || preview.prerequisite_decision.is_blocked()
                    || preview.counts.replaced != 0
                    || preview.counts.added != 0
                    || preview.counts.stale != 0
                {
                    return Err(ReinstallDtoInvariantError::InvalidNoChanges);
                }
                let (Some(installed), Some(candidate)) =
                    (preview.installed_revision, preview.candidate_revision)
                else {
                    return Err(ReinstallDtoInvariantError::InvalidNoChanges);
                };
                if installed.revision_id != candidate.revision_id {
                    return Err(ReinstallDtoInvariantError::InvalidNoChanges);
                }
                Ok(Self::NoChanges {
                    prerequisite_decision,
                    plan_token: (),
                    installed_revision: installed.into(),
                    candidate_revision: candidate.into(),
                    counts: preview.counts.into(),
                    attachment_counts,
                    file_effects,
                    blocking_reasons: Vec::new(),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallManifestStatusDto {
    NotInstalled,
    Installed,
    CommittedCleanupPending,
    CleanupPending,
    RollbackRequired,
    RepairRequired,
    Unknown,
}

impl From<InstallManifestStatus> for InstallManifestStatusDto {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallRecoveryStatusDto {
    NotInstalled,
    Completed,
    CommittedCleanupPending,
    CleanupPending,
    RollbackRequired,
    RepairRequired,
    Unknown,
}

impl From<InstallRecoveryStatus> for InstallRecoveryStatusDto {
    fn from(status: InstallRecoveryStatus) -> Self {
        match status {
            InstallRecoveryStatus::NotInstalled => Self::NotInstalled,
            InstallRecoveryStatus::Completed => Self::Completed,
            InstallRecoveryStatus::CommittedCleanupPending => Self::CommittedCleanupPending,
            InstallRecoveryStatus::CleanupPending => Self::CleanupPending,
            InstallRecoveryStatus::RollbackRequired => Self::RollbackRequired,
            InstallRecoveryStatus::RepairRequired => Self::RepairRequired,
            InstallRecoveryStatus::Unknown => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallRecoveryActionKindDto {
    RollbackInstall,
    ReconcileReinstall,
    UninstallMissingTargets,
}

impl From<InstallRecoveryActionKind> for InstallRecoveryActionKindDto {
    fn from(action_kind: InstallRecoveryActionKind) -> Self {
        match action_kind {
            InstallRecoveryActionKind::RollbackInstall => Self::RollbackInstall,
            InstallRecoveryActionKind::ReconcileReinstall => Self::ReconcileReinstall,
            InstallRecoveryActionKind::UninstallMissingTargets => Self::UninstallMissingTargets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_app::{
        GamePrerequisiteDecision, GamePrerequisiteDecisionCode, GamePrerequisiteDecisionStatus,
        ReinstallBlockingReason, ReinstallBlockingReasonSummary, ReinstallPlanPreview,
        ReinstallPreviewStatus, ReinstallRevisionSummary, ReinstallTargetCounts,
    };
    use hmm_core::{GameId, ModRevisionId};
    use serde_json::{json, Value};

    #[test]
    fn preview_request_deserializes_nested_camel_case_layer_without_paths() {
        let value = json!({
            "gameId": "mhw",
            "profileId": "default",
            "modId": "mod-a",
            "candidateRevisionId": "revision-v2",
            "layer": { "name": "base", "priority": 10 }
        });

        let request: PreviewReinstallPlanRequestDto =
            serde_json::from_value(value).expect("deserialize preview request");

        assert_eq!(request.game_id, "mhw");
        assert_eq!(request.profile_id, "default");
        assert_eq!(request.mod_id, "mod-a");
        assert_eq!(request.candidate_revision_id, "revision-v2");
        assert_eq!(request.layer.name, "base");
        assert_eq!(request.layer.priority, 10);
    }

    #[test]
    fn revision_import_request_accepts_only_picker_archive_and_existing_mod_id() {
        let value = json!({
            "archivePath": "C:\\mods\\candidate-v2.zip",
            "modId": "mod-a"
        });
        let request: StartImportModRevisionTaskRequestDto =
            serde_json::from_value(value.clone()).expect("deserialize revision import request");

        assert_eq!(request.archive_path, "C:\\mods\\candidate-v2.zip");
        assert_eq!(request.mod_id, "mod-a");

        for forbidden in [
            "targetPath",
            "gameRoot",
            "sourcePath",
            "sandboxPath",
            "displayName",
            "author",
            "version",
        ] {
            let mut rejected = value.clone();
            rejected[forbidden] = json!("untrusted-value");
            assert!(
                serde_json::from_value::<StartImportModRevisionTaskRequestDto>(rejected).is_err(),
                "revision import unexpectedly accepted {forbidden}"
            );
        }
    }

    #[test]
    fn reinstall_requests_reject_forbidden_filesystem_fields() {
        for forbidden in [
            "targetPath",
            "deletePath",
            "backupRef",
            "manifestPath",
            "gameRoot",
            "sourcePath",
            "sandboxPath",
            "contentHash",
        ] {
            let value = json!({
                "gameId": "mhw",
                "profileId": "default",
                "modId": "mod-a",
                "candidateRevisionId": "revision-v2",
                "layer": { "name": "base", "priority": 0 }
            });
            let mut preview = value.clone();
            preview[forbidden] = json!("sensitive-value");

            assert!(
                serde_json::from_value::<PreviewReinstallPlanRequestDto>(preview).is_err(),
                "preview request unexpectedly accepted {forbidden}"
            );

            let mut start = value;
            start["planToken"] = json!(
                "reinstall-preview-v1:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            );
            start[forbidden] = json!("sensitive-value");
            assert!(
                serde_json::from_value::<StartReinstallTaskRequestDto>(start).is_err(),
                "start request unexpectedly accepted {forbidden}"
            );
        }
    }

    #[test]
    fn ready_preview_serializes_as_strict_discriminated_union() {
        let dto = ReinstallPlanPreviewDto::try_from(ReinstallPlanPreview {
            file_effects: Vec::new(),
            attachment_counts: hmm_app::ReinstallAttachmentCounts { retained: 1, excluded: 2 },
            status: ReinstallPreviewStatus::Ready,
            prerequisite_decision: warning_prerequisite_decision(),
            installed_revision: Some(revision("revision-v1")),
            candidate_revision: Some(revision("revision-v2")),
            counts: ReinstallTargetCounts {
                retained: 1,
                replaced: 2,
                added: 3,
                stale: 4,
            },
            blocking_reasons: Vec::new(),
            plan_token: Some("reinstall-preview-v1:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned()),
        })
        .expect("ready preview invariant");
        let value: Value = serde_json::to_value(dto).expect("serialize ready preview");

        assert_eq!(value["status"], "ready");
        assert_eq!(
            value["attachmentCounts"],
            json!({"retained": 1, "excluded": 2})
        );
        assert_eq!(value["installedRevision"]["revisionId"], "revision-v1");
        assert_eq!(value["candidateRevision"]["revisionId"], "revision-v2");
        assert_eq!(
            value["counts"],
            json!({
                "retained": 1,
                "replaced": 2,
                "added": 3,
                "stale": 4
            })
        );
        assert!(value["planToken"].as_str().is_some());
        assert_eq!(value["blockingReasons"], json!([]));
        assert_eq!(
            value["prerequisiteDecision"],
            json!({
                "status": "warning",
                "rulesVersion": 3,
                "codes": ["signature_unverified"]
            })
        );
        assert_public_preview_is_sanitized(&value);
    }

    #[test]
    fn no_changes_has_no_write_token_and_cannot_hide_file_changes_or_blockers() {
        let preview = ReinstallPlanPreview {
            status: ReinstallPreviewStatus::NoChanges,
            file_effects: Vec::new(),
            attachment_counts: Default::default(),
            prerequisite_decision: ready_prerequisite_decision(),
            installed_revision: Some(revision("revision-v1")),
            candidate_revision: Some(revision("revision-v1")),
            counts: ReinstallTargetCounts {
                retained: 1,
                ..Default::default()
            },
            blocking_reasons: Vec::new(),
            plan_token: None,
        };
        let value =
            serde_json::to_value(ReinstallPlanPreviewDto::try_from(preview.clone()).unwrap())
                .unwrap();
        assert_eq!(value["status"], "no_changes");
        assert!(value["planToken"].is_null());
        assert_eq!(value["blockingReasons"], json!([]));
        for case in ["token", "revision", "changes", "blocker", "prerequisite"] {
            let mut changed = preview.clone();
            match case {
                "token" => changed.plan_token = Some("unexpected-token".into()),
                "revision" => changed.candidate_revision = Some(revision("revision-v2")),
                "changes" => changed.counts.added = 1,
                "blocker" => changed
                    .blocking_reasons
                    .push(ReinstallBlockingReasonSummary {
                        reason: ReinstallBlockingReason::TargetChanged,
                        count: 1,
                    }),
                "prerequisite" => {
                    changed.prerequisite_decision.status = GamePrerequisiteDecisionStatus::Blocked
                }
                _ => unreachable!(),
            }
            assert!(
                ReinstallPlanPreviewDto::try_from(changed).is_err(),
                "accepted {case}"
            );
        }
    }

    #[test]
    fn candidate_not_found_serializes_null_candidate_and_token() {
        let dto = ReinstallPlanPreviewDto::try_from(ReinstallPlanPreview {
            file_effects: Vec::new(),
            attachment_counts: hmm_app::ReinstallAttachmentCounts::default(),
            status: ReinstallPreviewStatus::Blocked,
            prerequisite_decision: ready_prerequisite_decision(),
            installed_revision: None,
            candidate_revision: None,
            counts: ReinstallTargetCounts::default(),
            blocking_reasons: vec![ReinstallBlockingReasonSummary {
                reason: ReinstallBlockingReason::CandidateNotFound,
                count: 1,
            }],
            plan_token: None,
        })
        .expect("blocked preview invariant");
        let value: Value = serde_json::to_value(dto).expect("serialize blocked preview");

        assert_eq!(value["status"], "blocked");
        assert!(value.get("attachmentCounts").is_none());
        assert!(value["planToken"].is_null());
        assert!(value["installedRevision"].is_null());
        assert!(value["candidateRevision"].is_null());
        assert_eq!(
            value["blockingReasons"],
            json!([{
                "code": "candidate_not_found",
                "count": 1
            }])
        );
        assert_public_preview_is_sanitized(&value);
    }

    #[test]
    fn incomplete_ready_preview_is_rejected_before_serialization() {
        let result = ReinstallPlanPreviewDto::try_from(ReinstallPlanPreview {
            file_effects: Vec::new(),
            attachment_counts: hmm_app::ReinstallAttachmentCounts::default(),
            status: ReinstallPreviewStatus::Ready,
            prerequisite_decision: ready_prerequisite_decision(),
            installed_revision: Some(revision("revision-v1")),
            candidate_revision: None,
            counts: ReinstallTargetCounts::default(),
            blocking_reasons: Vec::new(),
            plan_token: None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn reconcile_reinstall_action_kind_uses_stable_snake_case_code() {
        let dto = InstallRecoveryActionKindDto::from(
            hmm_app::InstallRecoveryActionKind::ReconcileReinstall,
        );

        assert_eq!(
            serde_json::to_value(dto).expect("serialize reconciliation action kind"),
            json!("reconcile_reinstall")
        );
    }

    fn revision(revision_id: &str) -> ReinstallRevisionSummary {
        ReinstallRevisionSummary {
            revision_id: ModRevisionId::new(revision_id),
        }
    }

    fn ready_prerequisite_decision() -> GamePrerequisiteDecision {
        GamePrerequisiteDecision {
            game_id: GameId::mhw(),
            status: GamePrerequisiteDecisionStatus::Ready,
            rules_version: Some(3),
            codes: Vec::new(),
        }
    }

    fn warning_prerequisite_decision() -> GamePrerequisiteDecision {
        GamePrerequisiteDecision {
            game_id: GameId::mhw(),
            status: GamePrerequisiteDecisionStatus::Warning,
            rules_version: Some(3),
            codes: vec![GamePrerequisiteDecisionCode::SignatureUnverified],
        }
    }

    fn assert_public_preview_is_sanitized(value: &Value) {
        let serialized = serde_json::to_string(value).expect("serialize preview text");
        for forbidden in [
            "targetPath",
            "deletePath",
            "backupRef",
            "manifest",
            "gameRoot",
            "sourcePath",
            "sandboxPath",
            "sha256",
            "contentHash",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
    }
}
