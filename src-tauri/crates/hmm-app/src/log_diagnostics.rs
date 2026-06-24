use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogReadRequest, AuditLogReader, AuditLogWriter,
    DiagnosticPackageEntry, DiagnosticPackageExportRequest, DiagnosticPackageExporter,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

const AUDIT_LOG_DIAGNOSTICS_ENTRY_NAME: &str = "audit-log-diagnostics.json";
pub const MAX_AUDIT_LOG_DIAGNOSTIC_EVENTS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLogDiagnosticsExport {
    pub export_id: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub audit_event_count: usize,
}

pub struct AuditLogDiagnosticsExportService {
    audit_log_reader: Arc<dyn AuditLogReader>,
    diagnostic_exporter: Arc<dyn DiagnosticPackageExporter>,
    audit_log_writer: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
}

impl AuditLogDiagnosticsExportService {
    pub fn new(
        audit_log_reader: Arc<dyn AuditLogReader>,
        diagnostic_exporter: Arc<dyn DiagnosticPackageExporter>,
        audit_log_writer: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            audit_log_reader,
            diagnostic_exporter,
            audit_log_writer,
            clock,
        }
    }

    pub fn export_audit_log_diagnostics(
        &self,
        max_events: usize,
    ) -> anyhow::Result<AuditLogDiagnosticsExport> {
        let export_timestamp = self.clock.now_unix_millis()?;
        let max_events = max_events.min(MAX_AUDIT_LOG_DIAGNOSTIC_EVENTS);
        let events = match self
            .audit_log_reader
            .read_recent_sanitized(AuditLogReadRequest { max_events })
        {
            Ok(events) => events,
            Err(error) => {
                self.audit_log_writer
                    .record(audit_log_diagnostics_export_failure_audit_event(
                        export_timestamp,
                        "audit-log-diagnostics-unavailable.zip",
                        "audit_log_read_failed",
                        0,
                    ))?;
                return Err(error);
            }
        };
        let payload = serde_json::to_vec(&sanitized_audit_log_diagnostics_payload(&events))?;
        let file_name = format!("audit-log-diagnostics-{}.zip", export_timestamp);
        let export = match self
            .diagnostic_exporter
            .export_package(DiagnosticPackageExportRequest {
                file_name: &file_name,
                entries: &[DiagnosticPackageEntry {
                    name: AUDIT_LOG_DIAGNOSTICS_ENTRY_NAME,
                    bytes: &payload,
                }],
            }) {
            Ok(export) => export,
            Err(error) => {
                self.audit_log_writer
                    .record(audit_log_diagnostics_export_failure_audit_event(
                        export_timestamp,
                        &file_name,
                        "diagnostic_package_export_failed",
                        events.len(),
                    ))?;
                return Err(error);
            }
        };

        self.audit_log_writer
            .record(audit_log_diagnostics_export_success_audit_event(
                export_timestamp,
                &export,
                events.len(),
            ))?;

        Ok(AuditLogDiagnosticsExport {
            export_id: export.export_id,
            file_name: export.file_name,
            size_bytes: export.size_bytes,
            audit_event_count: events.len(),
        })
    }
}

fn sanitized_audit_log_diagnostics_payload(events: &[AuditLogEvent]) -> serde_json::Value {
    json!({
        "auditEventCount": events.len(),
        "events": events.iter().map(|event| {
            json!({
                "timestampUnixMillis": event.timestamp_unix_millis,
                "category": event.category,
                "operation": event.operation,
                "result": event.result,
                "fields": event.fields,
            })
        }).collect::<Vec<_>>(),
    })
}

fn audit_log_diagnostics_export_success_audit_event(
    timestamp_unix_millis: u128,
    export: &hmm_ports::DiagnosticPackageExportResult,
    audit_event_count: usize,
) -> AuditLogEvent {
    let mut fields = BTreeMap::new();
    fields.insert("export_id".to_owned(), export.export_id.clone());
    fields.insert("file_name".to_owned(), export.file_name.clone());
    fields.insert("size_bytes".to_owned(), export.size_bytes.to_string());
    fields.insert(
        "audit_event_count".to_owned(),
        audit_event_count.to_string(),
    );

    AuditLogEvent {
        timestamp_unix_millis,
        category: "diagnostic_export".to_owned(),
        operation: "export_audit_log_diagnostics".to_owned(),
        result: "success".to_owned(),
        fields,
    }
}

