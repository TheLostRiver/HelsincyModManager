use crate::controlled_fs::{
    is_link_or_reparse, open_child_directory_nofollow, open_regular_file_nofollow,
};
use crate::TaskScopedModImportSandboxLocator;
use anyhow::{Context, Result};
use cap_std::fs::Dir;
use hmm_ports::{ModImportSandboxLocator, ModPackageSizeReader};
use std::path::PathBuf;

pub struct SandboxModPackageSizeReader {
    locator: TaskScopedModImportSandboxLocator,
}

impl SandboxModPackageSizeReader {
    pub fn new_in_storage_root(storage_root: PathBuf) -> Self {
        Self {
            locator: TaskScopedModImportSandboxLocator::new_in_storage_root(storage_root),
        }
    }
}

impl ModPackageSizeReader for SandboxModPackageSizeReader {
    fn read_content_size(&self, package_id: &str) -> Result<u64> {
        // Validate the opaque identity before opening any path. The retained directory
        // capabilities keep enumeration inside the managed root even during concurrent removal.
        self.locator.sandbox_root_for_package(package_id)?;
        let root = self.locator.open_existing_sandbox_root()?;
        let package =
            open_child_directory_nofollow(&root, package_id.as_ref(), "package size root")?;
        let mut entries = 0;
        directory_size(&package, 0, &mut entries)
    }
}

fn directory_size(directory: &Dir, depth: usize, entries: &mut usize) -> Result<u64> {
    anyhow::ensure!(depth <= 128, "package size depth limit exceeded");
    let mut total = 0u64;
    for entry in directory
        .entries()
        .context("package size enumeration failed")?
    {
        let entry = entry.context("package size entry unavailable")?;
        *entries += 1;
        anyhow::ensure!(*entries <= 65_536, "package size entry limit exceeded");
        let name = entry.file_name();
        let metadata = directory.symlink_metadata(&name)?;
        anyhow::ensure!(
            !is_link_or_reparse(&metadata),
            "package size linked entry rejected"
        );
        let size = if metadata.is_dir() {
            let child = open_child_directory_nofollow(directory, &name, "package size directory")?;
            directory_size(&child, depth + 1, entries)?
        } else {
            open_regular_file_nofollow(directory, &name, "package size file")?
                .metadata()?
                .len()
        };
        total = total.checked_add(size).context("package size overflow")?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_std::ambient_authority;

    #[test]
    fn counts_all_content_without_modifying_it_and_rejects_escaping_ids() {
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("sandboxes/package-a");
        std::fs::create_dir_all(package.join("custom/nested")).unwrap();
        std::fs::write(package.join("custom/nested/author.bin"), b"1234567").unwrap();
        std::fs::write(package.join("readme.txt"), b"abc").unwrap();
        let reader = SandboxModPackageSizeReader::new_in_storage_root(temp.path().to_owned());
        assert_eq!(reader.read_content_size("package-a").unwrap(), 10);
        assert_eq!(std::fs::read(package.join("readme.txt")).unwrap(), b"abc");
        assert!(reader.read_content_size("../outside").is_err());
        assert!(reader.read_content_size("missing").is_err());
        assert!(!temp.path().join("sandboxes/missing").exists());
    }

    #[test]
    fn empty_packages_are_zero_and_depth_is_bounded() {
        let temp = tempfile::tempdir().unwrap();
        let root = Dir::open_ambient_dir(temp.path(), ambient_authority()).unwrap();
        assert_eq!(directory_size(&root, 0, &mut 0).unwrap(), 0);
        assert!(directory_size(&root, 129, &mut 0).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn package_directory_junction_is_rejected_without_reading_its_target() {
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        let root = temp.path().join("storage");
        let package = root.join("sandboxes").join("package");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("private.bin"), b"leave unchanged").unwrap();
        let link = package.join("linked");
        let output = std::process::Command::new("cmd")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "mklink failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let reader = SandboxModPackageSizeReader::new_in_storage_root(root);
        assert!(reader.read_content_size("package").is_err());
        assert_eq!(
            std::fs::read(outside.join("private.bin")).unwrap(),
            b"leave unchanged"
        );
        std::fs::remove_dir(&link).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn package_file_symlinks_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("sandboxes/package");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(temp.path().join("outside"), b"private").unwrap();
        std::os::unix::fs::symlink(temp.path().join("outside"), package.join("link")).unwrap();
        assert!(
            SandboxModPackageSizeReader::new_in_storage_root(temp.path().to_owned())
                .read_content_size("package")
                .is_err()
        );
    }
}
