use crate::controlled_fs::{
    create_new_regular_file, ensure_regular_file_metadata, open_child_directory_nofollow,
    open_or_create_child_directory, open_regular_file_nofollow,
};
use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs::Dir};
use hmm_core::{PluginSelectionScope, PluginSelectionSnapshot};
use hmm_ports::PluginSelectionRepository;
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 只保存配置意图；安装状态与恢复仍以安装清单为准。
pub struct JsonPluginSelectionRepository {
    root: PathBuf,
    read_only: bool,
}

impl JsonPluginSelectionRepository {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            read_only: false,
        }
    }
    pub fn read_only(root: PathBuf) -> Self {
        Self {
            root,
            read_only: true,
        }
    }

    fn selection_path(&self, scope: &PluginSelectionScope) -> PathBuf {
        let mut hash = Sha256::new();
        for value in [
            "hmm-plugin-selection-v1",
            scope.game_id.as_str(),
            scope.profile_id.as_str(),
            scope.mod_id.as_str(),
            scope.revision_id.as_str(),
        ] {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        self.root
            .join(format!("plugin-selection-{:x}.json", hash.finalize()))
    }

    fn open_root(&self, create: bool) -> Result<Option<Dir>> {
        anyhow::ensure!(
            self.root.is_absolute(),
            "plugin selection root must be absolute"
        );
        let anchor = self
            .root
            .ancestors()
            .last()
            .context("plugin selection root has no anchor")?;
        let mut directory = Dir::open_ambient_dir(anchor, ambient_authority())
            .context("plugin selection anchor is unavailable")?;
        for component in self.root.strip_prefix(anchor)?.components() {
            let Component::Normal(name) = component else {
                anyhow::bail!("plugin selection root is invalid");
            };
            if create {
                directory =
                    open_or_create_child_directory(&directory, name, "plugin selection directory")?;
            } else {
                match directory.symlink_metadata(name) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => {
                        return Err(error).context("plugin selection directory is unavailable")
                    }
                    Ok(_) => {}
                }
                directory =
                    open_child_directory_nofollow(&directory, name, "plugin selection directory")?;
            }
        }
        Ok(Some(directory))
    }
}

impl PluginSelectionRepository for JsonPluginSelectionRepository {
    fn load_selection(
        &self,
        scope: &PluginSelectionScope,
    ) -> Result<Option<PluginSelectionSnapshot>> {
        let Some(directory) = self.open_root(false)? else {
            return Ok(None);
        };
        let path = self.selection_path(scope);
        let name = path
            .file_name()
            .context("plugin selection name is unavailable")?;
        match directory.symlink_metadata(name) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("plugin selection is unavailable"),
        }
        let file = open_regular_file_nofollow(&directory, name, "plugin selection")?;
        let mut bytes = Vec::new();
        file.take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .context("plugin selection cannot be read")?;
        anyhow::ensure!(
            bytes.len() <= 16 * 1024 * 1024,
            "plugin selection record is too large"
        );
        let selection: PluginSelectionSnapshot =
            serde_json::from_slice(&bytes).context("plugin selection is invalid")?;
        selection.validate_scope(scope)?;
        Ok(Some(selection))
    }

    fn save_selection(&self, selection: &PluginSelectionSnapshot) -> Result<()> {
        if self.read_only {
            anyhow::bail!("plugin selection repository is read-only");
        }
        let directory = self
            .open_root(true)?
            .context("plugin selection root is unavailable")?;
        let path = self.selection_path(selection.scope());
        let name = path
            .file_name()
            .context("plugin selection name is unavailable")?;
        match directory.symlink_metadata(name) {
            Ok(metadata) => ensure_regular_file_metadata(&metadata, "plugin selection")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("plugin selection is unavailable"),
        }
        let bytes = serde_json::to_vec_pretty(selection)
            .context("plugin selection cannot be serialized")?;
        anyhow::ensure!(
            bytes.len() <= 16 * 1024 * 1024,
            "plugin selection record is too large"
        );
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let temporary = format!(
            "plugin-choice-{}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        );
        let mut file = create_new_regular_file(
            &directory,
            temporary.as_ref(),
            "plugin selection temporary file",
        )?;
        let result = (|| -> Result<()> {
            file.write_all(&bytes)
                .context("plugin selection cannot be written")?;
            file.sync_all()
                .context("plugin selection cannot be synchronized")?;
            drop(file);
            directory
                .rename(&temporary, &directory, name)
                .context("plugin selection cannot be replaced")?;
            sync_selection_directory(&directory)
        })();
        if result.is_err() {
            let _ = directory.remove_file(&temporary);
        }
        result
    }
}

