#!/usr/bin/env python3
from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path, PurePosixPath
import hashlib
import json
import re
import secrets
from typing import Iterable


SUMMARY_START = "<!-- PWF_COMPACT_SUMMARY_START -->"
SUMMARY_END = "<!-- PWF_COMPACT_SUMMARY_END -->"
AUTO_RECORD_PREFIX = "### Auto Record: "
ARCHIVE_HEADER = "# Progress Archive"
AUTO_RECORD_FIELDS = (
    "- Tool:",
    "- Session:",
    "- Plan-Source:",
    "- Phase:",
    "- Result:",
    "- Command:",
)
DATA_BLOCK_DELIMITER_RE = re.compile(r"^---(?:BEGIN|END) [A-Z ][A-Z ]* DATA---$")
PROGRESS_TRUNCATION_NOTE = "[planning-with-files] progress context truncated; oldest content omitted."


@dataclass(frozen=True)
class AutoRecord:
    index: int
    lines: tuple[str, ...]
    timestamp: str
    tool: str
    session: str
    plan_source: str
    files: tuple[str, ...]


@dataclass(frozen=True)
class CompactResult:
    archived_count: int
    kept_count: int
    total_auto_records: int
    archive_path: Path
    changed: bool
    dry_run: bool
    summary: str


@dataclass(frozen=True)
class RolloverResult:
    archived_count: int
    kept_count: int
    total_auto_records: int
    archive_path: Path
    active_path: Path
    index_path: Path
    changed: bool
    dry_run: bool
    summary: str


@dataclass(frozen=True)
class ProgressDoctorIssue:
    severity: str
    code: str
    path: str
    message: str
    effect: str
    action: str


@dataclass(frozen=True)
class ProgressDoctorReport:
    progress_path: Path
    active_path: Path
    index_path: Path
    index_exists: bool
    rollover_events: int
    referenced_paths: tuple[str, ...]
    orphan_paths: tuple[str, ...]
    issues: tuple[ProgressDoctorIssue, ...]

    @property
    def has_errors(self) -> bool:
        return any(issue.severity == "error" for issue in self.issues)

    @property
    def has_warnings(self) -> bool:
        return any(issue.severity == "warn" for issue in self.issues)


INVALID_INDEX_JSON = "invalid_index_json"
INVALID_EVENT_SCHEMA = "invalid_event_schema"
PATH_ESCAPES_ROOT = "path_escapes_root"
ACTIVE_ROLE_MISMATCH = "active_role_mismatch"
ARCHIVE_ROLE_MISMATCH = "archive_role_mismatch"
MISSING_LATEST_ACTIVE = "missing_latest_active"
MISSING_SOURCE_ACTIVE = "missing_source_active"
MISSING_ARCHIVE = "missing_archive"
HASH_MISMATCH = "hash_mismatch"
ORPHAN_SEGMENT = "orphan_segment"
LEGACY_PROGRESS_ONLY = "legacy_progress_only"
LEGACY_ARCHIVE_FILE = "legacy_archive_file"


def count_auto_records(progress_path: Path) -> int:
    if not progress_path.is_file():
        return 0
    return sum(1 for line in _read_lines(progress_path) if line.startswith(AUTO_RECORD_PREFIX))


def extract_compaction_summary(progress_path: Path, line_limit: int = 20) -> str:
    if not progress_path.is_file() or line_limit < 1:
        return ""
    lines = _read_lines(progress_path)
    start = _find_line(lines, SUMMARY_START)
    end = _find_line(lines, SUMMARY_END)
    if start is None or end is None or end <= start:
        return ""
    summary_lines = lines[start + 1 : end]
    return "\n".join(summary_lines[:line_limit]).strip()


def extract_recent_progress_context(
    progress_path: Path,
    record_limit: int,
    manual_tail_lines: int,
    max_chars: int,
) -> str:
    if not progress_path.is_file() or max_chars < 1:
        return ""

    lines = _remove_managed_summary(_read_lines(progress_path))
    if not any(line.strip() for line in lines):
        return ""

    nodes, records = _parse_nodes(lines)
    kept_record_indexes = {record.index for record in records[-max(0, record_limit) :]}
    manual_keys = _recent_manual_line_keys(nodes, max(0, manual_tail_lines))

    units: list[tuple[str, ...]] = []
    for node_index, (kind, payload) in enumerate(nodes):
        if kind == "record":
            record = payload
            if isinstance(record, AutoRecord) and record.index in kept_record_indexes:
                units.append(tuple(_record_context_lines(record, max_chars)))
            continue

        text_lines = [
            line
            for line_index, line in enumerate(payload)  # type: ignore[arg-type]
            if (node_index, line_index) in manual_keys
        ]
        if text_lines:
            units.append(tuple(text_lines))

    return _escape_data_block_content(_render_context_units_with_budget(units, max_chars))


