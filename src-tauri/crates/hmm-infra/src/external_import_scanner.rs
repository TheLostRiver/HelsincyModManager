use crate::controlled_fs::{
    is_link_or_reparse, open_child_directory_nofollow, open_existing_directory_nofollow,
    open_regular_file_nofollow,
};
use crate::external_import_source_registry::{
    HuntingBoxDirectorySourceRegistry, RegisteredHuntingBoxSource,
    HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID,
};
use anyhow::{anyhow, bail, Result};
use cap_std::fs::{Dir, File, Metadata};
use hmm_core::{
    ExternalImportBatchId, ExternalImportCandidate, ExternalImportCandidateId,
    ExternalImportCandidateStatus, ExternalImportConflictKind, ExternalImportMetadataHint,
    ExternalImportResourceBudget, ExternalImportResourceUsage,
};
use hmm_ports::{
    CancellationToken, ExternalImportScanRequest, ExternalImportScanResult, ExternalImportScanner,
};
use quick_xml::events::Event;
use quick_xml::Reader;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::{Read, Take};
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

const INFO_XML_MAX_BYTES: u64 = 64 * 1024;
const INFO_XML_MAX_DEPTH: usize = 32;
const INFO_XML_MAX_FIELDS: usize = 32;
const INFO_XML_MAX_FIELD_CHARS: usize = 256;
const INFO_XML_MAX_TEXT_CHARS: usize = 4096;
const FINGERPRINT_READ_BUFFER_BYTES: usize = 64 * 1024;
const UNAVAILABLE_CONTENT_FINGERPRINT: &str = "unavailable";

pub struct HuntingBoxDirectoryScanner {
    registry: Arc<HuntingBoxDirectorySourceRegistry>,
}

impl HuntingBoxDirectoryScanner {
    pub fn new(registry: Arc<HuntingBoxDirectorySourceRegistry>) -> Self {
        Self { registry }
    }
}

impl ExternalImportScanner for HuntingBoxDirectoryScanner {
    fn scan(&self, request: ExternalImportScanRequest<'_>) -> Result<ExternalImportScanResult> {
        ensure_not_cancelled(request.cancellation_token)?;
        let registration = self
            .registry
            .resolve_directory(&request.source.source_id)?
            .ok_or_else(|| anyhow!("external import source is unavailable"))?;
        if registration.source.adapter_id.as_str() != HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID
            || request.batch.adapter_id != registration.source.adapter_id
            || request.source.adapter_id != registration.source.adapter_id
        {
            bail!("external import source adapter is invalid");
        }
        let root_directory = open_existing_directory_nofollow(
            &registration.root_directory,
            "external import source root",
        )
        .map_err(|_| anyhow!("external import source is unavailable"))?;

        let mut entries = match read_root_entries(
            &root_directory,
            request.resource_budget.max_total_candidates,
            request.cancellation_token,
        ) {
            Ok(RootEntryRead::Complete(entries)) => entries,
            Ok(RootEntryRead::CandidateLimitExceeded) => {
                return Ok(ExternalImportScanResult {
                    candidates: vec![self.root_candidate_limit_exceeded(
                        &registration,
                        request.batch.batch_id.clone(),
                    )],
                    observed_resource_usage: ExternalImportResourceUsage::default(),
                });
            }
            Err(WalkFailure::Cancelled) => bail!("external import scan cancelled"),
            Err(_) => return Err(anyhow!("external import source is unavailable")),
        };
        entries.sort_by_key(|entry| root_entry_sort_key(entry));

        let mut candidates = Vec::with_capacity(entries.len());
        let mut observed_resource_usage = ExternalImportResourceUsage::default();
        for (ordinal, entry) in entries.into_iter().enumerate() {
            ensure_not_cancelled(request.cancellation_token)?;
            candidates.push(self.scan_entry(
                &registration,
                &root_directory,
                request.batch.batch_id.clone(),
                ordinal,
                entry,
                request.resource_budget,
                request.cancellation_token,
                &mut observed_resource_usage,
            )?);
        }

        mark_duplicate_content(&mut candidates);
        mark_duplicate_display_names(&mut candidates);
        Ok(ExternalImportScanResult {
            candidates,
            observed_resource_usage,
        })
    }
}

impl HuntingBoxDirectoryScanner {
    fn root_candidate_limit_exceeded(
        &self,
        registration: &RegisteredHuntingBoxSource,
        batch_id: ExternalImportBatchId,
    ) -> ExternalImportCandidate {
        let mut candidate = new_candidate(
            batch_id,
            self.registry
                .source_item_key_hash(registration, b"root-candidate-limit-exceeded"),
        );
        candidate.preview_status = ExternalImportCandidateStatus::ResourceLimitExceeded;
        candidate
    }

