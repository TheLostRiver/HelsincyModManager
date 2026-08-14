use anyhow::{Context, Result};
use cap_fs_ext::{DirExt as _, FollowSymlinks, OpenOptionsFollowExt as _};
use cap_std::ambient_authority;
#[cfg(windows)]
use cap_std::fs::MetadataExt as _;
use cap_std::fs::{Dir, File, Metadata, OpenOptions};
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::path::Path;

/// Opens a directory's final path component without following a link or reparse point.
/// The caller supplies an application-controlled directory path, never an untrusted relative path.
pub(crate) fn open_existing_directory_nofollow(path: &Path, label: &str) -> Result<Dir> {
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .with_context(|| format!("{label} must have a parent directory"))?;
    let name = path
        .file_name()
        .with_context(|| format!("{label} must have a final path component"))?;
    let parent = Dir::open_ambient_dir(parent_path, ambient_authority())
        .with_context(|| format!("failed to open {label} parent directory"))?;
    open_child_directory_nofollow(&parent, name, label)
}

/// Creates a directory only at its final component, then opens that component no-follow.
pub(crate) fn open_or_create_directory_nofollow(path: &Path, label: &str) -> Result<Dir> {
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .with_context(|| format!("{label} must have a parent directory"))?;
    let name = path
        .file_name()
        .with_context(|| format!("{label} must have a final path component"))?;
    let parent = Dir::open_ambient_dir(parent_path, ambient_authority())
        .with_context(|| format!("failed to open {label} parent directory"))?;
    open_or_create_child_directory(&parent, name, label)
}

pub(crate) fn open_child_directory_nofollow(
    parent: &Dir,
    name: &OsStr,
    label: &str,
) -> Result<Dir> {
    let directory = parent
        .open_dir_nofollow(name)
        .with_context(|| format!("failed to open {label}"))?;
    ensure_real_directory(&directory, label)?;
    Ok(directory)
}

pub(crate) fn open_or_create_child_directory(
    parent: &Dir,
    name: &OsStr,
    label: &str,
) -> Result<Dir> {
    match parent.create_dir(name) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).with_context(|| format!("failed to create {label}")),
    }
    open_child_directory_nofollow(parent, name, label)
}

/// Creates a previously absent direct child and immediately reopens it without following links.
/// Existing entries fail closed so callers never adopt or clean up an attacker-provided scope.
pub(crate) fn create_new_child_directory(parent: &Dir, name: &OsStr, label: &str) -> Result<Dir> {
    parent
        .create_dir(name)
        .with_context(|| format!("failed to create {label}"))?;
    open_child_directory_nofollow(parent, name, label)
}

/// Descends through application-controlled literal directory names from an already verified
/// capability root. Each component is created and reopened without following links.
pub(crate) fn open_or_create_directory_chain(
    root: &Dir,
    components: &[&str],
    label: &str,
) -> Result<Dir> {
    let mut current = root
        .try_clone()
        .with_context(|| format!("failed to clone {label} root handle"))?;
    for component in components {
        current = open_or_create_child_directory(&current, OsStr::new(component), label)?;
    }
    Ok(current)
}

/// Descends through application-controlled literal directory names without creating missing
/// components. Every reopened child is checked through the existing capability handle.
pub(crate) fn open_existing_directory_chain(
    root: &Dir,
    components: &[&str],
    label: &str,
) -> Result<Dir> {
    let mut current = root
        .try_clone()
        .with_context(|| format!("failed to clone {label} root handle"))?;
    for component in components {
        current = open_child_directory_nofollow(&current, OsStr::new(component), label)?;
    }
    Ok(current)
}

pub(crate) fn create_new_regular_file(parent: &Dir, name: &OsStr, label: &str) -> Result<File> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let file = parent
        .open_with(name, &options)
        .with_context(|| format!("failed to create {label}"))?;
    ensure_regular_file_metadata(
        &file
            .metadata()
            .with_context(|| format!("failed to inspect created {label}"))?,
        label,
    )?;
    Ok(file)
}

pub(crate) fn open_regular_file_nofollow(parent: &Dir, name: &OsStr, label: &str) -> Result<File> {
    let metadata = parent
        .symlink_metadata(name)
        .with_context(|| format!("failed to inspect {label}"))?;
    ensure_regular_file_metadata(&metadata, label)?;

    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = parent
        .open_with(name, &options)
        .with_context(|| format!("failed to open {label}"))?;
    ensure_regular_file_metadata(
        &file
            .metadata()
            .with_context(|| format!("failed to inspect opened {label}"))?,
        label,
    )?;
    Ok(file)
}

pub(crate) fn ensure_real_directory(directory: &Dir, label: &str) -> Result<()> {
    let metadata = directory
        .dir_metadata()
        .with_context(|| format!("failed to inspect {label}"))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        anyhow::bail!("{label} is not a real directory");
    }
    Ok(())
}

pub(crate) fn ensure_regular_file_metadata(metadata: &Metadata, label: &str) -> Result<()> {
    if !metadata.is_file() || is_link_or_reparse(metadata) {
        anyhow::bail!("{label} is not a regular file");
    }
    Ok(())
}

pub(crate) fn is_link_or_reparse(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

/// Removes a direct child using opened no-follow directory handles only. A link, reparse point,
/// unsupported entry, or concurrent replacement aborts cleanup rather than traversing outside root.
pub(crate) fn remove_child_tree_nofollow(parent: &Dir, name: &OsStr, label: &str) -> Result<()> {
    let child = match open_child_directory_nofollow(parent, name, label) {
        Ok(child) => child,
        Err(error) if is_not_found(&error) => return Ok(()),
        Err(error) => return Err(error),
    };
    remove_open_directory_contents(&child, label)?;
    drop(child);
    match parent.remove_dir(name) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {label}")),
    }
}

pub(crate) fn remove_empty_child_directory(parent: &Dir, name: &OsStr, label: &str) -> Result<()> {
    match parent.remove_dir(name) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove empty {label}")),
    }
}

fn remove_open_directory_contents(directory: &Dir, label: &str) -> Result<()> {
    let entries = directory
        .entries()
        .with_context(|| format!("failed to read {label}"))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read {label} entry"))?;
        let name = entry.file_name();
        let metadata = directory
            .symlink_metadata(&name)
            .with_context(|| format!("failed to inspect {label} entry"))?;
        if is_link_or_reparse(&metadata) {
            anyhow::bail!("{label} contains a link or reparse point");
        }
        if metadata.is_dir() {
            let child = open_child_directory_nofollow(directory, &name, label)?;
            remove_open_directory_contents(&child, label)?;
            drop(child);
            directory
                .remove_dir(&name)
                .with_context(|| format!("failed to remove {label} directory entry"))?;
        } else if metadata.is_file() {
            let opened = open_regular_file_nofollow(directory, &name, label)?;
            drop(opened);
            directory
                .remove_file(&name)
                .with_context(|| format!("failed to remove {label} file entry"))?;
        } else {
            anyhow::bail!("{label} contains an unsupported entry");
        }
    }
    Ok(())
}

pub(crate) fn is_not_found(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|cause| cause.kind() == ErrorKind::NotFound)
}
