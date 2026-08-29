use crate::install_commit::{
    atomic_write_file, ensure_contained_existing_path, ensure_existing_directory,
};
use anyhow::{Context, Result};
use hmm_core::{ModId, ProfileId, ReplacementBindingSnapshot};
use hmm_ports::ReplacementSelectionRepository;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

pub struct JsonReplacementSelectionRepository {
    selection_root: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct StoredReplacementSelection {
    schema_version: u32,
    profile_id: ProfileId,
    binding: ReplacementBindingSnapshot,
}

impl JsonReplacementSelectionRepository {
    pub fn new(selection_root: PathBuf) -> Self {
        Self { selection_root }
    }

    fn selection_path(&self, profile_id: &ProfileId, mod_id: &ModId) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(b"profile:");
        hasher.update(profile_id.as_str().as_bytes());
        hasher.update(b"\0mod:");
        hasher.update(mod_id.as_str().as_bytes());
        let digest_hex: String = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();

        self.selection_root
            .join(format!("selection-{digest_hex}.json"))
    }

    fn deserialize_selection(
        &self,
        path: &std::path::Path,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<ReplacementBindingSnapshot> {
        let serialized =
            fs::read_to_string(path).context("failed to read replacement selection")?;
        let stored: StoredReplacementSelection = serde_json::from_str(&serialized)
            .context("failed to deserialize replacement selection")?;
        if stored.profile_id != *profile_id || stored.binding.binding().mod_id() != mod_id {
            anyhow::bail!("replacement selection does not match the requested profile/mod");
        }
        Ok(stored.binding)
    }
}

impl ReplacementSelectionRepository for JsonReplacementSelectionRepository {
    fn load_selection(
        &self,
        profile_id: &ProfileId,
        mod_id: &ModId,
    ) -> Result<Option<ReplacementBindingSnapshot>> {
        match fs::symlink_metadata(&self.selection_root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).context("failed to inspect replacement selection root")
            }
        }

        ensure_existing_directory(&self.selection_root, "replacement selection root")?;
        ensure_contained_existing_path(&self.selection_root, &self.selection_root)?;
        let selection_path = self.selection_path(profile_id, mod_id);
        let metadata = match fs::symlink_metadata(&selection_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("failed to inspect replacement selection"),
        };
        if metadata.is_symlink() || !metadata.is_file() {
            anyhow::bail!("replacement selection is not a regular file");
        }
        ensure_contained_existing_path(&self.selection_root, &selection_path)?;
        Ok(Some(self.deserialize_selection(
            &selection_path,
            profile_id,
            mod_id,
        )?))
    }

    fn save_selection(&self, binding: &ReplacementBindingSnapshot) -> Result<()> {
        if !self.selection_root.exists() {
            fs::create_dir_all(&self.selection_root)
                .context("failed to create replacement selection root")?;
        }
        ensure_existing_directory(&self.selection_root, "replacement selection root")?;
        ensure_contained_existing_path(&self.selection_root, &self.selection_root)?;
        let selection_path =
            self.selection_path(binding.binding().profile_id(), binding.binding().mod_id());
        let stored = StoredReplacementSelection {
            schema_version: 1,
            profile_id: binding.binding().profile_id().clone(),
            binding: binding.clone(),
        };
        let serialized = serde_json::to_vec_pretty(&stored)
            .context("failed to serialize replacement selection")?;
        atomic_write_file(&selection_path, &serialized)
    }

    fn remove_selection(&self, profile_id: &ProfileId, mod_id: &ModId) -> Result<()> {
        match fs::symlink_metadata(&self.selection_root) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(error).context("failed to inspect replacement selection root")
            }
        }

        let selection_path = self.selection_path(profile_id, mod_id);
        match fs::symlink_metadata(&selection_path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).context("failed to inspect replacement selection"),
        }
        ensure_contained_existing_path(&self.selection_root, &selection_path)?;
        fs::remove_file(&selection_path).context("failed to remove replacement selection")
    }
}
