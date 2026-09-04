use crate::game_directory_probe::directory_is_writable;
use hmm_ports::{
    validate_mod_storage_directory_shape, ModStorageDirectoryError, ModStorageDirectoryInspection,
    ModStorageDirectoryInspectionRequest, ModStorageDirectoryInspector, MOD_STORAGE_MARKER_NAME,
    MOD_STORAGE_MARKER_SCHEMA, MOD_STORAGE_SANDBOX_DIRECTORY,
};
use std::fs::{self, Metadata, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path};

/// File-system side of the Mod storage directory rules. Every verdict here is about the
/// directory the user pointed at; nothing below `sandboxes/` is inspected or touched.
pub struct FileSystemModStorageDirectoryInspector;

/// What a candidate directory currently contains, as far as the claim rules care.
enum DirectoryContents {
    Empty,
    /// Valid marker present; other entries are HMM's own or the user's business.
    Claimed,
    /// No marker, but nothing besides HMM's `sandboxes/` layout — a marker that was deleted
    /// by hand can be rewritten without adopting a foreign directory.
    LayoutOnly,
    Foreign,
}

impl ModStorageDirectoryInspector for FileSystemModStorageDirectoryInspector {
    fn inspect(
        &self,
        request: ModStorageDirectoryInspectionRequest<'_>,
    ) -> Result<ModStorageDirectoryInspection, ModStorageDirectoryError> {
        let path = request.path;
        validate_mod_storage_directory_shape(path)?;
        let parent = path
            .parent()
            .ok_or(ModStorageDirectoryError::FileSystemRoot)?;
        reject_link_or_reparse_chain(parent).map_err(|error| match error {
            ModStorageDirectoryError::Unavailable => ModStorageDirectoryError::ParentMissing,
            other => other,
        })?;
        if !parent.is_dir() {
            return Err(ModStorageDirectoryError::ParentMissing);
        }
        let exists = existing_real_directory(path)?;
        for root in request.exclusive_roots {
            if self.directories_overlap(path, root) {
                return Err(ModStorageDirectoryError::OverlapsGameRoot);
            }
        }
        // A current root that is gone (unplugged drive) cannot contain anything; a candidate
        // below it fails as `ParentMissing` on its own, so the overlap check only runs for a
        // present directory — otherwise the fail-closed overlap verdict would reject every path.
        if let Some(current_root) = request.current_root {
            if current_root.is_dir() && self.directories_overlap(path, current_root) {
                return Err(ModStorageDirectoryError::OverlapsCurrentRoot);
            }
        }
        let claimed = if exists {
            match classify_contents(path)? {
                DirectoryContents::Claimed => true,
                DirectoryContents::Empty | DirectoryContents::LayoutOnly => false,
                DirectoryContents::Foreign => {
                    return Err(ModStorageDirectoryError::MarkerRequired);
                }
            }
        } else {
            false
        };
        let probe_target = if exists { path } else { parent };
        if !directory_is_writable(probe_target) {
            return Err(ModStorageDirectoryError::NotWritable);
        }
        Ok(ModStorageDirectoryInspection { exists, claimed })
    }