    #[allow(clippy::too_many_arguments)]
    fn scan_entry(
        &self,
        registration: &RegisteredHuntingBoxSource,
        root_directory: &Dir,
        batch_id: ExternalImportBatchId,
        ordinal: usize,
        file_name: OsString,
        resource_budget: &ExternalImportResourceBudget,
        cancellation_token: &dyn CancellationToken,
        observed_resource_usage: &mut ExternalImportResourceUsage,
    ) -> Result<ExternalImportCandidate> {
        let item_identity = source_item_identity(&file_name, ordinal);
        let source_item_key_hash = self
            .registry
            .source_item_key_hash(registration, item_identity.as_bytes());
        let mut candidate = new_candidate(batch_id, source_item_key_hash);
        let entry_metadata = match root_directory.symlink_metadata(&file_name) {
            Ok(metadata) => metadata,
            Err(_) => {
                candidate.preview_status = ExternalImportCandidateStatus::SourceUnreadable;
                return Ok(candidate);
            }
        };

        if is_link_or_reparse(&entry_metadata) {
            candidate.preview_status = ExternalImportCandidateStatus::StructureInvalid;
            return Ok(candidate);
        }
        if !entry_metadata.is_dir() {
            candidate.preview_status = ExternalImportCandidateStatus::UnsupportedEntry;
            return Ok(candidate);
        }
        let Some(directory_name) = file_name.to_str() else {
            candidate.preview_status = ExternalImportCandidateStatus::StructureInvalid;
            return Ok(candidate);
        };
        if !is_numeric_directory_name(directory_name) {
            candidate.preview_status = ExternalImportCandidateStatus::StructureInvalid;
            return Ok(candidate);
        }

        let item_directory = match open_child_directory_nofollow(
            root_directory,
            &file_name,
            "external import candidate directory",
        ) {
            Ok(directory) => directory,
            Err(_) => {
                candidate.preview_status = ExternalImportCandidateStatus::SourceUnreadable;
                return Ok(candidate);
            }
        };
        let files_directory = match open_child_directory_nofollow(
            &item_directory,
            std::ffi::OsStr::new("files"),
            "external import candidate files directory",
        ) {
            Ok(directory) => directory,
            Err(_) => {
                candidate.preview_status = ExternalImportCandidateStatus::StructureInvalid;
                return Ok(candidate);
            }
        };
        let mut info_xml = match open_regular_file_nofollow(
            &item_directory,
            std::ffi::OsStr::new("info.xml"),
            "external import candidate metadata",
        ) {
            Ok(file) => file,
            Err(_) => {
                candidate.preview_status = ExternalImportCandidateStatus::StructureInvalid;
                return Ok(candidate);
            }
        };
        if !files_directory
            .dir_metadata()
            .is_ok_and(|metadata| metadata.is_dir() && !is_link_or_reparse(&metadata))
        {
            candidate.preview_status = ExternalImportCandidateStatus::StructureInvalid;
            return Ok(candidate);
        }

        let limits = remaining_content_limits(resource_budget, *observed_resource_usage);
        let content_scan = scan_content(&files_directory, limits, cancellation_token);
        match content_scan {
            ContentScanOutcome::Cancelled => bail!("external import scan cancelled"),
            ContentScanOutcome::Rejected { issue, usage } => {
                let _ = add_observed_usage(observed_resource_usage, usage, resource_budget);
                candidate.resource_usage = usage;
                candidate.preview_status = issue.into_status();
                return Ok(candidate);
            }
            ContentScanOutcome::Complete(content) => {
                if !add_observed_usage(observed_resource_usage, content.usage, resource_budget) {
                    candidate.resource_usage = content.usage;
                    candidate.preview_status = ExternalImportCandidateStatus::ResourceLimitExceeded;
                    return Ok(candidate);
                }
                candidate.resource_usage = content.usage;
                candidate.content_fingerprint = content.content_fingerprint;
            }
        }

        match parse_info_xml(&mut info_xml) {
            Ok(metadata_hint) => {
                candidate.metadata_hint = metadata_hint;
                candidate.preview_status = ExternalImportCandidateStatus::Ready;
            }
            Err(_) => {
                candidate.preview_status = ExternalImportCandidateStatus::MetadataInvalid;
            }
        }
        Ok(candidate)
    }
}

fn new_candidate(
    batch_id: ExternalImportBatchId,
    source_item_key_hash: String,
) -> ExternalImportCandidate {
    ExternalImportCandidate {
        batch_id,
        candidate_id: ExternalImportCandidateId::new(format!(
            "external-import-candidate-{}",
            Uuid::new_v4()
        )),
        source_item_key_hash,
        content_fingerprint: UNAVAILABLE_CONTENT_FINGERPRINT.to_owned(),
        metadata_hint: ExternalImportMetadataHint::default(),
        resource_usage: ExternalImportResourceUsage::default(),
        preview_status: ExternalImportCandidateStatus::StructureInvalid,
        conflict_kind: ExternalImportConflictKind::None,
    }
}

fn source_item_identity(file_name: &OsStr, ordinal: usize) -> String {
    file_name
        .to_str()
        .filter(|value| is_numeric_directory_name(value))
        .map(|value| format!("numeric-directory:{value}"))
        .unwrap_or_else(|| format!("source-entry:{ordinal}"))
}

