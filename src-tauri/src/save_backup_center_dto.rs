use crate::dto::{ProfileBackupRetentionDto, SteamAccountDisplaySummaryDto};
use crate::save_backup_dto::{SaveBackupStatusDto, SaveBackupSummaryDto, SaveBackupTriggerDto};
use hmm_app::{
    SaveBackupCenterItem, SaveBackupCenterPage, SaveBackupCenterProfileSummary,
    SaveBackupCenterSummary,
};
use hmm_core::SaveBackupRetentionReport;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuerySaveBackupCenterRequestDto {
    pub game_id: String,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub trigger: Option<SaveBackupTriggerDto>,
    #[serde(default)]
    pub status: Option<SaveBackupStatusDto>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

const fn default_limit() -> usize {
    hmm_app::DEFAULT_SAVE_BACKUP_CENTER_LIMIT
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSaveBackupNoteRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub backup_id: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSaveBackupRetentionRequestDto {
    pub game_id: String,
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSaveBackupNoteResultDto {
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupCenterPageDto {
    pub offset: usize,
    pub limit: usize,
    pub total_count: usize,
    pub summary: SaveBackupCenterSummaryDto,
    pub profiles: Vec<SaveBackupCenterProfileSummaryDto>,
    pub items: Vec<SaveBackupCenterItemDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupCenterSummaryDto {
    pub backup_count: u32,
    pub archive_bytes: u64,
    pub protected_count: u32,
    pub attention_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupCenterProfileSummaryDto {
    pub profile_id: String,
    pub profile_name: String,
    pub is_active: bool,
    pub steam_account: Option<SteamAccountDisplaySummaryDto>,
    pub retention: ProfileBackupRetentionDto,
    pub backup_count: u32,
    pub archive_bytes: u64,
    pub protected_count: u32,
    pub attention_count: u32,
    pub budget_satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupCenterItemDto {
    pub profile_name: String,
    pub backup: SaveBackupSummaryDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupRetentionReportDto {
    pub outcome: String,
    pub evidence_degraded: bool,
    pub scanned_count: u32,
    pub protected_count: u32,
    pub problem_count: u32,
    pub candidate_count: u32,
    pub deleted_count: u32,
    pub partial_count: u32,
    pub blocked_count: u32,
    pub archive_bytes_before: u64,
    pub archive_bytes_after: u64,
    pub released_bytes: u64,
    pub max_total_bytes: Option<u64>,
    pub budget_satisfied: bool,
}

impl From<SaveBackupCenterPage> for SaveBackupCenterPageDto {
    fn from(page: SaveBackupCenterPage) -> Self {
        Self {
            offset: page.offset,
            limit: page.limit,
            total_count: page.total_count,
            summary: page.summary.into(),
            profiles: page.profiles.into_iter().map(Into::into).collect(),
            items: page.items.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<SaveBackupCenterSummary> for SaveBackupCenterSummaryDto {
    fn from(summary: SaveBackupCenterSummary) -> Self {
        Self {
            backup_count: summary.backup_count,
            archive_bytes: summary.archive_bytes,
            protected_count: summary.protected_count,
            attention_count: summary.attention_count,
        }
    }
}

impl From<SaveBackupCenterProfileSummary> for SaveBackupCenterProfileSummaryDto {
    fn from(profile: SaveBackupCenterProfileSummary) -> Self {
        Self {
            profile_id: profile.profile_id.as_str().to_owned(),
            profile_name: profile.profile_name,
            is_active: profile.is_active,
            steam_account: profile.steam_account.map(Into::into),
            retention: profile.retention.into(),
            backup_count: profile.backup_count,
            archive_bytes: profile.archive_bytes,
            protected_count: profile.protected_count,
            attention_count: profile.attention_count,
            budget_satisfied: profile.budget_satisfied,
        }
    }
}

impl From<SaveBackupCenterItem> for SaveBackupCenterItemDto {
    fn from(item: SaveBackupCenterItem) -> Self {
        Self {
            profile_name: item.profile_name,
            backup: item.backup.into(),
        }
    }
}

impl From<SaveBackupRetentionReport> for SaveBackupRetentionReportDto {
    fn from(report: SaveBackupRetentionReport) -> Self {
        Self {
            outcome: report.outcome.as_str().to_owned(),
            evidence_degraded: report.evidence_degraded,
            scanned_count: report.scanned_count,
            protected_count: report.protected_count,
            problem_count: report.problem_count,
            candidate_count: report.candidate_count,
            deleted_count: report.deleted_count,
            partial_count: report.partial_count,
            blocked_count: report.blocked_count,
            archive_bytes_before: report.archive_bytes_before,
            archive_bytes_after: report.archive_bytes_after,
            released_bytes: report.released_bytes,
            max_total_bytes: report.max_total_bytes,
            budget_satisfied: report.budget_satisfied,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_app::{
        SaveBackupCenterItem, SaveBackupCenterPage, SaveBackupCenterProfileSummary,
        SaveBackupCenterSummary,
    };
    use hmm_core::{
        GameId, ProfileBackupRetention, ProfileDirectoryMode, ProfileDirectorySelection,
        ProfileDirectoryStatus, ProfileId, SaveBackupRetentionOutcome, SaveBackupRetentionReport,
        SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger, SteamAccountDisplaySummary,
    };

    #[test]
    fn backup_center_dto_is_camel_case_and_omits_private_backup_facts() {
        let backup = SaveBackupSummary {
            backup_id: "mhw:default:20260815-120000:manual".to_owned(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            trigger: SaveBackupTrigger::Manual,
            status: SaveBackupStatus::RetentionPartial,
            archive_file_name: "fixture.zip".to_owned(),
            manifest_file_name: "fixture.manifest.json".to_owned(),
            archive_size_bytes: 128,
            retention_released_bytes: 64,
            archive_sha256: "sha256:private".to_owned(),
            file_count: 1,
            created_at: 42,
            source_path_label: Some("remote".to_owned()),
            source_path_hash: "sha256:source".to_owned(),
            backup_directory: ProfileDirectorySelection {
                mode: ProfileDirectoryMode::Custom,
                status: ProfileDirectoryStatus::Valid,
                directory: Some("D:/PrivateBackups".to_owned()),
                path_label: Some("PrivateBackups".to_owned()),
                messages: Vec::new(),
            },
            notes: Some("display note".to_owned()),
        };
        let dto: SaveBackupCenterPageDto = SaveBackupCenterPage {
            offset: 0,
            limit: 30,
            total_count: 1,
            summary: SaveBackupCenterSummary {
                backup_count: 1,
                archive_bytes: 64,
                protected_count: 0,
                attention_count: 1,
            },
            profiles: vec![SaveBackupCenterProfileSummary {
                profile_id: ProfileId::new("default"),
                profile_name: "Default".to_owned(),
                is_active: true,
                steam_account: Some(SteamAccountDisplaySummary {
                    account_name: Some("Hunter".to_owned()),
                    avatar_url: None,
                    account_label: "Steam 12****34".to_owned(),
                }),
                retention: ProfileBackupRetention {
                    max_count: 20,
                    max_age_days: Some(30),
                    max_total_bytes: Some(1_024),
                },
                backup_count: 1,
                archive_bytes: 64,
                protected_count: 0,
                attention_count: 1,
                budget_satisfied: true,
            }],
            items: vec![SaveBackupCenterItem {
                profile_name: "Default".to_owned(),
                backup,
            }],
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize center dto");
        assert_eq!(value["totalCount"], 1);
        assert_eq!(value["summary"]["archiveBytes"], 64);
        assert_eq!(value["profiles"][0]["retention"]["maxTotalBytes"], 1_024);
        assert_eq!(value["items"][0]["backup"]["status"], "retention_partial");
        assert!(value["items"][0]["backup"]
            .get("retentionReleasedBytes")
            .is_none());
        let serialized = value.to_string();
        assert!(!serialized.contains("D:/PrivateBackups"));
        assert!(!serialized.contains("fixture.manifest.json"));
        assert!(!serialized.contains("sha256:private"));
        assert!(!serialized.contains("sha256:source"));
    }

    #[test]
    fn backup_center_query_defaults_pagination_limit() {
        let request: QuerySaveBackupCenterRequestDto = serde_json::from_value(serde_json::json!({
            "gameId": "mhw",
            "trigger": "pre_restore",
            "status": "retention_partial"
        }))
        .expect("deserialize query");
        assert_eq!(request.offset, 0);
        assert_eq!(request.limit, hmm_app::DEFAULT_SAVE_BACKUP_CENTER_LIMIT);
        assert_eq!(request.trigger, Some(SaveBackupTriggerDto::PreRestore));
        assert_eq!(request.status, Some(SaveBackupStatusDto::RetentionPartial));
    }

    #[test]
    fn retention_report_dto_exposes_evidence_degradation() {
        let dto: SaveBackupRetentionReportDto = SaveBackupRetentionReport {
            outcome: SaveBackupRetentionOutcome::Completed,
            evidence_degraded: true,
            scanned_count: 3,
            protected_count: 1,
            problem_count: 0,
            candidate_count: 1,
            deleted_count: 1,
            partial_count: 0,
            blocked_count: 0,
            archive_bytes_before: 128,
            archive_bytes_after: 64,
            released_bytes: 64,
            max_total_bytes: Some(64),
            budget_satisfied: true,
        }
        .into();

        let value = serde_json::to_value(dto).expect("serialize retention report");
        assert_eq!(value["outcome"], "completed");
        assert_eq!(value["evidenceDegraded"], true);
        assert_eq!(value["releasedBytes"], 64);
        let fields = value
            .as_object()
            .expect("retention report is an object")
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            fields,
            [
                "archiveBytesAfter",
                "archiveBytesBefore",
                "blockedCount",
                "budgetSatisfied",
                "candidateCount",
                "deletedCount",
                "evidenceDegraded",
                "maxTotalBytes",
                "outcome",
                "partialCount",
                "problemCount",
                "protectedCount",
                "releasedBytes",
                "scannedCount",
            ]
            .into_iter()
            .collect()
        );
        let serialized = value.to_string();
        for forbidden in [
            "backupDirectory",
            "archivePath",
            "manifestPath",
            "steamId",
            "errorMessage",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }
}
