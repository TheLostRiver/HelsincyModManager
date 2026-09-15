use crate::controlled_fs::{create_new_regular_file, open_regular_file_nofollow};
use anyhow::{Context, Result};
use cap_std::fs::Dir;
use std::{
    ffi::OsStr,
    io::{Read, Write},
};

const MAX_RECORD_BYTES: u64 = 64 * 1024 * 1024;

pub(super) fn read_json(root: &Dir, name: &OsStr) -> Result<Option<serde_json::Value>> {
    match root.symlink_metadata(name) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    let file = open_regular_file_nofollow(root, name, "installation record")?;
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_RECORD_BYTES,
        "installation record is too large"
    );
    Ok(Some(serde_json::from_slice(&bytes)?))
}

pub(super) fn write_json(root: &Dir, name: &str, bytes: &[u8]) -> Result<()> {
    // Both temporary creation and replacement use the verified directory handle.
    match root.symlink_metadata(name) {
        Ok(_) => {
            open_regular_file_nofollow(root, OsStr::new(name), "installation registry")?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_RECORD_BYTES,
        "installation registry is too large"
    );
    let temporary = format!("installation-scope-{}.tmp", uuid::Uuid::new_v4());
    let mut file =
        create_new_regular_file(root, OsStr::new(&temporary), "installation temporary file")?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        root.rename(&temporary, root, name)?;
        sync_directory(root)
    })();
    if result.is_err() {
        let _ = root.remove_file(&temporary);
    }
    result
}

fn sync_directory(directory: &Dir) -> Result<()> {
    #[cfg(windows)]
    let result = directory.try_clone()?.into_std_file().sync_all();
    #[cfg(not(windows))]
    let result = {
        use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt as _};
        use cap_std::fs::{OpenOptions, OpenOptionsExt as _};
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC)
            .follow(FollowSymlinks::No);
        directory
            .open_with(".", &options)
            .and_then(|file| file.sync_all())
    };
    #[cfg(windows)]
    if result
        .as_ref()
        .is_err_and(crate::install_commit::is_windows_directory_sync_capability_error)
    {
        return Ok(());
    }
    result.context("installation registry directory cannot be synchronized")
}
