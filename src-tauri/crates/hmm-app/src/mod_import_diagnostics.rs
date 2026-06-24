use hmm_core::PreviewImageRejectionReason;
use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogWriter, DiagnosticPackageEntry,
    DiagnosticPackageExportRequest, DiagnosticPackageExporter, ModImportResultRepository,
    StoredImportPreviewImage, StoredModImportAnalysis,
};
use serde_json::json;
use std::collections::BTreeMap;
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
    audit_log: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
}

impl PreviewImageDiagnosticsExportService {
    pub fn new(
        result_repository: Arc<dyn ModImportResultRepository>,
        diagnostic_exporter: Arc<dyn DiagnosticPackageExporter>,
        audit_log: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            result_repository,
            diagnostic_exporter,
            audit_log,
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
        let export_timestamp = self.clock.now_unix_millis()?;
        let file_name = format!("preview-image-diagnostics-{}.zip", export_timestamp);
        let export = match self
            .diagnostic_exporter
            .export_package(DiagnosticPackageExportRequest {
                file_name: &file_name,
                entries: &[DiagnosticPackageEntry {
                    name: PREVIEW_IMAGE_DIAGNOSTICS_ENTRY_NAME,
                    bytes: &payload,
                }],
            }) {
            Ok(export) => export,
            Err(error) => {
                self.audit_log
                    .record(preview_image_diagnostics_export_failure_audit_event(
                        export_timestamp,
                        &file_name,
                        &diagnostics,
                    ))?;
                return Err(error);
            }
        };
        self.audit_log
            .record(preview_image_diagnostics_export_audit_event(
                export_timestamp,
                &export,
                &diagnostics,
            ))?;

        Ok(PreviewImageDiagnosticsExport {
            export_id: export.export_id,
            file_name: export.file_name,
            size_bytes: export.size_bytes,
            diagnostics,
        })
    }
}

fn preview_image_diagnostics_export_audit_event(
    timestamp_unix_millis: u128,
    export: &hmm_ports::DiagnosticPackageExportResult,
    diagnostics: &PreviewImageDiagnosticsSummary,
) -> AuditLogEvent {
    let mut fields = BTreeMap::new();
    fields.insert("export_id".to_owned(), export.export_id.clone());
    fields.insert("file_name".to_owned(), export.file_name.clone());
    fields.insert("size_bytes".to_owned(), export.size_bytes.to_string());
    insert_preview_image_diagnostics_counts(&mut fields, diagnostics);

    AuditLogEvent {
        timestamp_unix_millis,
        category: "diagnostic_export".to_owned(),
        operation: "export_preview_image_diagnostics".to_owned(),
        result: "success".to_owned(),
        fields,
    }
}

fn preview_image_diagnostics_export_failure_audit_event(
    timestamp_unix_millis: u128,
    file_name: &str,
    diagnostics: &PreviewImageDiagnosticsSummary,
) -> AuditLogEvent {
    let mut fields = BTreeMap::new();
    fields.insert("file_name".to_owned(), file_name.to_owned());
    fields.insert(
        "error_code".to_owned(),
        "diagnostic_package_export_failed".to_owned(),
    );
    insert_preview_image_diagnostics_counts(&mut fields, diagnostics);

    AuditLogEvent {
        timestamp_unix_millis,
        category: "diagnostic_export".to_owned(),
        operation: "export_preview_image_diagnostics".to_owned(),
        result: "failure".to_owned(),
        fields,
    }
}

