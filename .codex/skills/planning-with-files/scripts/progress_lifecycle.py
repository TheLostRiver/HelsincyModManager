#!/usr/bin/env python3
from __future__ import annotations

from collections import Counter
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
import re
from typing import Iterable


SUMMARY_START = "<!-- PWF_COMPACT_SUMMARY_START -->"
SUMMARY_END = "<!-- PWF_COMPACT_SUMMARY_END -->"
AUTO_RECORD_PREFIX = "### Auto Record: "
AUTO_RECORD_FIELDS = (
    "- Tool:",
    "- Phase:",
    "- Result:",
    "- Command:",
)


@dataclass(frozen=True)
class AutoRecord:
    index: int
    lines: tuple[str, ...]
    timestamp: str
    tool: str
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


def _validate_archive_path(progress_path: Path, archive_path: Path) -> None:
    if archive_path.exists() and archive_path.is_dir():
        raise ValueError("archive path must be a file, not a directory")
    try:
        if progress_path.resolve() == archive_path.resolve():
            raise ValueError("archive path must be different from progress.md")
    except FileNotFoundError:
        if progress_path.absolute() == archive_path.absolute():
            raise ValueError("archive path must be different from progress.md")


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
    files: list[str] = []
    in_files = False
    for line in lines[1:]:
        if line.startswith("- Tool:"):
            tool = line.split(":", 1)[1].strip()
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
    return AutoRecord(index=index, lines=tuple(lines), timestamp=timestamp, tool=tool, files=tuple(files))


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
