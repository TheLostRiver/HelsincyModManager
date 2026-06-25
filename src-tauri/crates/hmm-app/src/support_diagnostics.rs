use hmm_ports::{
    AppClock, AuditLogEvent, AuditLogReadRequest, AuditLogReader, AuditLogWriter,
    DiagnosticPackageEntry, DiagnosticPackageExportRequest, DiagnosticPackageExporter,
    DiagnosticsEnvironmentProvider, DiagnosticsEnvironmentSummary, TextLogKind, TextLogLine,
    TextLogReadRequest, TextLogReader,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

const SUPPORT_DIAGNOSTICS_ENTRY_NAME: &str = "support-diagnostics.json";
const APP_LOG_DIAGNOSTICS_ENTRY_NAME: &str = "app-log-diagnostics.json";
const TASK_LOG_DIAGNOSTICS_ENTRY_NAME: &str = "task-log-diagnostics.json";
const SUPPORT_AUDIT_LOG_DIAGNOSTICS_ENTRY_NAME: &str = "audit-log-diagnostics.json";
pub const MAX_SUPPORT_DIAGNOSTIC_TEXT_LOG_LINES: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportDiagnosticsExport {
    pub export_id: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub app_log_line_count: usize,
    pub task_log_line_count: usize,
    pub audit_event_count: usize,
}

pub struct SupportDiagnosticsExportService {
    text_log_reader: Arc<dyn TextLogReader>,
    audit_log_reader: Arc<dyn AuditLogReader>,
    environment_provider: Arc<dyn DiagnosticsEnvironmentProvider>,
    diagnostic_exporter: Arc<dyn DiagnosticPackageExporter>,
    audit_log_writer: Arc<dyn AuditLogWriter>,
    clock: Arc<dyn AppClock>,
}

impl SupportDiagnosticsExportService {
    pub fn new(
        text_log_reader: Arc<dyn TextLogReader>,
        audit_log_reader: Arc<dyn AuditLogReader>,
        environment_provider: Arc<dyn DiagnosticsEnvironmentProvider>,
        diagnostic_exporter: Arc<dyn DiagnosticPackageExporter>,
        audit_log_writer: Arc<dyn AuditLogWriter>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            text_log_reader,
            audit_log_reader,
            environment_provider,
            diagnostic_exporter,
            audit_log_writer,
            clock,
        }
    }

    pub fn export_support_diagnostics(&self) -> anyhow::Result<SupportDiagnosticsExport> {
        let export_timestamp = self.clock.now_unix_millis()?;
        let platform_summary = self.environment_provider.summarize()?;
        let app_log_lines = self
            .text_log_reader
            .read_recent_sanitized(TextLogReadRequest {
                kind: TextLogKind::App,
                max_lines: MAX_SUPPORT_DIAGNOSTIC_TEXT_LOG_LINES,
            })?;
        let task_log_lines = self
            .text_log_reader
            .read_recent_sanitized(TextLogReadRequest {
                kind: TextLogKind::Task,
                max_lines: MAX_SUPPORT_DIAGNOSTIC_TEXT_LOG_LINES,
            })?;
        let audit_events = self
            .audit_log_reader
            .read_recent_sanitized(AuditLogReadRequest {
                max_events: crate::MAX_AUDIT_LOG_DIAGNOSTIC_EVENTS,
            })?;

        let support_payload = serde_json::to_vec(&support_diagnostics_payload(
            export_timestamp,
            &platform_summary,
            app_log_lines.len(),
            task_log_lines.len(),
            audit_events.len(),
        ))?;
        let app_log_payload =
            serde_json::to_vec(&text_log_diagnostics_payload("app_log", &app_log_lines))?;
        let task_log_payload =
            serde_json::to_vec(&text_log_diagnostics_payload("task_log", &task_log_lines))?;
        let audit_log_payload = serde_json::to_vec(&audit_log_diagnostics_payload(&audit_events))?;

        let file_name = format!("support-diagnostics-{}.zip", export_timestamp);
        let entries = [
            DiagnosticPackageEntry {
                name: SUPPORT_DIAGNOSTICS_ENTRY_NAME,
                bytes: &support_payload,
            },
            DiagnosticPackageEntry {
                name: APP_LOG_DIAGNOSTICS_ENTRY_NAME,
                bytes: &app_log_payload,
            },
            DiagnosticPackageEntry {
                name: TASK_LOG_DIAGNOSTICS_ENTRY_NAME,
                bytes: &task_log_payload,
            },
            DiagnosticPackageEntry {
                name: SUPPORT_AUDIT_LOG_DIAGNOSTICS_ENTRY_NAME,
                bytes: &audit_log_payload,
            },
        ];
        let export = match self
            .diagnostic_exporter
            .export_package(DiagnosticPackageExportRequest {
                file_name: &file_name,
                entries: &entries,
            }) {
            Ok(export) => export,
            Err(error) => {
                self.audit_log_writer
                    .record(support_diagnostics_export_failure_audit_event(
                        export_timestamp,
                        &file_name,
                        app_log_lines.len(),
                        task_log_lines.len(),
                        audit_events.len(),
                    ))?;
                return Err(error);
            }
        };

        self.audit_log_writer
            .record(support_diagnostics_export_success_audit_event(
                export_timestamp,
                &export,
                app_log_lines.len(),
                task_log_lines.len(),
                audit_events.len(),
            ))?;

        Ok(SupportDiagnosticsExport {
            export_id: export.export_id,
            file_name: export.file_name,
            size_bytes: export.size_bytes,
            app_log_line_count: app_log_lines.len(),
            task_log_line_count: task_log_lines.len(),
            audit_event_count: audit_events.len(),
        })
    }
}