def compact_progress(
    progress_path: Path,
    archive_path: Path,
    keep_records: int = 30,
    dry_run: bool = False,
    now: str | None = None,
) -> CompactResult:
    if keep_records < 1:
        raise ValueError("keep_records must be at least 1")

    _validate_archive_path(progress_path, archive_path)

    if not progress_path.is_file():
        return CompactResult(
            archived_count=0,
            kept_count=0,
            total_auto_records=0,
            archive_path=archive_path,
            changed=False,
            dry_run=dry_run,
            summary="",
        )

    compact_time = now or datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    lines = _remove_managed_summary(_read_lines(progress_path))
    nodes, records = _parse_nodes(lines)
    total_records = len(records)
    archived_records = records[:-keep_records]
    kept_count = min(total_records, keep_records)
    archived_count = len(archived_records)

    if archived_count == 0:
        return CompactResult(
            archived_count=0,
            kept_count=kept_count,
            total_auto_records=total_records,
            archive_path=archive_path,
            changed=False,
            dry_run=dry_run,
            summary="",
        )

    summary = _render_summary(compact_time, archive_path, archived_records, kept_count)
    if not dry_run:
        archived_indexes = {record.index for record in archived_records}
        kept_lines = _render_nodes(nodes, archived_indexes)
        updated_lines = _insert_summary(kept_lines, summary.splitlines())
        _append_archive(archive_path, compact_time, progress_path, archived_records, kept_count)
        _write_lines(progress_path, updated_lines)

    return CompactResult(
        archived_count=archived_count,
        kept_count=kept_count,
        total_auto_records=total_records,
        archive_path=archive_path,
        changed=not dry_run,
        dry_run=dry_run,
        summary=summary,
    )


