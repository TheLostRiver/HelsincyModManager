use hmm_core::PreviewImageRejectionReason;
use hmm_ports::{StoredImportPreviewImage, StoredModImportAnalysis};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewImageDiagnosticsSummary {
    pub total_imported_mods: usize,
    pub thumbnail_count: usize,
    pub fallback_count: usize,
    pub fallback_reasons: Vec<PreviewImageFallbackDiagnostic>,
    pub export_categories: Vec<PreviewImageDiagnosticExportCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewImageFallbackDiagnostic {
    pub reason: PreviewImageRejectionReason,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewImageDiagnosticExportCategory {
    pub category: PreviewImageDiagnosticExportCategoryId,
    pub status: PreviewImageDiagnosticExportCategoryStatus,
    pub reason: Option<PreviewImageDiagnosticExportExclusionReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewImageDiagnosticExportCategoryId {
    PreviewImageSummary,
    ThumbnailFiles,
    ThumbnailUrls,
    RawPackageContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewImageDiagnosticExportCategoryStatus {
    Included,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewImageDiagnosticExportExclusionReason {
    DerivedImageContent,
    OpaqueResourceReference,
    ThirdPartyModContent,
}

pub(crate) fn preview_image_diagnostics_from_stored(
    records: &[StoredModImportAnalysis],
) -> PreviewImageDiagnosticsSummary {
    let mut summary = PreviewImageDiagnosticsSummary {
        total_imported_mods: records.len(),
        thumbnail_count: 0,
        fallback_count: 0,
        fallback_reasons: Vec::new(),
        export_categories: preview_image_diagnostic_export_categories(),
    };

    for record in records {
        match &record.preview_image {
            StoredImportPreviewImage::Thumbnail { .. } => {
                summary.thumbnail_count += 1;
            }
            StoredImportPreviewImage::Fallback { reason } => {
                summary.fallback_count += 1;
                increment_fallback_reason(&mut summary.fallback_reasons, *reason);
            }
        }
    }

    summary
}

fn preview_image_diagnostic_export_categories() -> Vec<PreviewImageDiagnosticExportCategory> {
    vec![
        PreviewImageDiagnosticExportCategory {
            category: PreviewImageDiagnosticExportCategoryId::PreviewImageSummary,
            status: PreviewImageDiagnosticExportCategoryStatus::Included,
            reason: None,
        },
        PreviewImageDiagnosticExportCategory {
            category: PreviewImageDiagnosticExportCategoryId::ThumbnailFiles,
            status: PreviewImageDiagnosticExportCategoryStatus::Excluded,
            reason: Some(PreviewImageDiagnosticExportExclusionReason::DerivedImageContent),
        },
        PreviewImageDiagnosticExportCategory {
            category: PreviewImageDiagnosticExportCategoryId::ThumbnailUrls,
            status: PreviewImageDiagnosticExportCategoryStatus::Excluded,
            reason: Some(PreviewImageDiagnosticExportExclusionReason::OpaqueResourceReference),
        },
        PreviewImageDiagnosticExportCategory {
            category: PreviewImageDiagnosticExportCategoryId::RawPackageContent,
            status: PreviewImageDiagnosticExportCategoryStatus::Excluded,
            reason: Some(PreviewImageDiagnosticExportExclusionReason::ThirdPartyModContent),
        },
    ]
}

fn increment_fallback_reason(
    fallback_reasons: &mut Vec<PreviewImageFallbackDiagnostic>,
    reason: PreviewImageRejectionReason,
) {
    if let Some(entry) = fallback_reasons
        .iter_mut()
        .find(|entry| entry.reason == reason)
    {
        entry.count += 1;
        return;
    }

    fallback_reasons.push(PreviewImageFallbackDiagnostic { reason, count: 1 });
}