fn audit_log_diagnostics_export_failure_audit_event(
    timestamp_unix_millis: u128,
    file_name: &str,
    error_code: &str,
    audit_event_count: usize,
) -> AuditLogEvent {
    let mut fields = BTreeMap::new();
    fields.insert("file_name".to_owned(), file_name.to_owned());
    fields.insert("error_code".to_owned(), error_code.to_owned());
    fields.insert(
        "audit_event_count".to_owned(),
        audit_event_count.to_string(),
    );

    AuditLogEvent {
        timestamp_unix_millis,
        category: "diagnostic_export".to_owned(),
        operation: "export_audit_log_diagnostics".to_owned(),
        result: "failure".to_owned(),
        fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use hmm_ports::DiagnosticPackageExportResult;
    use std::sync::Mutex;

    #[test]
    fn export_service_writes_sanitized_audit_log_diagnostics_package_and_records_audit() {
        let reader = Arc::new(StaticAuditLogReader {
            events: vec![
                AuditLogEvent {
                    timestamp_unix_millis: 42,
                    category: "diagnostic_export".to_owned(),
                    operation: "export_preview_image_diagnostics".to_owned(),
                    result: "success".to_owned(),
                    fields: BTreeMap::from([(
                        "file_name".to_owned(),
                        "preview-image-diagnostics-42.zip".to_owned(),
                    )]),
                },
                AuditLogEvent {
                    timestamp_unix_millis: 86_400_000,
                    category: "diagnostic_export".to_owned(),
                    operation: "export_preview_image_diagnostics".to_owned(),
                    result: "failure".to_owned(),
                    fields: BTreeMap::from([(
                        "error_code".to_owned(),
                        "diagnostic_package_export_failed".to_owned(),
                    )]),
                },
            ],
        });
        let exporter = Arc::new(RecordingDiagnosticPackageExporter::default());
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let service = AuditLogDiagnosticsExportService::new(
            reader,
            exporter.clone(),
            audit_log.clone(),
            Arc::new(FixedClock { unix_millis: 42 }),
        );

        let export = service
            .export_audit_log_diagnostics(2)
            .expect("export succeeds");

        assert_eq!(export.file_name, "audit-log-diagnostics-42.zip");
        assert_eq!(export.export_id, "audit-log-diagnostics-42.zip");
        assert_eq!(export.size_bytes, 4096);
        assert_eq!(export.audit_event_count, 2);
        let request = exporter.take_request().expect("export request");
        assert_eq!(request.file_name, "audit-log-diagnostics-42.zip");
        assert_eq!(request.entries.len(), 1);
        assert_eq!(request.entries[0].name, "audit-log-diagnostics.json");
        let payload = String::from_utf8(request.entries[0].bytes.clone()).expect("utf8 payload");
        assert!(payload.contains("\"auditEventCount\":2"));
        assert!(payload.contains("\"export_preview_image_diagnostics\""));
        assert!(payload.contains("\"diagnostic_package_export_failed\""));
        assert!(!payload.contains("C:/"));
        assert!(!payload.contains("raw_path"));
        assert!(!payload.contains("thumbnail://"));
        assert!(!payload.contains("contentHash"));
        assert!(!payload.contains("sandbox"));

        let event = audit_log.take_event().expect("success audit event");
        assert_eq!(event.operation, "export_audit_log_diagnostics");
        assert_eq!(event.category, "diagnostic_export");
        assert_eq!(event.result, "success");
        assert_eq!(event.fields["file_name"], "audit-log-diagnostics-42.zip");
        assert_eq!(event.fields["size_bytes"], "4096");
        assert_eq!(event.fields["audit_event_count"], "2");
        let serialized = serde_json::to_string(&event.fields).expect("serialize audit fields");
        assert!(!serialized.contains("C:/"));
        assert!(!serialized.contains("raw_path"));
        assert!(!serialized.contains("thumbnail://"));
        assert!(!serialized.contains("contentHash"));
        assert!(!serialized.contains("sandbox"));
    }

    #[test]
    fn export_service_caps_requested_audit_log_event_count() {
        let reader = Arc::new(StaticAuditLogReader {
            events: (0..201)
                .map(|index| AuditLogEvent {
                    timestamp_unix_millis: index,
                    category: "diagnostic_export".to_owned(),
                    operation: "export_preview_image_diagnostics".to_owned(),
                    result: "success".to_owned(),
                    fields: BTreeMap::from([(
                        "file_name".to_owned(),
                        format!("preview-image-diagnostics-{index}.zip"),
                    )]),
                })
                .collect(),
        });
        let exporter = Arc::new(RecordingDiagnosticPackageExporter::default());
        let service = AuditLogDiagnosticsExportService::new(
            reader,
            exporter.clone(),
            Arc::new(RecordingAuditLogWriter::default()),
            Arc::new(FixedClock { unix_millis: 42 }),
        );

        let export = service
            .export_audit_log_diagnostics(usize::MAX)
            .expect("export succeeds");

        assert_eq!(export.audit_event_count, 200);
        let request = exporter.take_request().expect("export request");
        let payload = String::from_utf8(request.entries[0].bytes.clone()).expect("utf8 payload");
        assert!(payload.contains("\"auditEventCount\":200"));
        assert!(payload.contains("\"preview-image-diagnostics-200.zip\""));
        assert!(!payload.contains("\"preview-image-diagnostics-0.zip\""));
    }

    #[test]
    fn export_service_records_failure_audit_event_when_audit_log_read_fails() {
        let exporter = Arc::new(RecordingDiagnosticPackageExporter::default());
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let service = AuditLogDiagnosticsExportService::new(
            Arc::new(FailingAuditLogReader),
            exporter.clone(),
            audit_log.clone(),
            Arc::new(FixedClock { unix_millis: 42 }),
        );

        let error = service
            .export_audit_log_diagnostics(2)
            .expect_err("audit log read fails");

        assert!(error.to_string().contains("C:/Users/Player"));
        assert!(
            exporter.take_request().is_none(),
            "diagnostic package must not be exported when audit log read fails"
        );
        let event = audit_log.take_event().expect("failure audit event");
        assert_eq!(event.operation, "export_audit_log_diagnostics");
        assert_eq!(event.category, "diagnostic_export");
        assert_eq!(event.result, "failure");
        assert_eq!(
            event.fields["file_name"],
            "audit-log-diagnostics-unavailable.zip"
        );
        assert_eq!(event.fields["error_code"], "audit_log_read_failed");
        assert_eq!(event.fields["audit_event_count"], "0");
        let serialized = serde_json::to_string(&event.fields).expect("serialize audit fields");
        assert!(!serialized.contains("C:/Users/Player"));
        assert!(!serialized.contains("raw_path"));
        assert!(!serialized.contains("thumbnail://"));
        assert!(!serialized.contains("contentHash"));
        assert!(!serialized.contains("sandbox"));
    }

    #[test]
    fn export_service_records_failure_audit_event_when_package_export_fails() {
        let reader = Arc::new(StaticAuditLogReader {
            events: vec![AuditLogEvent {
                timestamp_unix_millis: 42,
                category: "diagnostic_export".to_owned(),
                operation: "export_preview_image_diagnostics".to_owned(),
                result: "success".to_owned(),
                fields: BTreeMap::from([(
                    "file_name".to_owned(),
                    "preview-image-diagnostics-42.zip".to_owned(),
                )]),
            }],
        });
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let service = AuditLogDiagnosticsExportService::new(
            reader,
            Arc::new(FailingDiagnosticPackageExporter),
            audit_log.clone(),
            Arc::new(FixedClock { unix_millis: 42 }),
        );

        let error = service
            .export_audit_log_diagnostics(2)
            .expect_err("diagnostic package export fails");

        assert!(error.to_string().contains("C:/Users/Player"));
        let event = audit_log.take_event().expect("failure audit event");
        assert_eq!(event.operation, "export_audit_log_diagnostics");
        assert_eq!(event.category, "diagnostic_export");
        assert_eq!(event.result, "failure");
        assert_eq!(event.fields["file_name"], "audit-log-diagnostics-42.zip");
        assert_eq!(
            event.fields["error_code"],
            "diagnostic_package_export_failed"
        );
        assert_eq!(event.fields["audit_event_count"], "1");
        let serialized = serde_json::to_string(&event.fields).expect("serialize audit fields");
        assert!(!serialized.contains("C:/Users/Player"));
        assert!(!serialized.contains("raw_path"));
        assert!(!serialized.contains("thumbnail://"));
        assert!(!serialized.contains("contentHash"));
        assert!(!serialized.contains("sandbox"));
    }

    struct StaticAuditLogReader {
        events: Vec<AuditLogEvent>,
    }

    impl AuditLogReader for StaticAuditLogReader {
        fn read_recent_sanitized(
            &self,
            request: AuditLogReadRequest,
        ) -> Result<Vec<AuditLogEvent>> {
            Ok(self
                .events
                .iter()
                .rev()
                .take(request.max_events)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect())
        }
    }

    #[derive(Default)]
    struct RecordingDiagnosticPackageExporter {
        request: Mutex<Option<OwnedDiagnosticPackageExportRequest>>,
    }

    impl RecordingDiagnosticPackageExporter {
        fn take_request(&self) -> Option<OwnedDiagnosticPackageExportRequest> {
            self.request.lock().expect("request lock").take()
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

    struct FailingAuditLogReader;

    impl AuditLogReader for FailingAuditLogReader {
        fn read_recent_sanitized(
            &self,
            _request: AuditLogReadRequest,
        ) -> Result<Vec<AuditLogEvent>> {
            anyhow::bail!("failed to read C:/Users/Player/raw_path/audit.log")
        }
    }

    struct FailingDiagnosticPackageExporter;

    impl DiagnosticPackageExporter for FailingDiagnosticPackageExporter {
        fn export_package(
            &self,
            _request: DiagnosticPackageExportRequest<'_>,
        ) -> Result<DiagnosticPackageExportResult> {
            anyhow::bail!("failed to write C:/Users/Player/raw_path/audit-log.zip")
        }
    }

    #[derive(Default)]
    struct RecordingAuditLogWriter {
        event: Mutex<Option<AuditLogEvent>>,
    }

    impl RecordingAuditLogWriter {
        fn take_event(&self) -> Option<AuditLogEvent> {
            self.event.lock().expect("audit event lock").take()
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