def _read_lines(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8", errors="replace").splitlines()


def _write_lines(path: Path, lines: Iterable[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8", newline="\n")


def progress_index_path(progress_path: Path) -> Path:
    return progress_path.parent / "progress-index.ndjson"


def _safe_timestamp(value: str) -> str:
    return re.sub(r"[^0-9]", "", value)[:14] or datetime.now().strftime("%Y%m%d%H%M%S")


def generated_segment_paths(progress_path: Path, session_key: str, now: str, nonce: str) -> tuple[Path, Path]:
    if not re.fullmatch(r"[0-9a-f]{12}|unavailable|[A-Za-z0-9_-]{1,64}", session_key):
        raise ValueError("invalid session key for progress rollover")
    stamp = _safe_timestamp(now)
    safe_nonce = re.sub(r"[^A-Za-z0-9_-]", "", nonce)[:16] or "segment"
    active = progress_path.parent / "progress-active" / session_key / f"active-{stamp}-{safe_nonce}.md"
    archive = progress_path.parent / "progress-archive" / session_key / f"archive-{stamp}-{safe_nonce}.md"
    return active, archive


def _create_text_exclusive(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="utf-8", newline="\n") as handle:
        handle.write(content)


def _sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def _sha256_file_text(path: Path) -> str:
    return _sha256_text(path.read_text(encoding="utf-8", errors="replace"))


def _is_hex_sha256(value: object) -> bool:
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def _is_safe_relative_ref(value: str) -> bool:
    if not value or "\\" in value:
        return False
    normalized = PurePosixPath(value).as_posix()
    parts = PurePosixPath(value).parts
    return (
        bool(parts)
        and normalized == value
        and all(part not in {"", ".", ".."} for part in parts)
        and not PurePosixPath(value).is_absolute()
    )


def _is_archive_segment_ref(value: str) -> bool:
    parts = PurePosixPath(value).parts
    return (
        len(parts) >= 3
        and parts[0] == "progress-archive"
        and all(part not in {"", ".", ".."} for part in parts)
        and parts[-1].startswith("archive-")
        and parts[-1].endswith(".md")
    )


def _issue(severity: str, code: str, path: str, message: str, effect: str, action: str) -> ProgressDoctorIssue:
    return ProgressDoctorIssue(
        severity=severity,
        code=code,
        path=path,
        message=message,
        effect=effect,
        action=action,
    )


def _dedupe_issues(issues: Iterable[ProgressDoctorIssue]) -> tuple[ProgressDoctorIssue, ...]:
    seen: set[tuple[str, str, str]] = set()
    deduped: list[ProgressDoctorIssue] = []
    for issue in issues:
        key = (issue.severity, issue.code, issue.path)
        if key in seen:
            continue
        seen.add(key)
        deduped.append(issue)
    return tuple(deduped)


def _relative_to_progress_root(progress_path: Path, target: Path) -> str:
    try:
        return target.relative_to(progress_path.parent).as_posix()
    except ValueError:
        raise ValueError("progress rollover path escaped progress directory") from None


def _read_index_events(index_path: Path) -> list[dict[str, object]]:
    if not index_path.is_file():
        return []
    events: list[dict[str, object]] = []
    for line in index_path.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(payload, dict):
            events.append(payload)
    return events


def _read_index_events_with_issues(
    index_path: Path,
) -> tuple[list[tuple[int, dict[str, object]]], list[ProgressDoctorIssue]]:
    if not index_path.is_file():
        return [], []
    events: list[tuple[int, dict[str, object]]] = []
    issues: list[ProgressDoctorIssue] = []
    lines = index_path.read_text(encoding="utf-8", errors="replace").splitlines()
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as exc:
            issues.append(
                _issue(
                    "error",
                    INVALID_INDEX_JSON,
                    f"{index_path.name}:{line_number}",
                    f"invalid JSON: {exc.msg}",
                    "Doctor cannot fully trust the rollover event chain after this line.",
                    "Inspect progress-index.ndjson manually; PWF did not modify files.",
                )
            )
            continue
        if not isinstance(payload, dict):
            issues.append(
                _issue(
                    "error",
                    INVALID_EVENT_SCHEMA,
                    f"{index_path.name}:{line_number}",
                    "index line is not a JSON object",
                    "The rollover event chain contains data that cannot describe progress storage.",
                    "Inspect progress-index.ndjson manually; PWF did not modify files.",
                )
            )
            continue
        events.append((line_number, payload))
    return events, issues


def _resolve_index_ref(
    progress_path: Path, value: object, *, role: str, line_number: int
) -> tuple[str | None, Path | None, list[ProgressDoctorIssue]]:
    index_path = progress_index_path(progress_path)
    if not isinstance(value, str) or not value:
        return None, None, [
            _issue(
                "error",
                INVALID_EVENT_SCHEMA,
                f"{index_path.name}:{line_number}",
                f"missing {role} path",
                "The rollover event cannot be used safely.",
                "Inspect progress-index.ndjson manually; PWF did not modify files.",
            )
        ]
    if not _is_safe_relative_ref(value):
        return value, None, [
            _issue(
                "error",
                PATH_ESCAPES_ROOT,
                value,
                f"{role} path is not a safe relative path",
                "A malformed reference could point outside the progress storage root.",
                "Inspect progress-index.ndjson manually; PWF did not modify files.",
            )
        ]
    candidate = progress_path.parent / value
    try:
        candidate.resolve().relative_to(progress_path.parent.resolve())
    except (OSError, ValueError):
        return value, None, [
            _issue(
                "error",
                PATH_ESCAPES_ROOT,
                value,
                f"{role} path escapes the progress storage root",
                "A malformed reference could point outside the progress storage root.",
                "Inspect progress-index.ndjson manually; PWF did not modify files.",
            )
        ]
    return value, candidate, []


def latest_rollover_event(progress_path: Path) -> dict[str, object] | None:
    for event in reversed(_read_index_events(progress_index_path(progress_path))):
        if event.get("event") == "rollover":
            return event
    return None


def _is_active_segment_ref(value: str) -> bool:
    parts = PurePosixPath(value).parts
    return (
        len(parts) >= 3
        and parts[0] == "progress-active"
        and all(part not in {"", ".", ".."} for part in parts)
        and parts[-1].startswith("active-")
        and parts[-1].endswith(".md")
    )


def _is_source_active_ref(value: str, progress_path: Path) -> bool:
    return value == progress_path.name or _is_active_segment_ref(value)


def current_active_progress(progress_path: Path) -> Path:
    for event in reversed(_read_index_events(progress_index_path(progress_path))):
        if event.get("event") != "rollover":
            continue
        new_active = event.get("new_active")
        if not isinstance(new_active, str) or not new_active:
            continue
        if not _is_active_segment_ref(new_active):
            continue
        candidate = progress_path.parent / new_active
        try:
            candidate.resolve().relative_to(progress_path.parent.resolve())
        except (OSError, ValueError):
            continue
        if candidate.is_file():
            return candidate
    return progress_path


def _generated_segment_inventory(progress_path: Path) -> tuple[str, ...]:
    inventory: list[str] = []
    for directory, matcher in (("progress-active", "active-*.md"), ("progress-archive", "archive-*.md")):
        base = progress_path.parent / directory
        if base.is_dir():
            inventory.extend(
                _relative_to_progress_root(progress_path, path)
                for path in sorted(base.glob(f"**/{matcher}"))
            )
    return tuple(sorted(inventory))


def _orphan_segment_issues(progress_path: Path, referenced: set[str]) -> tuple[list[str], list[ProgressDoctorIssue]]:
    orphan_paths: list[str] = []
    issues: list[ProgressDoctorIssue] = []
    for rel in _generated_segment_inventory(progress_path):
        if rel in referenced:
            continue
        orphan_paths.append(rel)
        issues.append(
            _issue(
                "warn",
                ORPHAN_SEGMENT,
                rel,
                "generated progress segment is not referenced by progress-index.ndjson",
                "A previous rollover may have created files before appending the index.",
                "Keep it for analysis, or remove manually only if you are sure it is no longer needed.",
            )
        )
    return orphan_paths, issues


def doctor_progress_storage(progress_path: Path) -> ProgressDoctorReport:
    index_path = progress_index_path(progress_path)
    events_with_lines, issues = _read_index_events_with_issues(index_path)
    rollover_events = [(line, event) for line, event in events_with_lines if event.get("event") == "rollover"]
    referenced: set[str] = set()
    active_path = progress_path

    if not index_path.is_file():
        if progress_path.is_file():
            issues.append(
                _issue(
                    "info",
                    LEGACY_PROGRESS_ONLY,
                    progress_path.name,
                    "legacy progress.md is the active progress file",
                    "No append-only rollover index exists yet.",
                    "No action is needed.",
                )
            )
        legacy_archive = progress_path.parent / "progress.archive.md"
        if legacy_archive.is_file():
            issues.append(
                _issue(
                    "info",
                    LEGACY_ARCHIVE_FILE,
                    "progress.archive.md",
                    "legacy progress archive file is present",
                    "This file is old-format cold storage and is not modified by rollover.",
                    "Keep it for audit, or remove it manually only if you no longer need it.",
                )
            )
        orphan_paths, orphan_issues = _orphan_segment_issues(progress_path, referenced)
        issues.extend(orphan_issues)
        return ProgressDoctorReport(
            progress_path,
            active_path,
            index_path,
            False,
            0,
            (),
            tuple(sorted(orphan_paths)),
            _dedupe_issues(issues),
        )

    for line_number, event in rollover_events:
        if event.get("version") != 1:
            issues.append(
                _issue(
                    "warn",
                    INVALID_EVENT_SCHEMA,
                    f"{index_path.name}:{line_number}",
                    f"unexpected rollover version={event.get('version')!r}",
                    "This doctor version may not understand every field in the event.",
                    "Upgrade PWF if this was written by a newer version.",
                )
            )

        old_active_ref, old_active_path, old_active_issues = _resolve_index_ref(
            progress_path,
            event.get("old_active"),
            role="old_active",
            line_number=line_number,
        )
        issues.extend(old_active_issues)
        source_sha256_valid = _is_hex_sha256(event.get("source_sha256"))
        if not source_sha256_valid:
            issues.append(
                _issue(
                    "error",
                    INVALID_EVENT_SCHEMA,
                    f"{index_path.name}:{line_number}",
                    "missing or invalid source_sha256",
                    "The rollover source file cannot be verified.",
                    "Inspect progress-index.ndjson manually; PWF did not modify files.",
                )
            )
        if old_active_ref:
            if old_active_path is not None:
                referenced.add(old_active_ref)
            if not _is_source_active_ref(old_active_ref, progress_path):
                issues.append(
                    _issue(
                        "error",
                        ACTIVE_ROLE_MISMATCH,
                        old_active_ref,
                        "old active path is not progress.md or under progress-active/",
                        "Rollover source files must be active progress files.",
                        "Inspect the index manually; PWF did not move files.",
                    )
                )
            elif old_active_path is not None and not old_active_path.is_file():
                issues.append(
                    _issue(
                        "error",
                        MISSING_SOURCE_ACTIVE,
                        old_active_ref,
                        "indexed old active progress file is missing",
                        "The rollover source audit chain cannot be verified.",
                        "Inspect storage manually; PWF did not recreate files.",
                    )
                )
            elif source_sha256_valid and old_active_path is not None and old_active_path.is_file():
                actual = _sha256_file_text(old_active_path)
                if actual != event["source_sha256"]:
                    issues.append(
                        _issue(
                            "error",
                            HASH_MISMATCH,
                            old_active_ref,
                            "old active SHA-256 does not match progress-index.ndjson",
                            "The rollover source audit chain is not trustworthy.",
                            "Inspect the file and index manually; PWF did not modify either file.",
                        )
                    )

        archive_ref, archive_path, archive_issues = _resolve_index_ref(
            progress_path,
            event.get("archive"),
            role="archive",
            line_number=line_number,
        )
        issues.extend(archive_issues)
        if archive_ref:
            if archive_path is not None:
                referenced.add(archive_ref)
            if not _is_archive_segment_ref(archive_ref):
                issues.append(
                    _issue(
                        "error",
                        ARCHIVE_ROLE_MISMATCH,
                        archive_ref,
                        "archive path is not under progress-archive/",
                        "Archive and active directories are role-separated to prevent writes to sealed history.",
                        "Inspect the index manually; PWF did not move files.",
                    )
                )
            elif archive_path is not None and not archive_path.is_file():
                issues.append(
                    _issue(
                        "warn",
                        MISSING_ARCHIVE,
                        archive_ref,
                        "indexed archive file is missing",
                        "Historical auto records referenced by this event may be unavailable.",
                        "Inspect storage manually; PWF did not recreate files.",
                    )
                )
            elif archive_path is not None and _is_hex_sha256(event.get("archive_sha256")):
                actual = _sha256_file_text(archive_path)
                if actual != event["archive_sha256"]:
                    issues.append(
                        _issue(
                            "error",
                            HASH_MISMATCH,
                            archive_ref,
                            "archive SHA-256 does not match progress-index.ndjson",
                            "The archive audit chain is not trustworthy.",
                            "Inspect the file and index manually; PWF did not modify either file.",
                        )
                    )

        active_ref, candidate_active, active_issues = _resolve_index_ref(
            progress_path,
            event.get("new_active"),
            role="new_active",
            line_number=line_number,
        )
        issues.extend(active_issues)
        if active_ref:
            if candidate_active is not None:
                referenced.add(active_ref)
            is_latest = (line_number, event) == rollover_events[-1]
            if not _is_active_segment_ref(active_ref):
                issues.append(
                    _issue(
                        "error",
                        ACTIVE_ROLE_MISMATCH,
                        active_ref,
                        "active path is not under progress-active/",
                        "Hooks must only append to active progress segments.",
                        "Inspect the index manually; PWF did not move files.",
                    )
                )
            elif candidate_active is not None and not candidate_active.is_file() and is_latest:
                issues.append(
                    _issue(
                        "error",
                        MISSING_LATEST_ACTIVE,
                        active_ref,
                        "latest indexed active progress segment is missing",
                        "Hooks may fall back to the previous valid active segment or legacy progress.md.",
                        "Inspect storage manually; PWF did not recreate files.",
                    )
                )
            elif candidate_active is not None and candidate_active.is_file():
                active_path = candidate_active

    orphan_paths, orphan_issues = _orphan_segment_issues(progress_path, referenced)
    issues.extend(orphan_issues)

    for path in sorted((progress_path.parent / "progress-active").glob("**/archive-*.md")):
        rel = _relative_to_progress_root(progress_path, path)
        issues.append(
            _issue(
                "error",
                ARCHIVE_ROLE_MISMATCH,
                rel,
                "archive-shaped file is under progress-active/",
                "Archive and active directories are role-separated to prevent writes to sealed history.",
                "Inspect storage manually; PWF did not move files.",
            )
        )

    for path in sorted((progress_path.parent / "progress-archive").glob("**/active-*.md")):
        rel = _relative_to_progress_root(progress_path, path)
        issues.append(
            _issue(
                "error",
                ACTIVE_ROLE_MISMATCH,
                rel,
                "active-shaped file is under progress-archive/",
                "Hooks must only append to active progress segments.",
                "Inspect storage manually; PWF did not move files.",
            )
        )

    legacy_archive = progress_path.parent / "progress.archive.md"
    if legacy_archive.is_file():
        issues.append(
            _issue(
                "info",
                LEGACY_ARCHIVE_FILE,
                "progress.archive.md",
                "legacy progress archive file is present",
                "The new append-only rollover does not write to this file.",
                "Keep it for audit, or remove it manually only if you no longer need it.",
            )
        )

    return ProgressDoctorReport(
        progress_path=progress_path,
        active_path=active_path,
        index_path=index_path,
        index_exists=True,
        rollover_events=len(rollover_events),
        referenced_paths=tuple(sorted(referenced)),
        orphan_paths=tuple(sorted(orphan_paths)),
        issues=_dedupe_issues(issues),
    )


def _render_rollover_archive(
    *,
    plan_id: str,
    session_key: str,
    source_name: str,
    source_sha256: str,
    created_at: str,
    archived_records: list[AutoRecord],
    kept_count: int,
) -> str:
    body_lines: list[str] = []
    for record in archived_records:
        body_lines.extend(record.lines)
        if body_lines and body_lines[-1].strip():
            body_lines.append("")
    return "\n".join(
        [
            "# Progress Archive",
            "",
            "- Version: 1",
            f"- Plan-ID: {plan_id}",
            f"- Session: {session_key}",
            f"- Source-Progress: {source_name}",
            f"- Source-SHA256: {source_sha256}",
            f"- Created-At: {created_at}",
            f"- Archived Auto Records: {len(archived_records)}",
            f"- Kept Recent Auto Records: {kept_count}",
            "",
            "---BEGIN ARCHIVED AUTO RECORDS---",
            "\n".join(body_lines).rstrip(),
            "---END ARCHIVED AUTO RECORDS---",
            "",
        ]
    )


def _render_rollover_active(
    *,
    plan_id: str,
    session_key: str,
    continued_from: str,
    continued_from_sha256: str,
    archive_relpath: str,
    created_at: str,
    kept_lines: list[str],
) -> str:
    return "\n".join(
        [
            "# Progress Log",
            "",
            "- Version: 1",
            f"- Plan-ID: {plan_id}",
            f"- Session: {session_key}",
            f"- Continued-From: {continued_from}",
            f"- Continued-From-SHA256: {continued_from_sha256}",
            f"- Archive: {archive_relpath}",
            f"- Created-At: {created_at}",
            "",
            "## Recent Progress",
            "",
            "\n".join(kept_lines).strip(),
            "",
        ]
    )


def rollover_progress(
    progress_path: Path,
    *,
    plan_id: str,
    session_key: str,
    keep_records: int = 30,
    dry_run: bool = False,
    now: str | None = None,
    nonce: str | None = None,
) -> RolloverResult:
    if keep_records < 1:
        raise ValueError("keep_records must be at least 1")

    created_at = now or datetime.now().strftime("%Y-%m-%dT%H:%M:%SZ")
    nonce_value = nonce or secrets.token_hex(6)
    active_path, archive_path = generated_segment_paths(progress_path, session_key, created_at, nonce_value)
    index_path = progress_index_path(progress_path)
    source_path = current_active_progress(progress_path)

    if not source_path.is_file():
        return RolloverResult(
            archived_count=0,
            kept_count=0,
            total_auto_records=0,
            archive_path=archive_path,
            active_path=active_path,
            index_path=index_path,
            changed=False,
            dry_run=dry_run,
            summary="",
        )

    source_text = source_path.read_text(encoding="utf-8", errors="replace")
    lines = _remove_managed_summary(source_text.splitlines())
    nodes, records = _parse_nodes(lines)
    total_records = len(records)
    archived_records = records[:-keep_records]
    kept_count = min(total_records, keep_records)
    archived_count = len(archived_records)

    if archived_count == 0:
        return RolloverResult(
            archived_count=0,
            kept_count=kept_count,
            total_auto_records=total_records,
            archive_path=archive_path,
            active_path=active_path,
            index_path=index_path,
            changed=False,
            dry_run=dry_run,
            summary="",
        )

    source_sha = _sha256_text(source_text)
    archived_indexes = {record.index for record in archived_records}
    kept_lines = _render_nodes(nodes, archived_indexes)
    archive_rel = _relative_to_progress_root(progress_path, archive_path)
    active_rel = _relative_to_progress_root(progress_path, active_path)
    source_rel = _relative_to_progress_root(progress_path, source_path)
    archive_text = _render_rollover_archive(
        plan_id=plan_id,
        session_key=session_key,
        source_name=source_rel,
        source_sha256=source_sha,
        created_at=created_at,
        archived_records=archived_records,
        kept_count=kept_count,
    )
    active_text = _render_rollover_active(
        plan_id=plan_id,
        session_key=session_key,
        continued_from=source_rel,
        continued_from_sha256=source_sha,
        archive_relpath=archive_rel,
        created_at=created_at,
        kept_lines=kept_lines,
    )
    summary = _render_summary(created_at, archive_path, archived_records, kept_count)

    if not dry_run:
        if archive_path.exists() or active_path.exists():
            raise FileExistsError("progress rollover segment already exists")
        _create_text_exclusive(archive_path, archive_text)
        _create_text_exclusive(active_path, active_text)
        event = {
            "event": "rollover",
            "version": 1,
            "created_at": created_at,
            "session": session_key,
            "old_active": source_rel,
            "archive": archive_rel,
            "new_active": active_rel,
            "source_sha256": source_sha,
            "archive_sha256": _sha256_text(archive_text),
            "new_active_sha256": _sha256_text(active_text),
            "archived_auto_records": archived_count,
            "kept_recent_auto_records": kept_count,
        }
        index_path.parent.mkdir(parents=True, exist_ok=True)
        with index_path.open("a", encoding="utf-8", newline="\n") as handle:
            handle.write(json.dumps(event, ensure_ascii=True, sort_keys=True) + "\n")

    return RolloverResult(
        archived_count=archived_count,
        kept_count=kept_count,
        total_auto_records=total_records,
        archive_path=archive_path,
        active_path=active_path,
        index_path=index_path,
        changed=not dry_run,
        dry_run=dry_run,
        summary=summary,
    )


def _validate_archive_path(progress_path: Path, archive_path: Path) -> None:
    if archive_path.exists() and archive_path.is_dir():
        raise ValueError("archive path must be a file, not a directory")
    try:
        progress_resolved = progress_path.resolve()
        archive_resolved = archive_path.resolve()
    except FileNotFoundError:
        progress_resolved = progress_path.absolute()
        archive_resolved = archive_path.absolute()

    if progress_resolved == archive_resolved:
        raise ValueError("archive path must be different from progress.md")
    if archive_resolved.parent != progress_resolved.parent:
        raise ValueError("archive path must stay in the same directory as progress.md")
    if archive_resolved.suffix.lower() != ".md":
        raise ValueError("archive path must be a Markdown file")
    if archive_path.is_file():
        existing = archive_path.read_text(encoding="utf-8", errors="replace").lstrip()
        if existing and not existing.startswith(ARCHIVE_HEADER):
            raise ValueError("archive path must be empty or an existing progress archive")


def _find_line(lines: list[str], needle: str) -> int | None:
    for index, line in enumerate(lines):
        if line.strip() == needle:
            return index
    return None


def _remove_managed_summary(lines: list[str]) -> list[str]:
    cleaned: list[str] = []
    skipping = False
    for line in lines:
        if line.strip() == SUMMARY_START:
            skipping = True
            continue
        if skipping:
            if line.strip() == SUMMARY_END:
                skipping = False
            continue
        cleaned.append(line)
    return _collapse_extra_blank_lines(cleaned)


def _collapse_extra_blank_lines(lines: list[str]) -> list[str]:
    collapsed: list[str] = []
    blank_count = 0
    for line in lines:
        if line.strip():
            blank_count = 0
            collapsed.append(line)
            continue
        blank_count += 1
        if blank_count <= 2:
            collapsed.append(line)
    return collapsed


def _parse_nodes(lines: list[str]) -> tuple[list[tuple[str, object]], list[AutoRecord]]:
    nodes: list[tuple[str, object]] = []
    records: list[AutoRecord] = []
    text_buffer: list[str] = []
    index = 0
    line_index = 0
    while line_index < len(lines):
        line = lines[line_index]
        if not line.startswith(AUTO_RECORD_PREFIX):
            text_buffer.append(line)
            line_index += 1
            continue

        if text_buffer:
            nodes.append(("text", tuple(text_buffer)))
            text_buffer = []

        record_lines = [line]
        in_files = False
        line_index += 1
        while line_index < len(lines):
            candidate = lines[line_index]
            if candidate.startswith(AUTO_RECORD_PREFIX):
                break
            if candidate.startswith("#") and candidate.strip():
                break
            if not _is_auto_record_body_line(candidate, in_files=in_files):
                break
            record_lines.append(candidate)
            if candidate.startswith("- Files:"):
                in_files = True
            elif candidate.strip() and not candidate.startswith("  - "):
                in_files = False
            line_index += 1

        record = _make_record(index, record_lines)
        records.append(record)
        nodes.append(("record", record))
        index += 1

    if text_buffer:
        nodes.append(("text", tuple(text_buffer)))
    return nodes, records


def _is_auto_record_body_line(line: str, *, in_files: bool) -> bool:
    if not line.strip():
        return True
    if in_files and line.startswith("  - "):
        return True
    if line.startswith("- Files:"):
        return True
    return line.startswith(AUTO_RECORD_FIELDS)


def _make_record(index: int, lines: list[str]) -> AutoRecord:
    timestamp = lines[0][len(AUTO_RECORD_PREFIX) :].strip()
    tool = ""
    session = ""
    plan_source = ""
    files: list[str] = []
    in_files = False
    for line in lines[1:]:
        if line.startswith("- Tool:"):
            tool = line.split(":", 1)[1].strip()
            in_files = False
            continue
        if line.startswith("- Session:"):
            session = line.split(":", 1)[1].strip()
            in_files = False
            continue
        if line.startswith("- Plan-Source:"):
            plan_source = line.split(":", 1)[1].strip()
            in_files = False
            continue
        if line.startswith("- Files:"):
            in_files = True
            continue
        if line.startswith("- ") and not line.startswith("  - "):
            in_files = False
        if not in_files:
            continue
        match = re.search(r"`([^`]+)`", line)
        if match:
            files.append(match.group(1))
    return AutoRecord(
        index=index,
        lines=tuple(lines),
        timestamp=timestamp,
        tool=tool,
        session=session,
        plan_source=plan_source,
        files=tuple(files),
    )


def _recent_manual_line_keys(nodes: list[tuple[str, object]], limit: int) -> set[tuple[int, int]]:
    if limit < 1:
        return set()

    keys: list[tuple[int, int]] = []
    for node_index, (kind, payload) in enumerate(nodes):
        if kind != "text":
            continue
        for line_index, line in enumerate(payload):  # type: ignore[arg-type]
            if line.strip():
                keys.append((node_index, line_index))
    return set(keys[-limit:])


def _record_context_lines(record: AutoRecord, max_chars: int) -> tuple[str, ...]:
    text = "\n".join(record.lines)
    if len(text) <= max_chars:
        return record.lines
    return _safe_record_summary_lines(record)


def _safe_record_summary_lines(record: AutoRecord) -> tuple[str, ...]:
    tool = record.tool or "unknown"
    timestamp = record.timestamp or "unknown"
    file_count = len(record.files)
    lines = [
        f"{AUTO_RECORD_PREFIX}{timestamp}",
        f"- Tool: {tool}",
    ]
    if record.session:
        lines.append(f"- Session: {record.session}")
    if record.plan_source:
        lines.append(f"- Plan-Source: {record.plan_source}")
    lines.extend(
        [
            f"- Files: {file_count} paths omitted due to size",
            "- Note: oversized auto record summarized for prompt safety",
        ]
    )
    return tuple(lines)


def _render_context_units_with_budget(units: list[tuple[str, ...]], max_chars: int) -> str:
    kept = list(units)
    truncated = False
    while kept and len(_join_units(kept)) > max_chars:
        kept.pop(0)
        truncated = True

    if not kept:
        return PROGRESS_TRUNCATION_NOTE if truncated else ""

    text = _join_units(kept)
    if truncated:
        text = f"{PROGRESS_TRUNCATION_NOTE}\n\n{text}"
    return text


def _join_units(units: list[tuple[str, ...]]) -> str:
    rendered: list[str] = []
    for unit in units:
        if rendered and rendered[-1].strip():
            rendered.append("")
        rendered.extend(unit)
    return "\n".join(rendered).strip()


def _escape_data_block_content(content: str) -> str:
    escaped: list[str] = []
    for line in content.splitlines():
        if DATA_BLOCK_DELIMITER_RE.fullmatch(line.strip()):
            escaped.append(f"[escaped delimiter] {line}")
        else:
            escaped.append(line)
    return "\n".join(escaped)


def _render_nodes(nodes: list[tuple[str, object]], archived_indexes: set[int]) -> list[str]:
    rendered: list[str] = []
    for kind, payload in nodes:
        if kind == "text":
            rendered.extend(payload)  # type: ignore[arg-type]
            continue
        record = payload
        if isinstance(record, AutoRecord) and record.index not in archived_indexes:
            rendered.extend(record.lines)
    return _collapse_extra_blank_lines(rendered)


def _insert_summary(lines: list[str], summary_lines: list[str]) -> list[str]:
    if not summary_lines:
        return lines
    if lines and lines[0].startswith("# "):
        rest = list(lines[1:])
        while rest and not rest[0].strip():
            rest.pop(0)
        return [lines[0], "", *summary_lines, "", *rest]
    return [*summary_lines, "", *lines]


def _render_summary(
    compact_time: str,
    archive_path: Path,
    archived_records: list[AutoRecord],
    kept_count: int,
) -> str:
    tools = Counter(record.tool or "unknown" for record in archived_records)
    files = sorted({file for record in archived_records for file in record.files})
    timestamps = [record.timestamp for record in archived_records if record.timestamp]
    time_range = f"{timestamps[0]} to {timestamps[-1]}" if timestamps else "unknown"
    tool_text = ", ".join(f"{tool}={count}" for tool, count in sorted(tools.items())) or "none"
    lines = [
        SUMMARY_START,
        "## Compacted Progress Summary",
        "",
        f"- Last Compact: {compact_time}",
        f"- Archive: {archive_path.name}",
        f"- Archived Auto Records: {len(archived_records)}",
        f"- Archived Range: {time_range}",
        f"- Kept Recent Auto Records: {kept_count}",
        f"- Tools: {tool_text}",
        f"- Unique Files: {len(files)}",
        "- Note: Archived records are objective hook facts. Agent summaries remain interpretive and should be verified when accuracy matters.",
        SUMMARY_END,
    ]
    return "\n".join(lines)


def _append_archive(
    archive_path: Path,
    compact_time: str,
    progress_path: Path,
    archived_records: list[AutoRecord],
    kept_count: int,
) -> None:
    archive_path.parent.mkdir(parents=True, exist_ok=True)
    existing = archive_path.read_text(encoding="utf-8", errors="replace") if archive_path.is_file() else ""
    header = "" if existing.strip() else "# Progress Archive\n\n"
    summary = _render_archive_summary(compact_time, progress_path, archived_records, kept_count)
    body_lines: list[str] = []
    for record in archived_records:
        body_lines.extend(record.lines)
        if body_lines and body_lines[-1].strip():
            body_lines.append("")
    batch = "\n".join(
        [
            summary,
            "",
            "---BEGIN ARCHIVED AUTO RECORDS---",
            "\n".join(body_lines).rstrip(),
            "---END ARCHIVED AUTO RECORDS---",
            "",
        ]
    )
    archive_path.write_text(existing.rstrip() + ("\n\n" if existing.strip() else "") + header + batch, encoding="utf-8", newline="\n")


def _render_archive_summary(
    compact_time: str,
    progress_path: Path,
    archived_records: list[AutoRecord],
    kept_count: int,
) -> str:
    tools = Counter(record.tool or "unknown" for record in archived_records)
    files = sorted({file for record in archived_records for file in record.files})
    timestamps = [record.timestamp for record in archived_records if record.timestamp]
    time_range = f"{timestamps[0]} to {timestamps[-1]}" if timestamps else "unknown"
    tool_text = ", ".join(f"{tool}={count}" for tool, count in sorted(tools.items())) or "none"
    return "\n".join(
        [
            f"## Compact Batch: {compact_time}",
            f"- Source: {progress_path.name}",
            f"- Archived Auto Records: {len(archived_records)}",
            f"- Archived Range: {time_range}",
            f"- Kept Recent Auto Records: {kept_count}",
            f"- Tools: {tool_text}",
            f"- Unique Files: {len(files)}",
        ]
    )