fn is_numeric_directory_name(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn root_entry_sort_key(entry: &std::ffi::OsStr) -> (u8, u64, String) {
    let name = entry.to_string_lossy().into_owned();
    let numeric = name
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then(|| name.parse::<u64>().unwrap_or(u64::MAX));
    match numeric {
        Some(value) => (0, value, name),
        None => (1, 0, name),
    }
}

enum RootEntryRead {
    Complete(Vec<OsString>),
    CandidateLimitExceeded,
}

fn read_root_entries(
    root: &Dir,
    max_candidates: u64,
    cancellation_token: &dyn CancellationToken,
) -> std::result::Result<RootEntryRead, WalkFailure> {
    let entries = root.entries().map_err(|_| WalkFailure::SourceUnreadable)?;
    let mut result = Vec::new();
    for entry in entries {
        if cancellation_token.is_cancelled() {
            return Err(WalkFailure::Cancelled);
        }
        if u64::try_from(result.len()).unwrap_or(u64::MAX) >= max_candidates {
            return Ok(RootEntryRead::CandidateLimitExceeded);
        }
        result.push(
            entry
                .map_err(|_| WalkFailure::SourceUnreadable)?
                .file_name(),
        );
    }
    Ok(RootEntryRead::Complete(result))
}

fn mark_duplicate_content(candidates: &mut [ExternalImportCandidate]) {
    let mut fingerprints = BTreeSet::new();
    for candidate in candidates {
        if candidate.preview_status == ExternalImportCandidateStatus::Ready
            && !fingerprints.insert(candidate.content_fingerprint.clone())
        {
            candidate.preview_status = ExternalImportCandidateStatus::DuplicateInBatch;
            candidate.conflict_kind = ExternalImportConflictKind::ContentDuplicate;
        }
    }
}

fn mark_duplicate_display_names(candidates: &mut [ExternalImportCandidate]) {
    let mut display_names = BTreeSet::new();
    for candidate in candidates {
        if candidate.preview_status != ExternalImportCandidateStatus::Ready {
            continue;
        }
        let Some(display_name) = candidate.metadata_hint.display_name.as_deref() else {
            continue;
        };
        let normalized = display_name.trim().to_lowercase();
        if !normalized.is_empty() && !display_names.insert(normalized) {
            candidate.preview_status = ExternalImportCandidateStatus::NameCollision;
            candidate.conflict_kind = ExternalImportConflictKind::NameCollision;
        }
    }
}

#[derive(Clone, Copy)]
struct ContentLimits {
    max_files: u64,
    max_single_file_bytes: u64,
    max_total_bytes: u64,
    max_directory_depth: u32,
    max_directory_entries: u64,
}

fn remaining_content_limits(
    budget: &ExternalImportResourceBudget,
    observed: ExternalImportResourceUsage,
) -> ContentLimits {
    ContentLimits {
        max_files: budget
            .materialization
            .max_files
            .min(budget.max_total_files.saturating_sub(observed.file_count)),
        max_single_file_bytes: budget.materialization.max_single_file_bytes,
        max_total_bytes: budget
            .materialization
            .max_total_bytes
            .min(
                budget
                    .max_total_source_bytes
                    .saturating_sub(observed.source_bytes),
            )
            .min(
                budget
                    .max_total_materialization_bytes
                    .saturating_sub(observed.materialization_bytes),
            ),
        max_directory_depth: budget.materialization.max_directory_depth,
        max_directory_entries: budget
            .materialization
            .max_files
            .min(budget.max_total_files.saturating_sub(observed.file_count))
            .saturating_mul(2)
            .saturating_add(u64::from(budget.materialization.max_directory_depth))
            .max(1),
    }
}

fn add_observed_usage(
    observed: &mut ExternalImportResourceUsage,
    usage: ExternalImportResourceUsage,
    budget: &ExternalImportResourceBudget,
) -> bool {
    let Some(next) = observed
        .file_count
        .checked_add(usage.file_count)
        .zip(observed.source_bytes.checked_add(usage.source_bytes))
        .zip(
            observed
                .materialization_bytes
                .checked_add(usage.materialization_bytes),
        )
        .map(
            |((file_count, source_bytes), materialization_bytes)| ExternalImportResourceUsage {
                file_count,
                source_bytes,
                materialization_bytes,
            },
        )
    else {
        return false;
    };
    if !budget.permits(next) {
        return false;
    }
    *observed = next;
    true
}

struct ContentScan {
    usage: ExternalImportResourceUsage,
    content_fingerprint: String,
}

#[derive(Clone)]
pub(crate) struct ValidatedContentFile {
    pub source_segments: Vec<OsString>,
    /// A normalized, case-insensitive-safe archive entry path.
    pub archive_path: String,
    pub size_bytes: u64,
    pub content_hash: [u8; 32],
}

pub(crate) struct ValidatedContent {
    pub source_directory: Dir,
    pub usage: ExternalImportResourceUsage,
    pub content_fingerprint: String,
    pub files: Vec<ValidatedContentFile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentValidationError {
    Cancelled,
    Rejected,
}

enum ContentScanOutcome {
    Complete(ContentScan),
    Rejected {
        issue: CandidateIssue,
        usage: ExternalImportResourceUsage,
    },
    Cancelled,
}

#[derive(Clone, Copy)]
enum CandidateIssue {
    StructureInvalid,
    ResourceLimitExceeded,
    SourceUnreadable,
}

impl CandidateIssue {
    fn into_status(self) -> ExternalImportCandidateStatus {
        match self {
            Self::StructureInvalid => ExternalImportCandidateStatus::StructureInvalid,
            Self::ResourceLimitExceeded => ExternalImportCandidateStatus::ResourceLimitExceeded,
            Self::SourceUnreadable => ExternalImportCandidateStatus::SourceUnreadable,
        }
    }
}

enum WalkFailure {
    Cancelled,
    StructureInvalid,
    ResourceLimitExceeded,
    SourceUnreadable,
}

impl From<WalkFailure> for CandidateIssue {
    fn from(value: WalkFailure) -> Self {
        match value {
            WalkFailure::StructureInvalid => Self::StructureInvalid,
            WalkFailure::ResourceLimitExceeded => Self::ResourceLimitExceeded,
            WalkFailure::SourceUnreadable => Self::SourceUnreadable,
            WalkFailure::Cancelled => Self::SourceUnreadable,
        }
    }
}

fn scan_content(
    files_directory: &Dir,
    limits: ContentLimits,
    cancellation_token: &dyn CancellationToken,
) -> ContentScanOutcome {
    if limits.max_files == 0 || limits.max_total_bytes == 0 {
        return ContentScanOutcome::Rejected {
            issue: CandidateIssue::ResourceLimitExceeded,
            usage: ExternalImportResourceUsage::default(),
        };
    }

    let mut walker = ContentWalker::new(limits, cancellation_token);
    match walker.walk(files_directory, &[], &[], 0) {
        Ok(()) => ContentScanOutcome::Complete(ContentScan {
            usage: walker.usage,
            content_fingerprint: walker.content_fingerprint(),
        }),
        Err(WalkFailure::Cancelled) => ContentScanOutcome::Cancelled,
        Err(error) => ContentScanOutcome::Rejected {
            issue: error.into(),
            usage: walker.usage,
        },
    }
}

/// Reuses the scan walker's hostile-filesystem validation for one selected candidate. The
/// materializer must compare its returned fingerprint with the durable preview before writing.
pub(crate) fn validate_materialization_content(
    files_directory: Dir,
    resource_budget: &ExternalImportResourceBudget,
    cancellation_token: &dyn CancellationToken,
) -> Result<ValidatedContent, ContentValidationError> {
    let limits = remaining_content_limits(resource_budget, ExternalImportResourceUsage::default());
    if limits.max_files == 0 || limits.max_total_bytes == 0 {
        return Err(ContentValidationError::Rejected);
    }

    let mut walker = ContentWalker::new(limits, cancellation_token);
    match walker.walk(&files_directory, &[], &[], 0) {
        Ok(()) => Ok(walker.into_validated_content(files_directory)),
        Err(WalkFailure::Cancelled) => Err(ContentValidationError::Cancelled),
        Err(_) => Err(ContentValidationError::Rejected),
    }
}

struct ContentWalker<'a> {
    limits: ContentLimits,
    cancellation_token: &'a dyn CancellationToken,
    usage: ExternalImportResourceUsage,
    seen_path_keys: BTreeSet<String>,
    directory_entries_seen: u64,
    files: BTreeMap<String, FileDigest>,
}

struct FileDigest {
    source_segments: Vec<OsString>,
    size_bytes: u64,
    content_hash: [u8; 32],
}

impl<'a> ContentWalker<'a> {
    fn new(limits: ContentLimits, cancellation_token: &'a dyn CancellationToken) -> Self {
        Self {
            limits,
            cancellation_token,
            usage: ExternalImportResourceUsage::default(),
            seen_path_keys: BTreeSet::new(),
            directory_entries_seen: 0,
            files: BTreeMap::new(),
        }
    }

    fn walk(
        &mut self,
        directory: &Dir,
        relative_segments: &[String],
        source_segments: &[OsString],
        depth: u32,
    ) -> std::result::Result<(), WalkFailure> {
        self.ensure_not_cancelled()?;
        let mut entries = self.read_normalized_entries(directory)?;
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        for (segment, source_name) in entries {
            self.ensure_not_cancelled()?;
            let mut child_segments = relative_segments.to_vec();
            child_segments.push(segment);
            let mut child_source_segments = source_segments.to_vec();
            child_source_segments.push(source_name.clone());
            let path_key = child_segments.join("/");
            if !self.seen_path_keys.insert(path_key.clone()) {
                return Err(WalkFailure::StructureInvalid);
            }

            let metadata = directory
                .symlink_metadata(&source_name)
                .map_err(|_| WalkFailure::SourceUnreadable)?;
            if is_link_or_reparse(&metadata) {
                return Err(WalkFailure::StructureInvalid);
            }
            if metadata.is_dir() {
                let child_depth = depth
                    .checked_add(1)
                    .ok_or(WalkFailure::ResourceLimitExceeded)?;
                if child_depth > self.limits.max_directory_depth {
                    return Err(WalkFailure::ResourceLimitExceeded);
                }
                let child = open_child_directory_nofollow(
                    directory,
                    &source_name,
                    "external import content directory",
                )
                .map_err(|_| WalkFailure::SourceUnreadable)?;
                self.walk(&child, &child_segments, &child_source_segments, child_depth)?;
            } else if metadata.is_file() {
                self.add_file(
                    directory,
                    &source_name,
                    path_key,
                    child_source_segments,
                    metadata,
                )?;
            } else {
                return Err(WalkFailure::StructureInvalid);
            }
        }
        Ok(())
    }

    fn add_file(
        &mut self,
        directory: &Dir,
        source_name: &OsStr,
        path_key: String,
        source_segments: Vec<OsString>,
        metadata: Metadata,
    ) -> std::result::Result<(), WalkFailure> {
        let size_bytes = metadata.len();
        if size_bytes > self.limits.max_single_file_bytes {
            return Err(WalkFailure::ResourceLimitExceeded);
        }
        let next_file_count = self
            .usage
            .file_count
            .checked_add(1)
            .ok_or(WalkFailure::ResourceLimitExceeded)?;
        let next_source_bytes = self
            .usage
            .source_bytes
            .checked_add(size_bytes)
            .ok_or(WalkFailure::ResourceLimitExceeded)?;
        let next_materialization_bytes = self
            .usage
            .materialization_bytes
            .checked_add(size_bytes)
            .ok_or(WalkFailure::ResourceLimitExceeded)?;
        if next_file_count > self.limits.max_files
            || next_source_bytes > self.limits.max_total_bytes
            || next_materialization_bytes > self.limits.max_total_bytes
        {
            return Err(WalkFailure::ResourceLimitExceeded);
        }
        self.usage = ExternalImportResourceUsage {
            file_count: next_file_count,
            source_bytes: next_source_bytes,
            materialization_bytes: next_materialization_bytes,
        };
        let content_hash =
            hash_regular_file(directory, source_name, &metadata, self.cancellation_token)?;
        self.files.insert(
            path_key,
            FileDigest {
                source_segments,
                size_bytes,
                content_hash,
            },
        );
        Ok(())
    }

    fn content_fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"hmm.external-import.content-fingerprint.v1");
        for (path_key, digest) in &self.files {
            hasher.update((path_key.len() as u64).to_be_bytes());
            hasher.update(path_key.as_bytes());
            hasher.update(digest.size_bytes.to_be_bytes());
            hasher.update(digest.content_hash);
        }
        format!("sha256:{}", hex_encode(&hasher.finalize()))
    }

    fn into_validated_content(self, source_directory: Dir) -> ValidatedContent {
        ValidatedContent {
            usage: self.usage,
            content_fingerprint: self.content_fingerprint(),
            source_directory,
            files: self
                .files
                .into_iter()
                .map(|(archive_path, file)| ValidatedContentFile {
                    source_segments: file.source_segments,
                    archive_path,
                    size_bytes: file.size_bytes,
                    content_hash: file.content_hash,
                })
                .collect(),
        }
    }

    fn ensure_not_cancelled(&self) -> std::result::Result<(), WalkFailure> {
        if self.cancellation_token.is_cancelled() {
            Err(WalkFailure::Cancelled)
        } else {
            Ok(())
        }
    }

    fn read_normalized_entries(
        &mut self,
        directory: &Dir,
    ) -> std::result::Result<Vec<(String, OsString)>, WalkFailure> {
        let entries = directory
            .entries()
            .map_err(|_| WalkFailure::SourceUnreadable)?;
        let mut normalized = Vec::new();
        for entry in entries {
            self.ensure_not_cancelled()?;
            let entry = entry.map_err(|_| WalkFailure::SourceUnreadable)?;
            self.directory_entries_seen = self
                .directory_entries_seen
                .checked_add(1)
                .ok_or(WalkFailure::ResourceLimitExceeded)?;
            if self.directory_entries_seen > self.limits.max_directory_entries {
                return Err(WalkFailure::ResourceLimitExceeded);
            }
            let source_name = entry.file_name();
            normalized.push((normalized_path_segment(&source_name)?, source_name));
        }
        Ok(normalized)
    }
}

