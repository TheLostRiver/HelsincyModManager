use anyhow::{Context, Result};
use hmm_ports::{TextLogKind, TextLogLine, TextLogReadRequest, TextLogReader};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub struct FileSystemTextLogReader {
    app_data_root: PathBuf,
}

impl FileSystemTextLogReader {
    pub fn new(app_data_root: PathBuf) -> Self {
        Self { app_data_root }
    }

    fn log_dir(&self, kind: TextLogKind) -> PathBuf {
        match kind {
            TextLogKind::App => self.app_data_root.join("logs").join("app"),
            TextLogKind::Task => self.app_data_root.join("logs").join("tasks"),
        }
    }
}

impl TextLogReader for FileSystemTextLogReader {
    fn read_recent_sanitized(&self, request: TextLogReadRequest) -> Result<Vec<TextLogLine>> {
        if request.max_lines == 0 {
            return Ok(Vec::new());
        }

        let log_dir = self.log_dir(request.kind);
        let directory_entries = match fs::read_dir(&log_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error).context("failed to read text log directory"),
        };

        let mut log_files = Vec::new();
        for entry in directory_entries {
            let entry = entry.context("failed to read text log directory entry")?;
            if !entry
                .file_type()
                .context("failed to inspect text log entry")?
                .is_file()
            {
                continue;
            }

            let file_name = entry.file_name();
            let Some(file_name) = file_name.to_str() else {
                continue;
            };
            if is_text_log_file_name(request.kind, file_name) {
                log_files.push((file_name.to_owned(), entry.path()));
            }
        }
        log_files.sort_by(|left, right| left.0.cmp(&right.0));

        let mut lines = Vec::new();
        for (source, log_path) in log_files {
            read_sanitized_lines_from_file(&source, &log_path, request.max_lines, &mut lines)?;
        }

        Ok(lines)
    }
}

fn read_sanitized_lines_from_file(
    source: &str,
    log_path: &Path,
    max_lines: usize,
    lines: &mut Vec<TextLogLine>,
) -> Result<()> {
    let file = File::open(log_path).context("failed to open text log")?;
    for line in BufReader::new(file).lines() {
        let line = line.context("failed to read text log line")?;
        if line.trim().is_empty() || !is_safe_log_line(&line) {
            continue;
        }
        if lines.len() == max_lines {
            lines.remove(0);
        }
        lines.push(TextLogLine {
            source: source.to_owned(),
            line: line.to_owned(),
        });
    }

    Ok(())
}

fn is_text_log_file_name(kind: TextLogKind, file_name: &str) -> bool {
    match kind {
        TextLogKind::App => is_calendar_log_file_name(file_name, "app-"),
        TextLogKind::Task => is_task_log_file_name(file_name),
    }
}

fn is_calendar_log_file_name(file_name: &str, prefix: &str) -> bool {
    let bytes = file_name.as_bytes();
    let expected_len = prefix.len() + "1970-01-01.log".len();
    bytes.len() == expected_len
        && file_name.starts_with(prefix)
        && file_name.ends_with(".log")
        && bytes[prefix.len()..prefix.len() + 4]
            .iter()
            .all(u8::is_ascii_digit)
        && bytes[prefix.len() + 4] == b'-'
        && bytes[prefix.len() + 5..prefix.len() + 7]
            .iter()
            .all(u8::is_ascii_digit)
        && bytes[prefix.len() + 7] == b'-'
        && bytes[prefix.len() + 8..prefix.len() + 10]
            .iter()
            .all(u8::is_ascii_digit)
}

fn is_task_log_file_name(file_name: &str) -> bool {
    let Some(task_id) = file_name
        .strip_prefix("task-")
        .and_then(|value| value.strip_suffix(".log"))
    else {
        return false;
    };
    !task_id.is_empty()
        && task_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_safe_log_line(line: &str) -> bool {
    if line.is_empty() || line.chars().any(char::is_control) {
        return false;
    }

    let lower = line.to_ascii_lowercase();
    const FORBIDDEN_SNIPPETS: &[&str] = &[
        "thumbnail://",
        "thumbnailurl",
        "contenthash",
        "raw_path",
        "raw_mod_content",
        "raw_save_content",
        "token",
        "cookie",
        "api_key",
        "sandbox",
        "c:/",
        "c:\\",
        "\\users\\",
        "/users/",
    ];

    !FORBIDDEN_SNIPPETS
        .iter()
        .any(|snippet| lower.contains(snippet))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn text_log_reader_returns_recent_sanitized_app_and_task_lines() {
        let temp = tempfile::tempdir().expect("temp dir");
        let app_log_dir = temp.path().join("logs").join("app");
        let task_log_dir = temp.path().join("logs").join("tasks");
        fs::create_dir_all(&app_log_dir).expect("create app log dir");
        fs::create_dir_all(&task_log_dir).expect("create task log dir");
        fs::write(
            app_log_dir.join("app-1970-01-01.log"),
            [
                "application started",
                "failed to open C:/Users/Player/raw_path/mod.zip",
                "token=secret",
            ]
            .join("\n"),
        )
        .expect("write app log");
        fs::write(
            app_log_dir.join("app-1970-01-02.log"),
            "game discovery completed with redacted paths\n",
        )
        .expect("write second app log");
        fs::write(app_log_dir.join("debug.log"), "ignored unsafe file name\n")
            .expect("write ignored app log");
        fs::write(
            task_log_dir.join("task-mod-import-42.log"),
            [
                "task queued",
                "thumbnail://pkg-1/preview-768/secret-hash",
                "task completed",
            ]
            .join("\n"),
        )
        .expect("write task log");

        let reader = FileSystemTextLogReader::new(temp.path().to_path_buf());

        let app_lines = reader
            .read_recent_sanitized(TextLogReadRequest {
                kind: TextLogKind::App,
                max_lines: 10,
            })
            .expect("read sanitized app log lines");

        assert_eq!(app_lines.len(), 2);
        assert_eq!(app_lines[0].source, "app-1970-01-01.log");
        assert_eq!(app_lines[0].line, "application started");
        assert_eq!(app_lines[1].source, "app-1970-01-02.log");
        assert_eq!(
            app_lines[1].line,
            "game discovery completed with redacted paths"
        );

        let task_lines = reader
            .read_recent_sanitized(TextLogReadRequest {
                kind: TextLogKind::Task,
                max_lines: 1,
            })
            .expect("read sanitized task log lines");

        assert_eq!(task_lines.len(), 1);
        assert_eq!(task_lines[0].source, "task-mod-import-42.log");
        assert_eq!(task_lines[0].line, "task completed");
        let serialized = serde_json::to_string(&(app_lines, task_lines)).expect("serialize lines");
        assert!(!serialized.contains("C:/Users/Player"));
        assert!(!serialized.contains("raw_path"));
        assert!(!serialized.contains("token"));
        assert!(!serialized.contains("thumbnail://"));
        assert!(!serialized.contains("secret-hash"));
    }
}
