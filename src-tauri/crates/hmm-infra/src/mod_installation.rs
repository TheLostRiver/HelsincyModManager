use crate::controlled_fs::{
    create_new_regular_file, open_child_directory_nofollow, open_or_create_child_directory,
    open_regular_file_nofollow,
};
use anyhow::{bail, Context, Result};
use cap_std::{ambient_authority, fs::Dir};
use fs2::FileExt;
use hmm_core::{GameInstance, InstallManifest, ModInstallationContext, ProfileId};
use hmm_ports::{
    ModInstallationScopeError, ModInstallationScopeIndex, ModInstallationScopeRepository,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    path::{Component, PathBuf},
    sync::Mutex,
};

const REGISTRY: &str = "installation-scopes.json";
// Append-only registry snapshots are staged durably before replacing the primary snapshot.
// A missing primary file cannot silently bind existing files to a different game directory.
const REGISTRY_NEXT: &str = "installation-scopes.next.json";
const LOCK: &str = "installation-scopes.lock";

mod store;
use store::{read_json, write_json};

#[derive(Default, Serialize, Deserialize)]
struct ScopeRegistry {
    version: u32,
    installations: Vec<ModInstallationContext>,
}

/// Binds installation records to game directories. Legacy records remain in place with their
/// original namespace, hashes and backup references; account-profile CRUD cannot orphan them.
pub struct JsonModInstallationScopeRepository {
    install_root: PathBuf,
    access: Mutex<()>,
}

impl JsonModInstallationScopeRepository {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            install_root: app_data_dir.join("install"),
            access: Mutex::new(()),
        }
    }

    fn open_root(&self, create: bool) -> Result<Option<Dir>> {
        anyhow::ensure!(
            self.install_root.is_absolute(),
            "install root must be absolute"
        );
        let anchor = self
            .install_root
            .ancestors()
            .last()
            .context("install anchor is missing")?;
        let mut directory = Dir::open_ambient_dir(anchor, ambient_authority())?;
        for component in self.install_root.strip_prefix(anchor)?.components() {
            let Component::Normal(name) = component else {
                bail!("install root is invalid");
            };
            if create {
                directory = open_or_create_child_directory(&directory, name, "install directory")?;
            } else {
                match directory.symlink_metadata(name) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => return Err(error.into()),
                    Ok(_) => {}
                }
                directory = open_child_directory_nofollow(&directory, name, "install directory")?;
            }
        }
        Ok(Some(directory))
    }

    fn lock(root: &Dir) -> Result<std::fs::File> {
        let file = match root.symlink_metadata(LOCK) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Another process can create the lock between inspection and creation.
                match create_new_regular_file(root, OsStr::new(LOCK), "installation lock") {
                    Ok(file) => file,
                    Err(_) => {
                        open_regular_file_nofollow(root, OsStr::new(LOCK), "installation lock")?
                    }
                }
            }
            Err(error) => return Err(error.into()),
            Ok(_) => open_regular_file_nofollow(root, OsStr::new(LOCK), "installation lock")?,
        }
        .into_std();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(file),
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn read_registry(root: &Dir) -> Result<ScopeRegistry> {
        let primary = read_json(root, OsStr::new(REGISTRY))?
            .map(serde_json::from_value::<ScopeRegistry>)
            .transpose()?;
        let pending = read_json(root, OsStr::new(REGISTRY_NEXT))?
            .map(serde_json::from_value::<ScopeRegistry>)
            .transpose()?;
        for registry in [&primary, &pending].into_iter().flatten() {
            Self::validate_registry(registry)?;
        }
        match (primary, pending) {
            (Some(primary), Some(pending)) => {
                anyhow::ensure!(
                    pending.installations.starts_with(&primary.installations),
                    "installation registry snapshots disagree"
                );
                Ok(pending)
            }
            (Some(registry), None) | (None, Some(registry)) => Ok(registry),
            (None, None) => Ok(ScopeRegistry {
                version: 1,
                installations: Vec::new(),
            }),
        }
    }

    fn validate_registry(registry: &ScopeRegistry) -> Result<()> {
        anyhow::ensure!(registry.version == 1, "unsupported installation registry");
        let mut installations = BTreeSet::new();
        let mut scopes = BTreeSet::new();
        for entry in &registry.installations {
            anyhow::ensure!(
                valid_scope(entry.scope_id.as_str())
                    && valid_installation_key(&entry.installation_id)
                    && installations.insert(entry.installation_id.clone())
                    && scopes.insert(entry.scope_id.clone()),
                "invalid installation registry"
            );
        }
        Ok(())
    }

    fn record_scopes(root: &Dir, active_only: bool) -> Result<BTreeSet<ProfileId>> {
        let mut scopes = BTreeSet::new();
        for name in [
            "manifests",
            "recovery",
            "reinstall-recovery",
            "replacement-selections",
            "plugin-selections",
        ] {
            match root.symlink_metadata(name) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
                Ok(_) => {}
            }
            let directory =
                open_child_directory_nofollow(root, OsStr::new(name), "installation records")?;
            for entry in directory.entries()? {
                let file_name = entry?.file_name();
                if !file_name.to_string_lossy().ends_with(".json") {
                    continue;
                }
                let value = read_json(&directory, &file_name)?
                    .context("installation record disappeared")?;
                let id = value
                    .get("profile_id")
                    .or_else(|| value.get("scope").and_then(|scope| scope.get("profile_id")))
                    .and_then(|id| id.as_str())
                    .context("installation record has no namespace")?;
                anyhow::ensure!(valid_scope(id), "invalid installation namespace");
                let scope = ProfileId::new(id);
                if name == "manifests" {
                    anyhow::ensure!(
                        file_name.to_str() == Some(&format!("{id}.json")),
                        "manifest namespace mismatch"
                    );
                    let manifest: InstallManifest = serde_json::from_value(value)?;
                    manifest.validate()?;
                    if active_only
                        && manifest.entries.is_empty()
                        && manifest.replacement_bindings.is_empty()
                        && manifest.plugin_selections.is_empty()
                        && manifest.status.consumption()
                            == hmm_core::InstallManifestStatusConsumption::TrustEntries
                    {
                        continue;
                    }
                }
                scopes.insert(scope);
            }
        }
        Ok(scopes)
    }

    fn resolve(
        &self,
        game: &GameInstance,
        persist: bool,
    ) -> Result<ModInstallationContext, ModInstallationScopeError> {
        let key = installation_key(game).map_err(|_| ModInstallationScopeError::GameUnavailable)?;
        let _access = self
            .access
            .lock()
            .map_err(|_| ModInstallationScopeError::Unavailable)?;
        let Some(root) = self
            .open_root(persist)
            .map_err(|_| ModInstallationScopeError::Unavailable)?
        else {
            return Ok(ModInstallationContext {
                game_id: game.game_id.clone(),
                installation_id: key,
                scope_id: ProfileId::new("default"),
            });
        };
        let _lock = persist
            .then(|| Self::lock(&root))
            .transpose()
            .map_err(|_| ModInstallationScopeError::Unavailable)?;
        let mut registry =
            Self::read_registry(&root).map_err(|_| ModInstallationScopeError::Unavailable)?;
        if let Some(context) = registry
            .installations
            .iter()
            .find(|entry| entry.installation_id == key && entry.game_id == game.game_id)
        {
            return Ok(context.clone());
        }
        let recorded =
            Self::record_scopes(&root, true).map_err(|_| ModInstallationScopeError::Unavailable)?;
        let bound = registry
            .installations
            .iter()
            .map(|entry| entry.scope_id.clone())
            .collect::<BTreeSet<_>>();
        let unbound = recorded.difference(&bound).cloned().collect::<Vec<_>>();
        if unbound.len() > 1 || (!registry.installations.is_empty() && !unbound.is_empty()) {
            return Err(ModInstallationScopeError::LegacyAmbiguous);
        }
        if unbound
            .iter()
            .any(|id| id.as_str().starts_with("installation-") && id.as_str() != key)
        {
            return Err(ModInstallationScopeError::LegacyAmbiguous);
        }
        // Retain the first namespace for existing installations and CLI compatibility. Subsequent
        // directories always receive distinct namespaces; a save profile cannot select either.
        let scope_id = match unbound.first() {
            Some(id) => id.clone(),
            None if registry.installations.is_empty() => ProfileId::new("default"),
            None => ProfileId::new(&key),
        };
        let context = ModInstallationContext {
            game_id: game.game_id.clone(),
            installation_id: key,
            scope_id,
        };
        registry.installations.push(context.clone());
        if persist {
            let bytes = serde_json::to_vec_pretty(&registry)
                .map_err(|_| ModInstallationScopeError::Unavailable)?;
            write_json(&root, REGISTRY_NEXT, &bytes)
                .map_err(|_| ModInstallationScopeError::Unavailable)?;
            write_json(&root, REGISTRY, &bytes)
                .map_err(|_| ModInstallationScopeError::Unavailable)?;
        }
        Ok(context)
    }

    /// Read-only automation can inspect the same binding without creating a registry or lock file.
    pub fn inspect_scope(
        &self,
        game: &GameInstance,
    ) -> Result<ModInstallationContext, ModInstallationScopeError> {
        self.resolve(game, false)
    }
}