fn normalized_path_segment(value: &OsStr) -> std::result::Result<String, WalkFailure> {
    let value = value.to_str().ok_or(WalkFailure::StructureInvalid)?;
    if unsafe_path_segment(value) {
        return Err(WalkFailure::StructureInvalid);
    }
    let normalized = value
        .nfkc()
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if unsafe_path_segment(&normalized) {
        return Err(WalkFailure::StructureInvalid);
    }
    Ok(normalized)
}

fn unsafe_path_segment(value: &str) -> bool {
    value.is_empty()
        || value == "."
        || value == ".."
        || value.ends_with([' ', '.'])
        || value
            .chars()
            .any(|character| character.is_control() || matches!(character, ':' | '/' | '\\'))
        || is_windows_reserved_name(value)
}

fn is_windows_reserved_name(value: &str) -> bool {
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .and_then(|suffix| suffix.parse::<u8>().ok())
            .is_some_and(|number| (1..=9).contains(&number))
}

fn hash_regular_file(
    directory: &Dir,
    source_name: &OsStr,
    metadata: &Metadata,
    cancellation_token: &dyn CancellationToken,
) -> std::result::Result<[u8; 32], WalkFailure> {
    let initial_size = metadata.len();
    let mut file =
        open_regular_file_nofollow(directory, source_name, "external import content file")
            .map_err(|_| WalkFailure::SourceUnreadable)?;
    let opened = file.metadata().map_err(|_| WalkFailure::SourceUnreadable)?;
    if !opened.is_file() || is_link_or_reparse(&opened) || opened.len() != initial_size {
        return Err(WalkFailure::SourceUnreadable);
    }
    let initial_modified = opened
        .modified()
        .map_err(|_| WalkFailure::SourceUnreadable)?;
    let mut reader: Take<&mut File> = (&mut file).take(initial_size.saturating_add(1));
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; FINGERPRINT_READ_BUFFER_BYTES];
    let mut bytes_read = 0_u64;
    loop {
        if cancellation_token.is_cancelled() {
            return Err(WalkFailure::Cancelled);
        }
        let count = reader
            .read(&mut buffer)
            .map_err(|_| WalkFailure::SourceUnreadable)?;
        if count == 0 {
            break;
        }
        bytes_read = bytes_read
            .checked_add(count as u64)
            .ok_or(WalkFailure::SourceUnreadable)?;
        if bytes_read > initial_size {
            return Err(WalkFailure::SourceUnreadable);
        }
        hasher.update(&buffer[..count]);
    }
    if bytes_read != initial_size {
        return Err(WalkFailure::SourceUnreadable);
    }
    let after = file.metadata().map_err(|_| WalkFailure::SourceUnreadable)?;
    if is_link_or_reparse(&after)
        || !after.is_file()
        || after.len() != initial_size
        || after
            .modified()
            .map_err(|_| WalkFailure::SourceUnreadable)?
            != initial_modified
    {
        return Err(WalkFailure::SourceUnreadable);
    }
    let digest = hasher.finalize();
    let mut content_hash = [0_u8; 32];
    content_hash.copy_from_slice(&digest);
    Ok(content_hash)
}