    fn claim(&self, path: &Path) -> Result<(), ModStorageDirectoryError> {
        validate_mod_storage_directory_shape(path)?;
        let parent = path
            .parent()
            .ok_or(ModStorageDirectoryError::FileSystemRoot)?;
        reject_link_or_reparse_chain(parent).map_err(|error| match error {
            ModStorageDirectoryError::Unavailable => ModStorageDirectoryError::ParentMissing,
            other => other,
        })?;
        if !existing_real_directory(path)? {
            match fs::create_dir(path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    return Err(ModStorageDirectoryError::ParentMissing);
                }
                Err(_) => return Err(ModStorageDirectoryError::NotWritable),
            }
            if !existing_real_directory(path)? {
                return Err(ModStorageDirectoryError::NotDirectory);
            }
        }
        match classify_contents(path)? {
            DirectoryContents::Claimed => Ok(()),
            DirectoryContents::Empty | DirectoryContents::LayoutOnly => write_marker(path),
            DirectoryContents::Foreign => Err(ModStorageDirectoryError::MarkerRequired),
        }
    }

    fn verify_claimed(&self, path: &Path) -> Result<(), ModStorageDirectoryError> {
        validate_mod_storage_directory_shape(path)?;
        reject_link_or_reparse_chain(path)?;
        if !existing_real_directory(path)? {
            return Err(ModStorageDirectoryError::Unavailable);
        }
        match classify_contents(path)? {
            DirectoryContents::Claimed
            | DirectoryContents::LayoutOnly
            | DirectoryContents::Empty => Ok(()),
            DirectoryContents::Foreign => Err(ModStorageDirectoryError::MarkerRequired),
        }
    }

    fn sandbox_directory_has_entries(
        &self,
        storage_root: &Path,
    ) -> Result<bool, ModStorageDirectoryError> {
        match fs::read_dir(storage_root.join(MOD_STORAGE_SANDBOX_DIRECTORY)) {
            Ok(mut entries) => match entries.next() {
                None => Ok(false),
                Some(Ok(_)) => Ok(true),
                Some(Err(_)) => Err(ModStorageDirectoryError::Unavailable),
            },
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(_) => Err(ModStorageDirectoryError::Unavailable),
        }
    }

    fn directories_overlap(&self, left: &Path, right: &Path) -> bool {
        let (Some(left), Some(right)) = (canonical_anchor(left), canonical_anchor(right)) else {
            return true;
        };
        left.starts_with(&right) || right.starts_with(&left)
    }
}

/// `Ok(true)` for a real directory, `Ok(false)` when nothing exists at the path.
fn existing_real_directory(path: &Path) -> Result<bool, ModStorageDirectoryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if is_link_or_reparse(&metadata) {
                return Err(ModStorageDirectoryError::LinkRejected);
            }
            if !metadata.is_dir() {
                return Err(ModStorageDirectoryError::NotDirectory);
            }
            Ok(true)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err(ModStorageDirectoryError::Unavailable),
    }
}

fn classify_contents(path: &Path) -> Result<DirectoryContents, ModStorageDirectoryError> {
    let entries = fs::read_dir(path)
        .map_err(|_| ModStorageDirectoryError::Unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ModStorageDirectoryError::Unavailable)?;
    if entries.is_empty() {
        return Ok(DirectoryContents::Empty);
    }
    let has_marker = entries
        .iter()
        .any(|entry| entry.file_name() == MOD_STORAGE_MARKER_NAME);
    if has_marker {
        validate_marker(path)?;
        return Ok(DirectoryContents::Claimed);
    }
    let layout_only = entries.iter().all(|entry| {
        entry.file_name() == MOD_STORAGE_SANDBOX_DIRECTORY
            && fs::symlink_metadata(entry.path())
                .map(|metadata| metadata.is_dir() && !is_link_or_reparse(&metadata))
                .unwrap_or(false)
    });
    Ok(if layout_only {
        DirectoryContents::LayoutOnly
    } else {
        DirectoryContents::Foreign
    })
}

fn validate_marker(path: &Path) -> Result<(), ModStorageDirectoryError> {
    let marker_path = path.join(MOD_STORAGE_MARKER_NAME);
    let metadata =
        fs::symlink_metadata(&marker_path).map_err(|_| ModStorageDirectoryError::MarkerInvalid)?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(ModStorageDirectoryError::MarkerInvalid);
    }
    let file = fs::File::open(&marker_path).map_err(|_| ModStorageDirectoryError::MarkerInvalid)?;
    let mut contents = Vec::new();
    file.take((MOD_STORAGE_MARKER_SCHEMA.len() + 1) as u64)
        .read_to_end(&mut contents)
        .map_err(|_| ModStorageDirectoryError::MarkerInvalid)?;
    if contents == MOD_STORAGE_MARKER_SCHEMA.as_bytes() {
        Ok(())
    } else {
        Err(ModStorageDirectoryError::MarkerInvalid)
    }
}