impl ModInstallationScopeRepository for JsonModInstallationScopeRepository {
    fn inspect_scope(
        &self,
        game: &GameInstance,
    ) -> Result<ModInstallationContext, ModInstallationScopeError> {
        self.resolve(game, false)
    }

    fn resolve_scope(
        &self,
        game: &GameInstance,
    ) -> Result<ModInstallationContext, ModInstallationScopeError> {
        self.resolve(game, true)
    }
}

impl ModInstallationScopeIndex for JsonModInstallationScopeRepository {
    fn list_scope_ids(&self) -> Result<Vec<ProfileId>> {
        let _access = self
            .access
            .lock()
            .map_err(|_| anyhow::anyhow!("installation registry is busy"))?;
        let Some(root) = self.open_root(false)? else {
            return Ok(Vec::new());
        };
        let _lock = Self::lock(&root)?;
        let registry = Self::read_registry(&root)?;
        let mut scopes = Self::record_scopes(&root, false)?;
        scopes.extend(
            registry
                .installations
                .into_iter()
                .map(|entry| entry.scope_id),
        );
        Ok(scopes.into_iter().collect())
    }
}

fn installation_key(game: &GameInstance) -> Result<String> {
    anyhow::ensure!(
        game.root_dir.is_absolute(),
        "game directory must be absolute"
    );
    let path = std::fs::canonicalize(&game.root_dir).context("game directory is unavailable")?;
    anyhow::ensure!(path.is_dir(), "game directory must be a directory");
    let path = path
        .to_str()
        .context("game directory encoding is invalid")?;
    #[cfg(windows)]
    let path = path
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
        .to_lowercase();
    let mut hash = Sha256::new();
    hash.update(game.game_id.as_str().as_bytes());
    hash.update([0]);
    hash.update(path.as_bytes());
    Ok(format!("installation-{:x}", hash.finalize()))
}

fn valid_scope(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn valid_installation_key(value: &str) -> bool {
    value.strip_prefix("installation-").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[cfg(test)]
mod tests;