#[derive(Clone, Copy)]
enum MetadataField {
    ModuleName,
    Name,
    Author,
    Version,
    ModType,
}

#[derive(Default)]
struct ParsedMetadata {
    module_name: Option<String>,
    name: Option<String>,
    author: Option<String>,
    version: Option<String>,
    mod_type: Option<String>,
}

impl ParsedMetadata {
    fn set_if_missing(&mut self, field: MetadataField, value: String) {
        match field {
            MetadataField::ModuleName if self.module_name.is_none() => {
                self.module_name = Some(value)
            }
            MetadataField::Name if self.name.is_none() => self.name = Some(value),
            MetadataField::Author if self.author.is_none() => self.author = Some(value),
            MetadataField::Version if self.version.is_none() => self.version = Some(value),
            MetadataField::ModType if self.mod_type.is_none() => self.mod_type = Some(value),
            _ => {}
        }
    }

    fn into_hint(self) -> ExternalImportMetadataHint {
        ExternalImportMetadataHint {
            display_name: self.module_name.or(self.name),
            author: self.author,
            version: self.version,
            source_mod_type: self.mod_type,
        }
    }
}

#[derive(Clone, Copy)]
enum MetadataParseError {
    Invalid,
}

fn parse_info_xml(
    file: &mut File,
) -> std::result::Result<ExternalImportMetadataHint, MetadataParseError> {
    let xml = read_bounded_xml(file)?;
    let mut reader = Reader::from_reader(xml.as_slice());
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut depth = 0_usize;
    let mut field_count = 0_usize;
    let mut total_text_chars = 0_usize;
    let mut field_stack: Vec<Option<MetadataField>> = Vec::new();
    let mut metadata = ParsedMetadata::default();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                field_count = field_count
                    .checked_add(1)
                    .ok_or(MetadataParseError::Invalid)?;
                depth = depth.checked_add(1).ok_or(MetadataParseError::Invalid)?;
                if field_count > INFO_XML_MAX_FIELDS || depth > INFO_XML_MAX_DEPTH {
                    return Err(MetadataParseError::Invalid);
                }
                field_stack.push(metadata_field(event.local_name().as_ref()));
            }
            Ok(Event::Empty(event)) => {
                field_count = field_count
                    .checked_add(1)
                    .ok_or(MetadataParseError::Invalid)?;
                if field_count > INFO_XML_MAX_FIELDS {
                    return Err(MetadataParseError::Invalid);
                }
                let _ = metadata_field(event.local_name().as_ref());
            }
            Ok(Event::End(_)) => {
                if depth == 0 || field_stack.pop().is_none() {
                    return Err(MetadataParseError::Invalid);
                }
                depth -= 1;
            }
            Ok(Event::Text(event)) => {
                let decoded = event.decode().map_err(|_| MetadataParseError::Invalid)?;
                let text = sanitize_xml_text(&decoded, &mut total_text_chars)?;
                if let (Some(Some(field)), Some(text)) = (field_stack.last(), text) {
                    metadata.set_if_missing(*field, text);
                }
            }
            Ok(Event::DocType(_)) | Ok(Event::GeneralRef(_)) | Ok(Event::CData(_)) => {
                return Err(MetadataParseError::Invalid)
            }
            Ok(Event::Comment(_)) | Ok(Event::Decl(_)) | Ok(Event::PI(_)) => {}
            Ok(Event::Eof) => {
                if depth != 0 {
                    return Err(MetadataParseError::Invalid);
                }
                return Ok(metadata.into_hint());
            }
            Err(_) => return Err(MetadataParseError::Invalid),
        }
        buffer.clear();
    }
}