fn write_marker(path: &Path) -> Result<(), ModStorageDirectoryError> {
    let mut marker = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.join(MOD_STORAGE_MARKER_NAME))
        .map_err(|_| ModStorageDirectoryError::NotWritable)?;
    marker
        .write_all(MOD_STORAGE_MARKER_SCHEMA.as_bytes())
        .and_then(|_| marker.sync_all())
        .map_err(|_| ModStorageDirectoryError::NotWritable)
}

/// Every existing ancestor (the path itself included) must be a plain directory. A missing
/// component maps to `Unavailable` so callers can decide whether absence is acceptable.
fn reject_link_or_reparse_chain(path: &Path) -> Result<(), ModStorageDirectoryError> {
    let mut ancestors = path.ancestors().collect::<Vec<_>>();
    ancestors.reverse();
    for ancestor in ancestors {
        if ancestor.as_os_str().is_empty()
            || ancestor
                .components()
                .all(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
        {
            continue;
        }
        let metadata =
            fs::symlink_metadata(ancestor).map_err(|_| ModStorageDirectoryError::Unavailable)?;
        if is_link_or_reparse(&metadata) {
            return Err(ModStorageDirectoryError::LinkRejected);
        }
    }
    Ok(())
}

/// Canonical form of the nearest existing ancestor plus the remaining lexical tail, so two
/// not-yet-created directories under the same parent still compare correctly.
fn canonical_anchor(path: &Path) -> Option<std::path::PathBuf> {
    let existing = path.ancestors().find(|ancestor| ancestor.exists())?;
    let canonical = existing.canonicalize().ok()?;
    let tail = path.strip_prefix(existing).ok()?;
    Some(normalize_case(canonical.join(tail)))
}

#[cfg(windows)]
fn normalize_case(path: std::path::PathBuf) -> std::path::PathBuf {
    std::path::PathBuf::from(path.to_string_lossy().to_ascii_lowercase())
}

#[cfg(not(windows))]
fn normalize_case(path: std::path::PathBuf) -> std::path::PathBuf {
    path
}

fn is_link_or_reparse(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn inspector() -> FileSystemModStorageDirectoryInspector {
        FileSystemModStorageDirectoryInspector
    }

    fn inspect(path: &Path) -> Result<ModStorageDirectoryInspection, ModStorageDirectoryError> {
        inspect_with_roots(path, &[])
    }

    fn inspect_with_roots(
        path: &Path,
        exclusive_roots: &[PathBuf],
    ) -> Result<ModStorageDirectoryInspection, ModStorageDirectoryError> {
        inspector().inspect(ModStorageDirectoryInspectionRequest {
            path,
            exclusive_roots,
            current_root: None,
        })
    }

    fn inspect_with_current_root(
        path: &Path,
        current_root: &Path,
    ) -> Result<ModStorageDirectoryInspection, ModStorageDirectoryError> {
        inspector().inspect(ModStorageDirectoryInspectionRequest {
            path,
            exclusive_roots: &[],
            current_root: Some(current_root),
        })
    }

    #[test]
    fn candidates_overlapping_the_current_root_are_rejected_unless_that_root_is_gone() {
        let temp = tempfile::tempdir().expect("temp");
        let current = temp.path().join("current");
        fs::create_dir_all(current.join(MOD_STORAGE_SANDBOX_DIRECTORY)).expect("layout");
        let sibling = temp.path().join("sibling");

        assert_eq!(
            inspect_with_current_root(&current, &current),
            Err(ModStorageDirectoryError::OverlapsCurrentRoot)
        );
        assert_eq!(
            inspect_with_current_root(
                &current.join(MOD_STORAGE_SANDBOX_DIRECTORY).join("nested"),
                &current
            ),
            Err(ModStorageDirectoryError::OverlapsCurrentRoot),
            "a root below the current sandboxes would show up as a package"
        );
        assert_eq!(
            inspect_with_current_root(temp.path(), &current),
            Err(ModStorageDirectoryError::OverlapsCurrentRoot),
            "a root containing the current root is rejected before the marker rule"
        );
        assert_eq!(
            inspect_with_current_root(&sibling, &current),
            Ok(ModStorageDirectoryInspection {
                exists: false,
                claimed: false,
            })
        );

        fs::remove_dir_all(&current).expect("remove current root");
        assert_eq!(
            inspect_with_current_root(&sibling, &current),
            Ok(ModStorageDirectoryInspection {
                exists: false,
                claimed: false,
            }),
            "an absent current root must not block every candidate"
        );
    }

    #[test]
    fn absent_directory_with_existing_parent_is_acceptable_and_unclaimed() {
        let temp = tempfile::tempdir().expect("temp");
        let candidate = temp.path().join("mods");

        assert_eq!(
            inspect(&candidate),
            Ok(ModStorageDirectoryInspection {
                exists: false,
                claimed: false,
            })
        );
        assert!(!candidate.exists(), "inspect must not create the directory");
    }

    #[test]
    fn absent_parent_is_rejected() {
        let temp = tempfile::tempdir().expect("temp");
        let candidate = temp.path().join("missing").join("mods");

        assert_eq!(
            inspect(&candidate),
            Err(ModStorageDirectoryError::ParentMissing)
        );
    }

    #[test]
    fn empty_directory_is_acceptable_and_unclaimed() {
        let temp = tempfile::tempdir().expect("temp");
        let candidate = temp.path().join("mods");
        fs::create_dir(&candidate).expect("create candidate");

        assert_eq!(
            inspect(&candidate),
            Ok(ModStorageDirectoryInspection {
                exists: true,
                claimed: false,
            })
        );
        assert_eq!(
            fs::read_dir(&candidate).expect("read").count(),
            0,
            "inspect must leave no probe residue"
        );
    }

    #[test]
    fn regular_file_is_not_a_directory() {
        let temp = tempfile::tempdir().expect("temp");
        let candidate = temp.path().join("mods");
        fs::write(&candidate, b"file").expect("write file");

        assert_eq!(
            inspect(&candidate),
            Err(ModStorageDirectoryError::NotDirectory)
        );
    }

    #[test]
    fn foreign_non_empty_directory_requires_marker() {
        let temp = tempfile::tempdir().expect("temp");
        let candidate = temp.path().join("mods");
        fs::create_dir(&candidate).expect("create candidate");
        fs::write(candidate.join("notes.txt"), b"user data").expect("write file");

        assert_eq!(
            inspect(&candidate),
            Err(ModStorageDirectoryError::MarkerRequired)
        );
        assert_eq!(
            inspector().claim(&candidate),
            Err(ModStorageDirectoryError::MarkerRequired)
        );
        assert!(!candidate.join(MOD_STORAGE_MARKER_NAME).exists());
    }

    #[test]
    fn layout_only_directory_can_be_reclaimed() {
        let temp = tempfile::tempdir().expect("temp");
        let candidate = temp.path().join("mods");
        fs::create_dir_all(candidate.join(MOD_STORAGE_SANDBOX_DIRECTORY).join("pkg-1"))
            .expect("create layout");

        assert_eq!(
            inspect(&candidate),
            Ok(ModStorageDirectoryInspection {
                exists: true,
                claimed: false,
            })
        );
        inspector()
            .claim(&candidate)
            .expect("claim layout-only directory");
        assert_eq!(
            fs::read(candidate.join(MOD_STORAGE_MARKER_NAME)).expect("read marker"),
            MOD_STORAGE_MARKER_SCHEMA.as_bytes()
        );
    }

    #[test]
    fn claim_creates_directory_and_writes_byte_exact_marker() {
        let temp = tempfile::tempdir().expect("temp");
        let candidate = temp.path().join("mods");

        inspector().claim(&candidate).expect("claim");

        let marker = fs::read(candidate.join(MOD_STORAGE_MARKER_NAME)).expect("read marker");
        assert_eq!(marker, MOD_STORAGE_MARKER_SCHEMA.as_bytes());
        assert_eq!(marker.len(), 45);
        assert_eq!(
            inspect(&candidate),
            Ok(ModStorageDirectoryInspection {
                exists: true,
                claimed: true,
            })
        );
        inspector()
            .claim(&candidate)
            .expect("claiming an already claimed directory is idempotent");
    }

    #[test]
    fn tampered_marker_is_rejected() {
        let temp = tempfile::tempdir().expect("temp");
        let candidate = temp.path().join("mods");
        fs::create_dir(&candidate).expect("create candidate");
        fs::write(
            candidate.join(MOD_STORAGE_MARKER_NAME),
            b"{\"kind\":\"hmm.mod-storage\",\"schemaVersion\":2}\n",
        )
        .expect("write marker");

        assert_eq!(
            inspect(&candidate),
            Err(ModStorageDirectoryError::MarkerInvalid)
        );
        assert_eq!(
            inspector().claim(&candidate),
            Err(ModStorageDirectoryError::MarkerInvalid)
        );
    }

    #[test]
    fn marker_that_is_a_directory_is_invalid() {
        let temp = tempfile::tempdir().expect("temp");
        let candidate = temp.path().join("mods");
        fs::create_dir_all(candidate.join(MOD_STORAGE_MARKER_NAME)).expect("create fake marker");

        assert_eq!(
            inspect(&candidate),
            Err(ModStorageDirectoryError::MarkerInvalid)
        );
    }

    #[test]
    fn overlapping_game_root_is_rejected_in_both_directions() {
        let temp = tempfile::tempdir().expect("temp");
        let game_root = temp.path().join("game");
        fs::create_dir_all(game_root.join("nativePC")).expect("create game root");
        let inside_game = game_root.join("mods");
        let containing_game = temp.path().to_path_buf();

        assert_eq!(
            inspect_with_roots(&inside_game, std::slice::from_ref(&game_root)),
            Err(ModStorageDirectoryError::OverlapsGameRoot)
        );
        assert_eq!(
            inspect_with_roots(&containing_game, std::slice::from_ref(&game_root)),
            Err(ModStorageDirectoryError::OverlapsGameRoot)
        );
        assert_eq!(
            inspect_with_roots(&game_root, std::slice::from_ref(&game_root)),
            Err(ModStorageDirectoryError::OverlapsGameRoot)
        );
        let sibling = temp.path().join("mods");
        assert!(inspect_with_roots(&sibling, std::slice::from_ref(&game_root)).is_ok());
    }

    #[test]
    fn overlap_treats_unresolvable_paths_as_overlapping() {
        let temp = tempfile::tempdir().expect("temp");
        let inspector = inspector();
        // A relative path has no existing ancestor to anchor on, so it cannot be proven
        // disjoint; the conservative answer is "overlaps".
        assert!(inspector.directories_overlap(Path::new("no-such-relative-dir"), temp.path()));
        assert!(!inspector.directories_overlap(&temp.path().join("a"), &temp.path().join("b")));
    }

    #[test]
    fn sandbox_directory_emptiness_is_reported() {
        let temp = tempfile::tempdir().expect("temp");
        let root = temp.path().join("mods");
        let inspector = inspector();

        assert_eq!(inspector.sandbox_directory_has_entries(&root), Ok(false));
        fs::create_dir_all(root.join(MOD_STORAGE_SANDBOX_DIRECTORY)).expect("create sandboxes");
        assert_eq!(inspector.sandbox_directory_has_entries(&root), Ok(false));
        fs::create_dir_all(
            root.join(MOD_STORAGE_SANDBOX_DIRECTORY)
                .join("mod-import-1-0"),
        )
        .expect("create package");
        assert_eq!(inspector.sandbox_directory_has_entries(&root), Ok(true));
    }

    #[test]
    fn verify_claimed_accepts_claimed_layout_only_and_empty_directories() {
        let temp = tempfile::tempdir().expect("temp");
        let inspector = inspector();
        let claimed = temp.path().join("claimed");
        inspector.claim(&claimed).expect("claim");
        let layout_only = temp.path().join("layout-only");
        fs::create_dir_all(layout_only.join(MOD_STORAGE_SANDBOX_DIRECTORY)).expect("layout");
        let empty = temp.path().join("empty");
        fs::create_dir(&empty).expect("empty");

        assert_eq!(inspector.verify_claimed(&claimed), Ok(()));
        assert_eq!(inspector.verify_claimed(&layout_only), Ok(()));
        assert_eq!(inspector.verify_claimed(&empty), Ok(()));
        assert!(
            !layout_only.join(MOD_STORAGE_MARKER_NAME).exists(),
            "startup verification must not write a marker"
        );
    }

    #[test]
    fn verify_claimed_rejects_missing_foreign_and_tampered_directories() {
        let temp = tempfile::tempdir().expect("temp");
        let inspector = inspector();
        let missing = temp.path().join("unplugged").join("mods");
        let foreign = temp.path().join("foreign");
        fs::create_dir(&foreign).expect("foreign");
        fs::write(foreign.join("photo.jpg"), b"not ours").expect("foreign file");
        let tampered = temp.path().join("tampered");
        fs::create_dir(&tampered).expect("tampered");
        fs::write(tampered.join(MOD_STORAGE_MARKER_NAME), b"{}").expect("bad marker");

        assert_eq!(
            inspector.verify_claimed(&missing),
            Err(ModStorageDirectoryError::Unavailable)
        );
        assert_eq!(
            inspector.verify_claimed(&foreign),
            Err(ModStorageDirectoryError::MarkerRequired)
        );
        assert_eq!(
            inspector.verify_claimed(&tampered),
            Err(ModStorageDirectoryError::MarkerInvalid)
        );
        assert_eq!(
            inspector.verify_claimed(Path::new("relative")),
            Err(ModStorageDirectoryError::NotAbsolute)
        );
    }

    #[test]
    fn shape_errors_surface_before_any_file_system_access() {
        assert_eq!(
            inspect(Path::new("relative/mods")),
            Err(ModStorageDirectoryError::NotAbsolute)
        );
        assert_eq!(
            inspector().claim(Path::new("relative/mods")),
            Err(ModStorageDirectoryError::NotAbsolute)
        );
    }

    #[cfg(windows)]
    fn create_directory_link(target: &Path, link: &Path) {
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_str().expect("link path"),
                target.to_str().expect("target path"),
            ])
            .output()
            .expect("create directory junction");
        assert!(
            output.status.success(),
            "mklink failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    fn create_directory_link(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create directory symlink");
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).expect("remove directory junction");
    }

    #[cfg(unix)]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).expect("remove directory symlink");
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn directory_that_is_a_link_is_rejected() {
        let temp = tempfile::tempdir().expect("temp");
        let target = temp.path().join("real");
        fs::create_dir(&target).expect("create target");
        let link = temp.path().join("mods");
        create_directory_link(&target, &link);

        let inspected = inspect(&link);
        let claimed = inspector().claim(&link);

        remove_directory_link(&link);
        assert_eq!(inspected, Err(ModStorageDirectoryError::LinkRejected));
        assert_eq!(claimed, Err(ModStorageDirectoryError::LinkRejected));
    }

    #[test]
    #[cfg(any(unix, windows))]
    fn link_in_parent_chain_is_rejected() {
        let temp = tempfile::tempdir().expect("temp");
        let target = temp.path().join("real");
        fs::create_dir(&target).expect("create target");
        let link = temp.path().join("linked-parent");
        create_directory_link(&target, &link);
        let candidate = link.join("mods");

        let inspected = inspect(&candidate);

        remove_directory_link(&link);
        assert_eq!(inspected, Err(ModStorageDirectoryError::LinkRejected));
    }
}