fn support_diagnostics_payload(
    generated_at_unix_millis: u128,
    platform_summary: &DiagnosticsEnvironmentSummary,
    app_log_line_count: usize,
    task_log_line_count: usize,
    audit_event_count: usize,
) -> serde_json::Value {
    json!({
        "generatedAtUnixMillis": generated_at_unix_millis,
        "platformSummary": platform_summary,
        "exportCategories": [
            {
                "category": "platform_summary",
                "status": "included",
            },
            {
                "category": "app_log",
                "status": "included",
                "lineCount": app_log_line_count,
            },
            {
                "category": "task_log",
                "status": "included",
                "lineCount": task_log_line_count,
            },
            {
                "category": "audit_log",
                "status": "included",
                "eventCount": audit_event_count,
            },
        ],
    })
}

fn text_log_diagnostics_payload(kind: &str, lines: &[TextLogLine]) -> serde_json::Value {
    json!({
        "kind": kind,
        "lineCount": lines.len(),
        "lines": lines,
    })
}

fn audit_log_diagnostics_payload(events: &[AuditLogEvent]) -> serde_json::Value {
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

fn support_diagnostics_export_success_audit_event(
    timestamp_unix_millis: u128,
    export: &hmm_ports::DiagnosticPackageExportResult,
    app_log_line_count: usize,
    task_log_line_count: usize,
    audit_event_count: usize,
) -> AuditLogEvent {
    let mut fields = support_diagnostics_audit_fields(
        app_log_line_count,
        task_log_line_count,
        audit_event_count,
    );
    fields.insert("export_id".to_owned(), export.export_id.clone());
    fields.insert("file_name".to_owned(), export.file_name.clone());
    fields.insert("size_bytes".to_owned(), export.size_bytes.to_string());

    AuditLogEvent {
        timestamp_unix_millis,
        category: "diagnostic_export".to_owned(),
        operation: "export_support_diagnostics".to_owned(),
        result: "success".to_owned(),
        fields,
    }
}

fn support_diagnostics_export_failure_audit_event(
    timestamp_unix_millis: u128,
    file_name: &str,
    app_log_line_count: usize,
    task_log_line_count: usize,
    audit_event_count: usize,
) -> AuditLogEvent {
    let mut fields = support_diagnostics_audit_fields(
        app_log_line_count,
        task_log_line_count,
        audit_event_count,
    );
    fields.insert("file_name".to_owned(), file_name.to_owned());
    fields.insert(
        "error_code".to_owned(),
        "diagnostic_package_export_failed".to_owned(),
    );

    AuditLogEvent {
        timestamp_unix_millis,
        category: "diagnostic_export".to_owned(),
        operation: "export_support_diagnostics".to_owned(),
        result: "failure".to_owned(),
        fields,
    }
}

fn support_diagnostics_audit_fields(
    app_log_line_count: usize,
    task_log_line_count: usize,
    audit_event_count: usize,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "app_log_line_count".to_owned(),
            app_log_line_count.to_string(),
        ),
        (
            "task_log_line_count".to_owned(),
            task_log_line_count.to_string(),
        ),
        (
            "audit_event_count".to_owned(),
            audit_event_count.to_string(),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use hmm_ports::{
        AppClock, AuditLogEvent, AuditLogReadRequest, AuditLogReader, AuditLogWriter,
        DiagnosticPackageExportRequest, DiagnosticPackageExportResult, DiagnosticPackageExporter,
        DiagnosticsEnvironmentProvider, DiagnosticsEnvironmentSummary, TextLogKind, TextLogLine,
        TextLogReadRequest, TextLogReader,
    };
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    #[test]
    fn export_service_writes_support_diagnostics_package_and_records_audit() {
        let text_logs = Arc::new(StaticTextLogReader);
        let audit_reader = Arc::new(StaticAuditLogReader);
        let environment = Arc::new(StaticDiagnosticsEnvironmentProvider);
        let exporter = Arc::new(RecordingDiagnosticPackageExporter::default());
        let audit_log = Arc::new(RecordingAuditLogWriter::default());
        let service = SupportDiagnosticsExportService::new(
            text_logs,
            audit_reader,
            environment,
            exporter.clone(),
            audit_log.clone(),
            Arc::new(FixedClock { unix_millis: 42 }),
        );

        let export = service
            .export_support_diagnostics()
            .expect("support diagnostics export succeeds");

        assert_eq!(export.file_name, "support-diagnostics-42.zip");
        assert_eq!(export.export_id, "support-diagnostics-42.zip");
        assert_eq!(export.size_bytes, 4096);
        assert_eq!(export.app_log_line_count, 1);
        assert_eq!(export.task_log_line_count, 1);
        assert_eq!(export.audit_event_count, 1);

        let request = exporter.take_request().expect("export request");
        assert_eq!(request.file_name, "support-diagnostics-42.zip");
        assert_eq!(
            request.entry_names(),
            vec![
                "support-diagnostics.json",
                "app-log-diagnostics.json",
                "task-log-diagnostics.json",
                "audit-log-diagnostics.json",
            ]
        );
        let payload = request.all_payload_text();
        assert!(payload.contains("\"platformSummary\""));
        assert!(payload.contains("\"appVersion\":\"0.1.0-alpha.0\""));
        assert!(payload.contains("\"gameAdapterIds\":[\"mhw\"]"));
        assert!(payload.contains("\"app log ready\""));
        assert!(payload.contains("\"task completed\""));
        assert!(payload.contains("\"export_preview_image_diagnostics\""));
        assert!(!payload.contains("C:/"));
        assert!(!payload.contains("raw_path"));
        assert!(!payload.contains("thumbnail://"));
        assert!(!payload.contains("contentHash"));
        assert!(!payload.contains("sandbox"));

        let event = audit_log.take_event().expect("success audit event");
        assert_eq!(event.operation, "export_support_diagnostics");
        assert_eq!(event.category, "diagnostic_export");
        assert_eq!(event.result, "success");
        assert_eq!(event.fields["file_name"], "support-diagnostics-42.zip");
        assert_eq!(event.fields["size_bytes"], "4096");
        assert_eq!(event.fields["app_log_line_count"], "1");
        assert_eq!(event.fields["task_log_line_count"], "1");
        assert_eq!(event.fields["audit_event_count"], "1");
    }

    struct StaticTextLogReader;

    impl TextLogReader for StaticTextLogReader {
        fn read_recent_sanitized(&self, request: TextLogReadRequest) -> Result<Vec<TextLogLine>> {
            Ok(match request.kind {
                TextLogKind::App => vec![TextLogLine {
                    source: "app-1970-01-01.log".to_owned(),
                    line: "app log ready".to_owned(),
                }],
                TextLogKind::Task => vec![TextLogLine {
                    source: "task-mod-import-42.log".to_owned(),
                    line: "task completed".to_owned(),
                }],
            }
            .into_iter()
            .take(request.max_lines)
            .collect())
        }
    }

    struct StaticAuditLogReader;

    impl AuditLogReader for StaticAuditLogReader {
        fn read_recent_sanitized(
            &self,
            request: AuditLogReadRequest,
        ) -> Result<Vec<AuditLogEvent>> {
            Ok(vec![AuditLogEvent {
                timestamp_unix_millis: 42,
                category: "diagnostic_export".to_owned(),
                operation: "export_preview_image_diagnostics".to_owned(),
                result: "success".to_owned(),
                fields: BTreeMap::from([(
                    "file_name".to_owned(),
                    "preview-image-diagnostics-42.zip".to_owned(),
                )]),
            }]
            .into_iter()
            .take(request.max_events)
            .collect())
        }
    }

    struct StaticDiagnosticsEnvironmentProvider;

    impl DiagnosticsEnvironmentProvider for StaticDiagnosticsEnvironmentProvider {
        fn summarize(&self) -> Result<DiagnosticsEnvironmentSummary> {
            Ok(DiagnosticsEnvironmentSummary {
                app_version: "0.1.0-alpha.0".to_owned(),
                os: "windows".to_owned(),
                arch: "x86_64".to_owned(),
                game_adapter_ids: vec!["mhw".to_owned()],
            })
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

    impl OwnedDiagnosticPackageExportRequest {
        fn entry_names(&self) -> Vec<&str> {
            self.entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect()
        }

        fn all_payload_text(&self) -> String {
            self.entries
                .iter()
                .map(|entry| String::from_utf8(entry.bytes.clone()).expect("utf8 payload"))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    struct OwnedDiagnosticPackageEntry {
        name: String,
        bytes: Vec<u8>,
    }
}
