use hmm_core::PreviewImageRejectionReason;
use hmm_ports::{
    AppClock, DiagnosticPackageEntry, DiagnosticPackageExportRequest, DiagnosticPackageExporter,
    ModImportResultRepository, StoredImportPreviewImage, StoredModImportAnalysis,
};
use serde_json::json;
use std::sync::Arc;

const PREVIEW_IMAGE_DIAGNOSTICS_ENTRY_NAME: &str = "preview-image-diagnostics.json";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewImageDiagnosticsExport {
    pub export_id: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub diagnostics: PreviewImageDiagnosticsSummary,
}

pub struct PreviewImageDiagnosticsExportService {
    result_repository: Arc<dyn ModImportResultRepository>,
    diagnostic_exporter: Arc<dyn DiagnosticPackageExporter>,
    clock: Arc<dyn AppClock>,
}

impl PreviewImageDiagnosticsExportService {
    pub fn new(
        result_repository: Arc<dyn ModImportResultRepository>,
        diagnostic_exporter: Arc<dyn DiagnosticPackageExporter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            result_repository,
            diagnostic_exporter,
            clock,
        }
    }

    pub fn export_preview_image_diagnostics(
        &self,
    ) -> anyhow::Result<PreviewImageDiagnosticsExport> {
        let records = self.result_repository.list_analysis()?;
        let diagnostics = preview_image_diagnostics_from_stored(&records);
        let payload =
            serde_json::to_vec(&sanitized_preview_image_diagnostics_payload(&diagnostics))?;
        let file_name = format!(
            "preview-image-diagnostics-{}.zip",
            self.clock.now_unix_millis()?
        );
        let export = self
            .diagnostic_exporter
            .export_package(DiagnosticPackageExportRequest {
                file_name: &file_name,
                entries: &[DiagnosticPackageEntry {
                    name: PREVIEW_IMAGE_DIAGNOSTICS_ENTRY_NAME,
                    bytes: &payload,
                }],
            })?;

        Ok(PreviewImageDiagnosticsExport {
            export_id: export.export_id,
            file_name: export.file_name,
            size_bytes: export.size_bytes,
            diagnostics,
        })
    }
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

fn sanitized_preview_image_diagnostics_payload(
    diagnostics: &PreviewImageDiagnosticsSummary,
) -> serde_json::Value {
    json!({
        "totalImportedMods": diagnostics.total_imported_mods,
        "thumbnailCount": diagnostics.thumbnail_count,
        "fallbackCount": diagnostics.fallback_count,
        "fallbackReasons": diagnostics.fallback_reasons.iter().map(|reason| {
            json!({
                "reason": preview_image_rejection_reason_key(reason.reason),
                "count": reason.count,
            })
        }).collect::<Vec<_>>(),
        "exportCategories": diagnostics.export_categories.iter().map(|category| {
            let mut value = json!({
                "category": diagnostic_export_category_key(category.category),
                "status": diagnostic_export_category_status_key(category.status),
            });
            if let Some(reason) = category.reason {
                value["reason"] = json!(diagnostic_export_exclusion_reason_key(reason));
            }
            value
        }).collect::<Vec<_>>(),
    })
}

fn preview_image_rejection_reason_key(reason: PreviewImageRejectionReason) -> &'static str {
    match reason {
        PreviewImageRejectionReason::Missing => "missing",
        PreviewImageRejectionReason::TooLarge => "too_large",
        PreviewImageRejectionReason::TooManyCandidates => "too_many_candidates",
        PreviewImageRejectionReason::UnsupportedFormat => "unsupported_format",
        PreviewImageRejectionReason::DecodeFailed => "decode_failed",
        PreviewImageRejectionReason::PixelLimitExceeded => "pixel_limit_exceeded",
        PreviewImageRejectionReason::CacheWriteFailed => "cache_write_failed",
    }
}

fn diagnostic_export_category_key(
    category: PreviewImageDiagnosticExportCategoryId,
) -> &'static str {
    match category {
        PreviewImageDiagnosticExportCategoryId::PreviewImageSummary => "preview_image_summary",
        PreviewImageDiagnosticExportCategoryId::ThumbnailFiles => "thumbnail_files",
        PreviewImageDiagnosticExportCategoryId::ThumbnailUrls => "thumbnail_urls",
        PreviewImageDiagnosticExportCategoryId::RawPackageContent => "raw_package_content",
    }
}

fn diagnostic_export_category_status_key(
    status: PreviewImageDiagnosticExportCategoryStatus,
) -> &'static str {
    match status {
        PreviewImageDiagnosticExportCategoryStatus::Included => "included",
        PreviewImageDiagnosticExportCategoryStatus::Excluded => "excluded",
    }
}