/// Rechecks the only XML-derived data that can affect an external-import selection. A selected
/// metadata-invalid candidate remains eligible only while its XML is still invalid and the caller
/// has already recorded the explicit ignore decision in the sealed selection.
pub(crate) fn metadata_matches_preview(
    info_xml: &mut File,
    preview_status: ExternalImportCandidateStatus,
    expected_metadata: &ExternalImportMetadataHint,
) -> bool {
    match parse_info_xml(info_xml) {
        Ok(metadata) => {
            preview_status != ExternalImportCandidateStatus::MetadataInvalid
                && metadata == *expected_metadata
        }
        Err(_) => preview_status == ExternalImportCandidateStatus::MetadataInvalid,
    }
}

fn read_bounded_xml(file: &mut File) -> std::result::Result<Vec<u8>, MetadataParseError> {
    let before = file.metadata().map_err(|_| MetadataParseError::Invalid)?;
    if !before.is_file() || is_link_or_reparse(&before) || before.len() > INFO_XML_MAX_BYTES {
        return Err(MetadataParseError::Invalid);
    }
    let before_modified = before.modified().map_err(|_| MetadataParseError::Invalid)?;
    let mut reader = (&mut *file).take(INFO_XML_MAX_BYTES.saturating_add(1));
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| MetadataParseError::Invalid)?;
    if bytes.len() as u64 > INFO_XML_MAX_BYTES {
        return Err(MetadataParseError::Invalid);
    }
    let after = file.metadata().map_err(|_| MetadataParseError::Invalid)?;
    if !after.is_file()
        || is_link_or_reparse(&after)
        || after.len() != before.len()
        || after.modified().map_err(|_| MetadataParseError::Invalid)? != before_modified
    {
        return Err(MetadataParseError::Invalid);
    }
    Ok(bytes)
}

fn metadata_field(name: &[u8]) -> Option<MetadataField> {
    match name {
        b"moduleName" | b"modulename" => Some(MetadataField::ModuleName),
        b"name" => Some(MetadataField::Name),
        b"author" => Some(MetadataField::Author),
        b"version" => Some(MetadataField::Version),
        b"modType" | b"modtype" => Some(MetadataField::ModType),
        _ => None,
    }
}

fn sanitize_xml_text(
    value: &str,
    total_text_chars: &mut usize,
) -> std::result::Result<Option<String>, MetadataParseError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().any(char::is_control) {
        return Err(MetadataParseError::Invalid);
    }
    let char_count = trimmed.chars().count();
    if char_count > INFO_XML_MAX_FIELD_CHARS {
        return Err(MetadataParseError::Invalid);
    }
    *total_text_chars = total_text_chars
        .checked_add(char_count)
        .ok_or(MetadataParseError::Invalid)?;
    if *total_text_chars > INFO_XML_MAX_TEXT_CHARS {
        return Err(MetadataParseError::Invalid);
    }
    Ok(Some(trimmed.to_owned()))
}

