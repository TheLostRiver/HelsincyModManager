use crate::save_backup_dto::SaveBackupSummaryDto;
use hmm_app::SaveRestorePreview;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewSaveRestoreRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub backup_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRestorePreviewDto {
    pub backup: SaveBackupSummaryDto,
    pub file_count: u32,
    pub total_uncompressed_bytes: u64,
    pub pre_restore_backup_enabled: bool,
    pub requires_additional_confirmation: bool,
    pub warning_codes: Vec<String>,
    pub preview_token: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSaveRestoreTaskRequestDto {
    pub game_id: String,
    pub profile_id: String,
    pub backup_id: String,
    pub preview_token: String,
    pub confirmed: bool,
    #[serde(default)]
    pub confirmed_without_pre_restore: bool,
}

impl TryFrom<SaveRestorePreview> for SaveRestorePreviewDto {
    type Error = ();

    fn try_from(preview: SaveRestorePreview) -> Result<Self, Self::Error> {
        Ok(Self {
            backup: preview.backup.into(),
            file_count: preview.file_count,
            total_uncompressed_bytes: preview.total_uncompressed_bytes,
            pre_restore_backup_enabled: preview.pre_restore_backup_enabled,
            requires_additional_confirmation: preview.requires_additional_confirmation,
            warning_codes: preview.warning_codes,
            preview_token: preview.preview_token,
            expires_at: u64::try_from(preview.expires_at_unix_millis).map_err(|_| ())?,
        })
    }
}