fn sync_selection_directory(directory: &Dir) -> Result<()> {
    #[cfg(windows)]
    let result = directory.try_clone()?.into_std_file().sync_all();
    #[cfg(not(windows))]
    let result = {
        use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt as _};
        use cap_std::fs::{OpenOptions, OpenOptionsExt as _};

        // Linux directory capabilities can use O_PATH, which cannot be synced. Reopen the
        // directory through its verified handle so a replaced path cannot redirect the sync.
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC)
            .follow(FollowSymlinks::No);
        directory
            .open_with(".", &options)
            .and_then(|directory| directory.sync_all())
    };
    #[cfg(windows)]
    if result
        .as_ref()
        .is_err_and(crate::install_commit::is_windows_directory_sync_capability_error)
    {
        return Ok(());
    }
    result.context("plugin selection directory cannot be synchronized")
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        GameId, InstallTargetPath, InstalledFileSummary, ModId, ModRevisionId, PackageFileId,
        PluginFileChoice, PluginFileChoiceKind, ProfileId,
    };

    fn selection() -> PluginSelectionSnapshot {
        PluginSelectionSnapshot::new(
            PluginSelectionScope {
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("profile"),
                mod_id: ModId::new("mod"),
                revision_id: ModRevisionId::new("revision"),
            },
            "fixture.plugin",
            1,
            vec![PluginFileChoice {
                package_file_id: PackageFileId::new("fixture-file"),
                target_path: InstallTargetPath::parse("content/plugin.bin", ["content"]).unwrap(),
                source_file: InstalledFileSummary {
                    size_bytes: 1,
                    sha256: "a".repeat(64),
                },
                choice: PluginFileChoiceKind::Exclude,
                excluded_by_package_selection: false,
            }],
        )
        .unwrap()
    }

    #[test]
    fn plugin_choices_roundtrip_and_are_isolated_by_profile_mod_and_revision() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("choices");
        let repository = JsonPluginSelectionRepository::new(root.clone());
        let selected = selection();
        assert!(repository
            .load_selection(selected.scope())
            .unwrap()
            .is_none());
        assert!(!root.exists());
        repository.save_selection(&selected).unwrap();
        assert_eq!(
            repository.load_selection(selected.scope()).unwrap(),
            Some(selected.clone())
        );
        for kind in ["profile", "mod", "revision"] {
            let mut scope = selected.scope().clone();
            match kind {
                "profile" => scope.profile_id = ProfileId::new("other"),
                "mod" => scope.mod_id = ModId::new("other"),
                "revision" => scope.revision_id = ModRevisionId::new("other"),
                _ => unreachable!(),
            }
            assert!(repository.load_selection(&scope).unwrap().is_none());
            fs::copy(
                repository.selection_path(selected.scope()),
                repository.selection_path(&scope),
            )
            .unwrap();
            assert!(
                repository.load_selection(&scope).is_err(),
                "scope must be checked after reading"
            );
        }
        let readonly = JsonPluginSelectionRepository::read_only(root);
        assert_eq!(
            readonly.load_selection(selected.scope()).unwrap(),
            Some(selected.clone())
        );
        assert!(readonly.save_selection(&selected).is_err());
    }

    #[test]
    fn replacing_plugin_choices_persists_the_new_record_without_temporary_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("choices");
        let repository = JsonPluginSelectionRepository::new(root.clone());
        let original = selection();
        repository.save_selection(&original).unwrap();

        let mut files = original.files().to_vec();
        files[0].choice = PluginFileChoiceKind::Include;
        let replacement = PluginSelectionSnapshot::new(
            original.scope().clone(),
            original.policy_id(),
            original.policy_version(),
            files,
        )
        .unwrap();
        JsonPluginSelectionRepository::new(root.clone())
            .save_selection(&replacement)
            .unwrap();

        assert_eq!(
            JsonPluginSelectionRepository::read_only(root.clone())
                .load_selection(original.scope())
                .unwrap(),
            Some(replacement)
        );
        let entries = fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(
            entries,
            vec![repository
                .selection_path(original.scope())
                .file_name()
                .unwrap()
                .to_owned()]
        );
    }

    #[cfg(unix)]
    #[test]
    fn directory_sync_uses_the_opened_handle_after_the_path_is_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("choices");
        let repository = JsonPluginSelectionRepository::new(root.clone());
        let directory = repository.open_root(true).unwrap().unwrap();
        let moved = temp.path().join("moved");
        fs::rename(&root, &moved).unwrap();
        let missing = temp.path().join("missing");
        std::os::unix::fs::symlink(&missing, &root).unwrap();

        directory.write("record.json", b"saved").unwrap();
        sync_selection_directory(&directory).unwrap();

        assert_eq!(fs::read(moved.join("record.json")).unwrap(), b"saved");
        assert!(!missing.exists());
        assert!(fs::symlink_metadata(root).unwrap().file_type().is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn directory_sync_propagates_a_non_directory_handle_error() {
        let file = tempfile::tempfile().unwrap();
        let directory = Dir::from_std_file(file);
        let error = sync_selection_directory(&directory).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .raw_os_error(),
            Some(libc::ENOTDIR)
        );
    }

    #[test]
    fn damaged_selection_is_an_error_instead_of_default_permission() {
        let temp = tempfile::tempdir().unwrap();
        let repository = JsonPluginSelectionRepository::new(temp.path().to_path_buf());
        let selected = selection();
        fs::write(
            repository.selection_path(selected.scope()),
            b"incomplete json",
        )
        .unwrap();
        assert!(repository.load_selection(selected.scope()).is_err());
    }

    #[test]
    fn readonly_repository_never_creates_a_missing_directory() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("missing/choices");
        let repository = JsonPluginSelectionRepository::read_only(root.clone());
        assert!(repository
            .load_selection(selection().scope())
            .unwrap()
            .is_none());
        assert!(repository.save_selection(&selection()).is_err());
        assert!(!root.exists());
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    fn directory_link(link: &std::path::Path, target: &std::path::Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }
    #[cfg(windows)]
    fn directory_link(link: &std::path::Path, target: &std::path::Path) {
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap()
            .status;
        assert!(status.success());
    }
    #[cfg(unix)]
    fn unlink_directory(link: &std::path::Path) {
        fs::remove_file(link).unwrap();
    }
    #[cfg(windows)]
    fn unlink_directory(link: &std::path::Path) {
        fs::remove_dir(link).unwrap();
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn linked_root_ancestor_and_record_are_rejected_without_outside_side_effects() {
        for kind in ["root", "ancestor", "record"] {
            let temp = tempfile::tempdir().unwrap();
            let outside = temp.path().join("outside");
            fs::create_dir(&outside).unwrap();
            fs::write(outside.join("sentinel"), b"unchanged").unwrap();
            let selected = selection();
            let (root, link) = match kind {
                "root" => {
                    let root = temp.path().join("choices");
                    (root.clone(), root)
                }
                "ancestor" => {
                    let link = temp.path().join("managed");
                    (link.join("choices"), link)
                }
                _ => {
                    let root = temp.path().join("choices");
                    fs::create_dir(&root).unwrap();
                    let repository = JsonPluginSelectionRepository::new(root.clone());
                    let link = repository.selection_path(selected.scope());
                    (root, link)
                }
            };
            directory_link(&link, &outside);
            let repository = JsonPluginSelectionRepository::new(root);
            assert!(
                repository.load_selection(selected.scope()).is_err(),
                "read followed {kind}"
            );
            assert!(
                repository.save_selection(&selected).is_err(),
                "write followed {kind}"
            );
            assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"unchanged");
            assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
            unlink_directory(&link);
        }
    }

    #[cfg(unix)]
    #[test]
    fn linked_record_file_is_not_read_or_replaced() {
        let temp = tempfile::tempdir().unwrap();
        let selected = selection();
        let outside = temp.path().join("outside.json");
        let bytes = serde_json::to_vec(&selected).unwrap();
        fs::write(&outside, &bytes).unwrap();
        let root = temp.path().join("choices");
        fs::create_dir(&root).unwrap();
        let repository = JsonPluginSelectionRepository::new(root);
        std::os::unix::fs::symlink(&outside, repository.selection_path(selected.scope())).unwrap();
        assert!(repository.load_selection(selected.scope()).is_err());
        assert!(repository.save_selection(&selected).is_err());
        assert_eq!(fs::read(&outside).unwrap(), bytes);
    }
}