fn ensure_not_cancelled(cancellation_token: &dyn CancellationToken) -> Result<()> {
    if cancellation_token.is_cancelled() {
        bail!("external import scan cancelled");
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        ExternalImportAdapterId, ExternalImportBatch, ExternalImportBatchImportStatus,
        ExternalImportScanStatus,
    };
    use hmm_ports::NeverCancelled;
    use std::fs;
    use std::path::Path;

    #[test]
    fn scanner_discovers_direct_numeric_directories_and_never_serializes_source_paths() {
        let fixture = tempfile::tempdir().expect("fixture root");
        write_candidate(
            fixture.path(),
            "101",
            "<info><moduleName>Fixture Mod</moduleName><author>Author</author><version>1.0</version><modType>skin</modType></info>",
            &[("nativePC/fixture.bin", b"fixture-bytes")],
        );
        fs::create_dir_all(fixture.path().join("not-a-candidate"))
            .expect("write non-numeric source entry");

        let result = scan_fixture(fixture.path(), ExternalImportResourceBudget::default());

        assert_eq!(result.candidates.len(), 2);
        let ready = result
            .candidates
            .iter()
            .find(|candidate| candidate.preview_status == ExternalImportCandidateStatus::Ready)
            .expect("numeric candidate discovered");
        assert_eq!(
            ready.metadata_hint.display_name.as_deref(),
            Some("Fixture Mod")
        );
        assert_eq!(ready.metadata_hint.author.as_deref(), Some("Author"));
        assert_eq!(ready.metadata_hint.version.as_deref(), Some("1.0"));
        assert_eq!(ready.metadata_hint.source_mod_type.as_deref(), Some("skin"));
        assert!(ready.content_fingerprint.starts_with("sha256:"));
        let invalid = result
            .candidates
            .iter()
            .find(|candidate| {
                candidate.preview_status == ExternalImportCandidateStatus::StructureInvalid
            })
            .expect("non-numeric directory remains visible");
        assert_eq!(invalid.conflict_kind, ExternalImportConflictKind::None);

        let serialized = serde_json::to_string(&result.candidates).expect("serialize preview");
        assert!(!serialized.contains(fixture.path().to_string_lossy().as_ref()));
        assert!(!serialized.contains("nativePC/fixture.bin"));
    }

    #[test]
    fn scanner_marks_dtd_metadata_invalid_after_safe_content_scan() {
        let fixture = tempfile::tempdir().expect("fixture root");
        write_candidate(
            fixture.path(),
            "202",
            "<!DOCTYPE info [<!ENTITY xxe SYSTEM 'file:///forbidden'>]><info><moduleName>&xxe;</moduleName></info>",
            &[("file.bin", b"safe-content")],
        );

        let result = scan_fixture(fixture.path(), ExternalImportResourceBudget::default());
        let candidate = result.candidates.first().expect("candidate");

        assert_eq!(
            candidate.preview_status,
            ExternalImportCandidateStatus::MetadataInvalid
        );
        assert_ne!(
            candidate.content_fingerprint,
            UNAVAILABLE_CONTENT_FINGERPRINT
        );
    }

    #[test]
    fn scanner_marks_later_same_name_different_content_as_a_collision() {
        let fixture = tempfile::tempdir().expect("fixture root");
        for (id, bytes) in [
            ("211", b"first-content".as_slice()),
            ("212", b"second-content".as_slice()),
        ] {
            write_candidate(
                fixture.path(),
                id,
                "<info><moduleName>Shared Name</moduleName></info>",
                &[("file.bin", bytes)],
            );
        }

        let result = scan_fixture(fixture.path(), ExternalImportResourceBudget::default());

        assert_eq!(
            result.candidates[0].preview_status,
            ExternalImportCandidateStatus::Ready
        );
        assert_eq!(
            result.candidates[1].preview_status,
            ExternalImportCandidateStatus::NameCollision
        );
        assert_eq!(
            result.candidates[1].conflict_kind,
            ExternalImportConflictKind::NameCollision
        );
    }

    #[test]
    fn scanner_keeps_structure_and_resource_failures_visible() {
        let fixture = tempfile::tempdir().expect("fixture root");
        let missing_files = fixture.path().join("301");
        fs::create_dir_all(&missing_files).expect("create malformed candidate");
        fs::write(missing_files.join("info.xml"), "<info />").expect("write xml");
        write_candidate(
            fixture.path(),
            "302",
            "<info><name>Too many files</name></info>",
            &[("one.bin", b"one"), ("two.bin", b"two")],
        );
        let budget = ExternalImportResourceBudget {
            materialization: hmm_core::ExternalImportMaterializationBudget {
                max_files: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        let result = scan_fixture(fixture.path(), budget);

        assert!(result
            .candidates
            .iter()
            .any(|candidate| candidate.preview_status
                == ExternalImportCandidateStatus::StructureInvalid));
        assert!(result.candidates.iter().any(|candidate| {
            candidate.preview_status == ExternalImportCandidateStatus::ResourceLimitExceeded
        }));
    }

    #[test]
    fn scanner_rejects_linked_files_and_metadata_without_following_them() {
        let fixture = tempfile::tempdir().expect("fixture root");
        let outside = tempfile::tempdir().expect("outside root");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, b"outside remains untouched").expect("write outside sentinel");
        write_candidate(
            fixture.path(),
            "320",
            "<info><name>Linked files</name></info>",
            &[("safe.bin", b"safe")],
        );
        let files_directory = fixture.path().join("320").join("files");
        fs::remove_dir_all(&files_directory).expect("remove fixture files directory");
        if !try_create_directory_link(outside.path(), &files_directory) {
            return;
        }

        let files_result = scan_fixture(fixture.path(), ExternalImportResourceBudget::default());

        assert_eq!(
            files_result.candidates[0].preview_status,
            ExternalImportCandidateStatus::StructureInvalid
        );
        assert_eq!(
            fs::read(&sentinel).expect("read outside sentinel"),
            b"outside remains untouched"
        );
        remove_directory_link(&files_directory);

        let metadata_fixture = tempfile::tempdir().expect("metadata fixture root");
        write_candidate(
            metadata_fixture.path(),
            "321",
            "<info><name>Linked metadata</name></info>",
            &[("safe.bin", b"safe")],
        );
        let metadata_path = metadata_fixture.path().join("321").join("info.xml");
        fs::remove_file(&metadata_path).expect("remove fixture metadata");
        if !try_create_directory_link(outside.path(), &metadata_path) {
            return;
        }

        let metadata_result = scan_fixture(
            metadata_fixture.path(),
            ExternalImportResourceBudget::default(),
        );

        assert_eq!(
            metadata_result.candidates[0].preview_status,
            ExternalImportCandidateStatus::StructureInvalid
        );
        assert_eq!(
            fs::read(&sentinel).expect("read outside sentinel"),
            b"outside remains untouched"
        );
        remove_directory_link(&metadata_path);
    }

    #[test]
    fn scanner_rejects_linked_content_and_nfkc_dangerous_segments() {
        let linked_fixture = tempfile::tempdir().expect("linked fixture root");
        let outside = tempfile::tempdir().expect("outside root");
        let sentinel = outside.path().join("sentinel.txt");
        fs::write(&sentinel, b"outside remains untouched").expect("write outside sentinel");
        write_candidate(
            linked_fixture.path(),
            "322",
            "<info><name>Linked content</name></info>",
            &[("safe.bin", b"safe")],
        );
        let linked_content = linked_fixture
            .path()
            .join("322")
            .join("files")
            .join("escape");
        if !try_create_directory_link(outside.path(), &linked_content) {
            return;
        }

        let linked_result = scan_fixture(
            linked_fixture.path(),
            ExternalImportResourceBudget::default(),
        );

        assert_eq!(
            linked_result.candidates[0].preview_status,
            ExternalImportCandidateStatus::StructureInvalid
        );
        assert_eq!(
            fs::read(&sentinel).expect("read outside sentinel"),
            b"outside remains untouched"
        );
        remove_directory_link(&linked_content);

        let unicode_fixture = tempfile::tempdir().expect("unicode fixture root");
        let dangerous_name = "safe\u{ff0f}segment.bin";
        write_candidate(
            unicode_fixture.path(),
            "323",
            "<info><name>Unicode</name></info>",
            &[(dangerous_name, b"safe")],
        );

        let unicode_result = scan_fixture(
            unicode_fixture.path(),
            ExternalImportResourceBudget::default(),
        );

        assert_eq!(
            unicode_result.candidates[0].preview_status,
            ExternalImportCandidateStatus::StructureInvalid
        );
    }

    #[test]
    fn scanner_bounds_empty_directory_fanout_before_collecting_all_entries() {
        let fixture = tempfile::tempdir().expect("fixture root");
        write_candidate(
            fixture.path(),
            "324",
            "<info><name>Fanout</name></info>",
            &[],
        );
        let files = fixture.path().join("324").join("files");
        for index in 0..4 {
            fs::create_dir(files.join(format!("empty-{index}"))).expect("create empty directory");
        }
        let budget = ExternalImportResourceBudget {
            materialization: hmm_core::ExternalImportMaterializationBudget {
                max_files: 1,
                max_directory_depth: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        let result = scan_fixture(fixture.path(), budget);

        assert_eq!(
            result.candidates[0].preview_status,
            ExternalImportCandidateStatus::ResourceLimitExceeded
        );
    }

    #[test]
    fn root_candidate_budget_returns_one_safe_resource_rejection_without_enumerating_overflow() {
        let fixture = tempfile::tempdir().expect("fixture root");
        for directory in ["401", "402", "403"] {
            fs::create_dir_all(fixture.path().join(directory))
                .expect("create root entry beyond candidate budget");
        }
        let budget = ExternalImportResourceBudget {
            max_total_candidates: 2,
            ..Default::default()
        };

        let result = scan_fixture(fixture.path(), budget);

        assert_eq!(result.candidates.len(), 1);
        assert_eq!(
            result.candidates[0].preview_status,
            ExternalImportCandidateStatus::ResourceLimitExceeded
        );
        assert_eq!(
            result.candidates[0].resource_usage,
            ExternalImportResourceUsage::default()
        );
    }

    #[test]
    fn content_fingerprint_is_independent_of_source_root() {
        let fixture_one = tempfile::tempdir().expect("first fixture root");
        let fixture_two = tempfile::tempdir().expect("second fixture root");
        for fixture in [&fixture_one, &fixture_two] {
            write_candidate(
                fixture.path(),
                "401",
                "<info><name>Same</name></info>",
                &[("nested/file.bin", b"same-content")],
            );
        }
        let app_data = tempfile::tempdir().expect("app data");
        let registry = Arc::new(
            HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("source registry"),
        );
        let scanner = HuntingBoxDirectoryScanner::new(Arc::clone(&registry));

        let first = scan_with(
            &scanner,
            &registry,
            fixture_one.path(),
            ExternalImportResourceBudget::default(),
        );
        let second = scan_with(
            &scanner,
            &registry,
            fixture_two.path(),
            ExternalImportResourceBudget::default(),
        );

        assert_eq!(
            first.candidates[0].content_fingerprint,
            second.candidates[0].content_fingerprint
        );
        assert_ne!(
            first.candidates[0].source_item_key_hash, second.candidates[0].source_item_key_hash,
            "source item handles remain scoped to their keyed source identity"
        );
    }

    #[test]
    fn cancelled_scan_does_not_write_or_mutate_the_source_fixture() {
        let fixture = tempfile::tempdir().expect("fixture root");
        let source_file = fixture.path().join("501").join("files").join("source.bin");
        write_candidate(
            fixture.path(),
            "501",
            "<info><name>Cancelled</name></info>",
            &[("source.bin", b"source-bytes")],
        );
        let before = fs::read(&source_file).expect("read fixture before scan");
        let app_data = tempfile::tempdir().expect("app data");
        let registry = Arc::new(
            HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("source registry"),
        );
        let source = registry
            .register_directory(fixture.path().to_path_buf())
            .expect("register source");
        let scanner = HuntingBoxDirectoryScanner::new(registry);
        let batch = batch_for(&source, "batch-cancelled");
        let cancellation = AlwaysCancelled;

        let result = scanner.scan(ExternalImportScanRequest {
            source: &source,
            batch: &batch,
            resource_budget: &ExternalImportResourceBudget::default(),
            cancellation_token: &cancellation,
        });

        assert!(
            result.is_err(),
            "cancelled scan is not reported as completed"
        );
        assert_eq!(
            fs::read(&source_file).expect("read fixture after scan"),
            before
        );
        assert!(!fixture.path().join("501").join("new-file").exists());
    }

    struct AlwaysCancelled;

    impl CancellationToken for AlwaysCancelled {
        fn is_cancelled(&self) -> bool {
            true
        }
    }

    fn scan_fixture(root: &Path, budget: ExternalImportResourceBudget) -> ExternalImportScanResult {
        let app_data = tempfile::tempdir().expect("app data");
        let registry = Arc::new(
            HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("source registry"),
        );
        let scanner = HuntingBoxDirectoryScanner::new(Arc::clone(&registry));
        scan_with(&scanner, &registry, root, budget)
    }

    fn scan_with(
        scanner: &HuntingBoxDirectoryScanner,
        registry: &Arc<HuntingBoxDirectorySourceRegistry>,
        root: &Path,
        budget: ExternalImportResourceBudget,
    ) -> ExternalImportScanResult {
        let source = registry
            .register_directory(root.to_path_buf())
            .expect("register source");
        let batch = batch_for(&source, "batch-fixture");
        scanner
            .scan(ExternalImportScanRequest {
                source: &source,
                batch: &batch,
                resource_budget: &budget,
                cancellation_token: &NeverCancelled,
            })
            .expect("scan fixture")
    }

    fn batch_for(source: &hmm_core::ExternalImportSource, batch_id: &str) -> ExternalImportBatch {
        ExternalImportBatch {
            batch_id: ExternalImportBatchId::new(batch_id),
            source_id: Some(source.source_id.clone()),
            adapter_id: ExternalImportAdapterId::new(HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID),
            source_fingerprint: "private-test-fingerprint".to_owned(),
            scan_status: ExternalImportScanStatus::Pending,
            import_status: ExternalImportBatchImportStatus::Pending,
            created_at_unix_millis: 1,
        }
    }

    fn write_candidate(root: &Path, id: &str, xml: &str, files: &[(&str, &[u8])]) {
        let candidate_root = root.join(id);
        let files_root = candidate_root.join("files");
        fs::create_dir_all(&files_root).expect("create fixture files directory");
        fs::write(candidate_root.join("info.xml"), xml).expect("write fixture xml");
        for (relative_path, bytes) in files {
            let path = files_root.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create fixture file parent");
            }
            fs::write(path, bytes).expect("write fixture content");
        }
    }

    #[cfg(unix)]
    fn try_create_directory_link(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn try_create_directory_link(target: &Path, link: &Path) -> bool {
        std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_str().expect("link path"),
                target.to_str().expect("target path"),
            ])
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(unix)]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).expect("remove directory symlink");
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).expect("remove directory junction");
    }
}