fn diagnostic_export_exclusion_reason_key(
    reason: PreviewImageDiagnosticExportExclusionReason,
) -> &'static str {
    match reason {
        PreviewImageDiagnosticExportExclusionReason::DerivedImageContent => "derived_image_content",
        PreviewImageDiagnosticExportExclusionReason::OpaqueResourceReference => {
            "opaque_resource_reference"
        }
        PreviewImageDiagnosticExportExclusionReason::ThirdPartyModContent => {
            "third_party_mod_content"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use hmm_ports::{
        AppClock, DiagnosticPackageExportRequest, DiagnosticPackageExportResult,
        DiagnosticPackageExporter, ModImportResultRepository, StoredImportPreviewImage,
        StoredModPackageMetadata,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn export_service_writes_sanitized_preview_image_diagnostics_package() {
        let repository = Arc::new(FakeModImportResultRepository::default());
        repository
            .save_analysis(&StoredModImportAnalysis {
                mod_id: "mod-1".to_owned(),
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                display_name: "Preview Mod".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Thumbnail {
                    thumbnail_url: "thumbnail://pkg-1/preview-768/secret-hash".to_owned(),
                    width: 320,
                    height: 180,
                    content_hash: "secret-hash".to_owned(),
                    variant: "preview-768".to_owned(),
                },
            })
            .expect("save thumbnail analysis");
        repository
            .save_analysis(&StoredModImportAnalysis {
                mod_id: "mod-2".to_owned(),
                task_id: "task-2".to_owned(),
                package_id: "pkg-2".to_owned(),
                display_name: "Fallback Mod".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::DecodeFailed,
                },
            })
            .expect("save fallback analysis");
        let exporter = Arc::new(RecordingDiagnosticPackageExporter::default());
        let exporter_port: Arc<dyn DiagnosticPackageExporter> = exporter.clone();
        let service = PreviewImageDiagnosticsExportService::new(
            repository,
            exporter_port,
            Arc::new(FixedClock { unix_millis: 42 }),
        );

        let export = service
            .export_preview_image_diagnostics()
            .expect("export succeeds");

        assert_eq!(export.file_name, "preview-image-diagnostics-42.zip");
        assert_eq!(export.export_id, "preview-image-diagnostics-42.zip");
        assert_eq!(export.size_bytes, 4096);
        let request = exporter.take_request();
        assert_eq!(request.file_name, "preview-image-diagnostics-42.zip");
        assert_eq!(request.entries.len(), 1);
        assert_eq!(request.entries[0].name, "preview-image-diagnostics.json");
        let payload = String::from_utf8(request.entries[0].bytes.clone()).expect("utf8 payload");
        assert!(payload.contains("\"totalImportedMods\":2"));
        assert!(payload.contains("\"thumbnailCount\":1"));
        assert!(payload.contains("\"fallbackCount\":1"));
        assert!(payload.contains("\"exportCategories\""));
        assert!(!payload.contains("thumbnail://"));
        assert!(!payload.contains("secret-hash"));
        assert!(!payload.contains("contentHash"));
        assert!(!payload.contains("thumbnailUrl"));
        assert!(!payload.contains("sandbox"));
        assert!(!payload.contains("C:/"));
    }

    #[derive(Default)]
    struct FakeModImportResultRepository {
        records: Mutex<Vec<StoredModImportAnalysis>>,
    }

    impl ModImportResultRepository for FakeModImportResultRepository {
        fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> Result<()> {
            self.records
                .lock()
                .expect("records lock")
                .push(analysis.clone());
            Ok(())
        }

        fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
            Ok(self.records.lock().expect("records lock").clone())
        }

        fn get_analysis(&self, _mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
            unimplemented!("not needed by diagnostics export test")
        }
    }

    #[derive(Default)]
    struct RecordingDiagnosticPackageExporter {
        request: Mutex<Option<OwnedDiagnosticPackageExportRequest>>,
    }

    impl RecordingDiagnosticPackageExporter {
        fn take_request(&self) -> OwnedDiagnosticPackageExportRequest {
            self.request
                .lock()
                .expect("request lock")
                .take()
                .expect("export request")
        }
    }

    impl DiagnosticPackageExporter for RecordingDiagnosticPackageExporter {
        fn export_package(
            &self,
            request: DiagnosticPackageExportRequest<'_>,
        ) -> Result<DiagnosticPackageExportResult> {
            *self.request.lock().expect("request lock") =
                Some(OwnedDiagnosticPackageExportRequest {
                    file_name: request.file_name.to_owned(),
                    entries: request
                        .entries
                        .iter()
                        .map(|entry| OwnedDiagnosticPackageEntry {
                            name: entry.name.to_owned(),
                            bytes: entry.bytes.to_vec(),
                        })
                        .collect(),
                });

            Ok(DiagnosticPackageExportResult {
                export_id: request.file_name.to_owned(),
                file_name: request.file_name.to_owned(),
                size_bytes: 4096,
            })
        }
    }

    struct FixedClock {
        unix_millis: u128,
    }

    impl AppClock for FixedClock {
        fn now_unix_millis(&self) -> Result<u128> {
            Ok(self.unix_millis)
        }
    }

    struct OwnedDiagnosticPackageExportRequest {
        file_name: String,
        entries: Vec<OwnedDiagnosticPackageEntry>,
    }

    struct OwnedDiagnosticPackageEntry {
        name: String,
        bytes: Vec<u8>,
    }
}