fn insert_preview_image_diagnostics_counts(
    fields: &mut BTreeMap<String, String>,
    diagnostics: &PreviewImageDiagnosticsSummary,
) {
    fields.insert(
        "total_imported_mods".to_owned(),
        diagnostics.total_imported_mods.to_string(),
    );
    fields.insert(
        "thumbnail_count".to_owned(),
        diagnostics.thumbnail_count.to_string(),
    );
    fields.insert(
        "fallback_count".to_owned(),
        diagnostics.fallback_count.to_string(),
    );
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
        AppClock, AuditLogEvent, AuditLogWriter, DiagnosticPackageExportRequest,
        DiagnosticPackageExportResult, DiagnosticPackageExporter, ModImportResultRepository,
        StoredImportPreviewImage, StoredModPackageMetadata,
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
        let audit_log: Arc<dyn AuditLogWriter> = Arc::new(RecordingAuditLogWriter::default());
        let service = PreviewImageDiagnosticsExportService::new(
            repository,
            exporter_port,
            audit_log,
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

    #[test]
    fn export_service_records_sanitized_audit_event_for_diagnostics_export() {
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
        let exporter = Arc::new(RecordingDiagnosticPackageExporter::default());
        let exporter_port: Arc<dyn DiagnosticPackageExporter> = exporter.clone();
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let audit_log_port: Arc<dyn AuditLogWriter> = audit_log.clone();
        let service = PreviewImageDiagnosticsExportService::new(
            repository,
            exporter_port,
            audit_log_port,
            Arc::new(FixedClock { unix_millis: 42 }),
        );

        service
            .export_preview_image_diagnostics()
            .expect("export succeeds");

        let event = audit_log.take_event();
        assert_eq!(event.operation, "export_preview_image_diagnostics");
        assert_eq!(event.category, "diagnostic_export");
        assert_eq!(event.result, "success");
        assert_eq!(
            event.fields["export_id"],
            "preview-image-diagnostics-42.zip"
        );
        assert_eq!(
            event.fields["file_name"],
            "preview-image-diagnostics-42.zip"
        );
        assert_eq!(event.fields["size_bytes"], "4096");
        assert_eq!(event.fields["total_imported_mods"], "1");
        let serialized = serde_json::to_string(&event.fields).expect("serialize audit fields");
        assert!(!serialized.contains("thumbnail://"));
        assert!(!serialized.contains("secret-hash"));
        assert!(!serialized.contains("contentHash"));
        assert!(!serialized.contains("thumbnailUrl"));
        assert!(!serialized.contains("sandbox"));
        assert!(!serialized.contains("C:/"));
    }

    #[test]
    fn export_service_records_failure_audit_event_when_package_export_fails() {
        let repository = Arc::new(FakeModImportResultRepository::default());
        repository
            .save_analysis(&StoredModImportAnalysis {
                mod_id: "mod-1".to_owned(),
                task_id: "task-1".to_owned(),
                package_id: "pkg-1".to_owned(),
                display_name: "Preview Mod".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::CacheWriteFailed,
                },
            })
            .expect("save fallback analysis");
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let audit_log_port: Arc<dyn AuditLogWriter> = audit_log.clone();
        let service = PreviewImageDiagnosticsExportService::new(
            repository,
            Arc::new(FailingDiagnosticPackageExporter),
            audit_log_port,
            Arc::new(FixedClock { unix_millis: 42 }),
        );

        let error = service
            .export_preview_image_diagnostics()
            .expect_err("export fails");

        assert!(error.to_string().contains("C:/Users/Player"));
        let event = audit_log.take_event();
        assert_eq!(event.operation, "export_preview_image_diagnostics");
        assert_eq!(event.category, "diagnostic_export");
        assert_eq!(event.result, "failure");
        assert_eq!(
            event.fields["file_name"],
            "preview-image-diagnostics-42.zip"
        );
        assert_eq!(
            event.fields["error_code"],
            "diagnostic_package_export_failed"
        );
        assert_eq!(event.fields["total_imported_mods"], "1");
        assert_eq!(event.fields["fallback_count"], "1");
        let serialized = serde_json::to_string(&event.fields).expect("serialize audit fields");
        assert!(!serialized.contains("C:/Users/Player"));
        assert!(!serialized.contains("raw_path"));
        assert!(!serialized.contains("thumbnail://"));
        assert!(!serialized.contains("contentHash"));
        assert!(!serialized.contains("sandbox"));
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

    struct FailingDiagnosticPackageExporter;

    impl DiagnosticPackageExporter for FailingDiagnosticPackageExporter {
        fn export_package(
            &self,
            _request: DiagnosticPackageExportRequest<'_>,
        ) -> Result<DiagnosticPackageExportResult> {
            anyhow::bail!("failed to write C:/Users/Player/raw_path/mod.zip")
        }
    }

    #[derive(Default)]
    struct RecordingAuditLogWriter {
        event: Mutex<Option<AuditLogEvent>>,
    }

    impl RecordingAuditLogWriter {
        fn take_event(&self) -> AuditLogEvent {
            self.event
                .lock()
                .expect("audit event lock")
                .take()
                .expect("audit event")
        }
    }

    impl AuditLogWriter for RecordingAuditLogWriter {
        fn record(&self, event: AuditLogEvent) -> Result<()> {
            *self.event.lock().expect("audit event lock") = Some(event);
            Ok(())
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
