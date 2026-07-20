use anyhow::{Context, Result};
use fs2::FileExt;
use hmm_core::{ModId, ModRevisionId};
use hmm_ports::{
    ModImportCatalogUpsert, ModImportResultRepository, StoredLogicalMod, StoredModImportAnalysis,
    StoredModOriginProvenance, StoredModRevision, MOD_IMPORT_UPSERT_CHUNK_SIZE,
    MOD_IMPORT_UPSERT_MAX_ENTRIES,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MOD_IMPORT_RESULTS_SCHEMA_V1: u32 = 1;
const MOD_REVISION_CATALOG_SCHEMA_V2: u32 = 2;

#[derive(Debug, Deserialize)]
struct ModImportResultsV1 {
    version: u32,
    records: Vec<StoredModImportAnalysis>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ModRevisionCatalogV2 {
    version: u32,
    mods: Vec<StoredLogicalMod>,
    revisions: Vec<StoredModRevision>,
}

impl Default for ModRevisionCatalogV2 {
    fn default() -> Self {
        Self {
            version: MOD_REVISION_CATALOG_SCHEMA_V2,
            mods: Vec::new(),
            revisions: Vec::new(),
        }
    }
}

struct LoadedCatalog {
    catalog: ModRevisionCatalogV2,
    migrated_from_v1: bool,
}

pub struct JsonModImportResultRepository {
    file_path: PathBuf,
    write_lock: Mutex<()>,
    #[cfg(test)]
    test_write_failure: Option<ModImportCatalogWriteFailure>,
    #[cfg(test)]
    test_catalog_save_count: AtomicUsize,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModImportCatalogWriteFailure {
    TempWrite,
    Rename,
    Unlock,
}

impl JsonModImportResultRepository {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            write_lock: Mutex::new(()),
            #[cfg(test)]
            test_write_failure: None,
            #[cfg(test)]
            test_catalog_save_count: AtomicUsize::new(0),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_test_write_failure(mut self, failure: ModImportCatalogWriteFailure) -> Self {
        self.test_write_failure = Some(failure);
        self
    }

    #[cfg(test)]
    pub(crate) fn catalog_save_count_for_test(&self) -> usize {
        self.test_catalog_save_count.load(Ordering::SeqCst)
    }

    fn read_catalog<T>(
        &self,
        operation: impl FnOnce(&ModRevisionCatalogV2) -> Result<T>,
    ) -> Result<T> {
        self.with_exclusive_lock(|| {
            let loaded = self.load_catalog()?;
            if loaded.migrated_from_v1 {
                self.save_catalog(&loaded.catalog)?;
            }
            operation(&loaded.catalog)
        })
    }

    fn mutate_catalog<T>(
        &self,
        operation: impl FnOnce(&mut ModRevisionCatalogV2) -> Result<T>,
    ) -> Result<T> {
        self.with_exclusive_lock(|| {
            let mut catalog = self.load_catalog()?.catalog;
            let value = operation(&mut catalog)?;
            validate_catalog(&catalog)?;
            self.save_catalog(&catalog)?;
            Ok(value)
        })
    }

    fn with_exclusive_lock<T>(&self, operation: impl FnOnce() -> Result<T>) -> Result<T> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| anyhow::anyhow!("mod revision catalog write lock poisoned"))?;
        let lock_file = self.open_lock_file()?;
        lock_file
            .lock_exclusive()
            .context("failed to lock mod revision catalog")?;

        let result = operation();
        #[cfg(test)]
        let unlock_result = if self.test_write_failure == Some(ModImportCatalogWriteFailure::Unlock)
        {
            let _ = FileExt::unlock(&lock_file);
            Err(anyhow::anyhow!(
                "injected mod revision catalog unlock failure"
            ))
        } else {
            lock_file
                .unlock()
                .context("failed to unlock mod revision catalog")
        };
        #[cfg(not(test))]
        let unlock_result = lock_file
            .unlock()
            .context("failed to unlock mod revision catalog");
        let _ = unlock_result;
        result
    }

    fn load_catalog(&self) -> Result<LoadedCatalog> {
        if !self.file_path.exists() {
            return Ok(LoadedCatalog {
                catalog: ModRevisionCatalogV2::default(),
                migrated_from_v1: false,
            });
        }

        let bytes = fs::read(&self.file_path).context("failed to read mod revision catalog")?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).context("mod revision catalog is corrupted")?;
        let version = value
            .get("version")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("mod revision catalog is corrupted"))?;

        match version {
            version if version == u64::from(MOD_IMPORT_RESULTS_SCHEMA_V1) => {
                let legacy: ModImportResultsV1 =
                    serde_json::from_value(value).context("mod revision catalog is corrupted")?;
                if legacy.version != MOD_IMPORT_RESULTS_SCHEMA_V1 {
                    anyhow::bail!("mod revision catalog is corrupted");
                }
                Ok(LoadedCatalog {
                    catalog: migrate_v1_catalog(legacy)?,
                    migrated_from_v1: true,
                })
            }
            version if version == u64::from(MOD_REVISION_CATALOG_SCHEMA_V2) => {
                let catalog: ModRevisionCatalogV2 =
                    serde_json::from_value(value).context("mod revision catalog is corrupted")?;
                validate_catalog(&catalog)?;
                Ok(LoadedCatalog {
                    catalog,
                    migrated_from_v1: false,
                })
            }
            _ => anyhow::bail!("mod revision catalog is corrupted"),
        }
    }

    fn save_catalog(&self, catalog: &ModRevisionCatalogV2) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .context("failed to create mod revision catalog directory")?;
        }

        let serialized = serde_json::to_string_pretty(catalog)
            .context("failed to serialize mod revision catalog")?;
        let temp_path = self.unique_temp_path();
        let result = (|| {
            #[cfg(test)]
            if self.test_write_failure == Some(ModImportCatalogWriteFailure::TempWrite) {
                anyhow::bail!("injected mod import catalog temp write failure");
            }

            {
                let mut temp_file = File::create(&temp_path)
                    .context("failed to create mod revision catalog temp file")?;
                temp_file
                    .write_all(serialized.as_bytes())
                    .context("failed to write mod revision catalog temp file")?;
                temp_file
                    .sync_all()
                    .context("failed to sync mod revision catalog temp file")?;
            }

            #[cfg(test)]
            if self.test_write_failure == Some(ModImportCatalogWriteFailure::Rename) {
                anyhow::bail!("injected mod import catalog rename failure");
            }

            fs::rename(&temp_path, &self.file_path)
                .context("failed to replace mod revision catalog")?;
            self.sync_parent_directory()?;
            #[cfg(test)]
            self.test_catalog_save_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }

    fn sync_parent_directory(&self) -> Result<()> {
        let Some(parent) = self.file_path.parent() else {
            return Ok(());
        };

        open_directory_for_sync(parent)
            .and_then(|directory| directory.sync_all())
            .context("failed to sync mod revision catalog directory")?;
        Ok(())
    }

    fn lock_file_path(&self) -> PathBuf {
        let lock_name = self
            .file_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("{name}.lock"))
            .unwrap_or_else(|| "mod-import-results.json.lock".to_owned());
        self.file_path
            .parent()
            .map(|parent| parent.join(&lock_name))
            .unwrap_or_else(|| PathBuf::from(lock_name))
    }

    fn unique_temp_path(&self) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let temp_name = self
            .file_path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("{name}.{}.{}.tmp", std::process::id(), nonce))
            .unwrap_or_else(|| {
                format!(
                    "mod-import-results.{}.{}.json.tmp",
                    std::process::id(),
                    nonce
                )
            });
        self.file_path
            .parent()
            .map(|parent| parent.join(&temp_name))
            .unwrap_or_else(|| PathBuf::from(temp_name))
    }

    fn open_lock_file(&self) -> Result<File> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .context("failed to create mod revision catalog directory")?;
        }
        OpenOptions::new()
            .create(true)
            .read(true)
            .truncate(false)
            .write(true)
            .open(self.lock_file_path())
            .context("failed to open mod revision catalog lock")
    }
}

impl ModImportResultRepository for JsonModImportResultRepository {
    fn save_new_mod(
        &self,
        logical_mod: &StoredLogicalMod,
        revision: &StoredModRevision,
    ) -> Result<()> {
        self.mutate_catalog(|catalog| {
            anyhow::ensure!(
                logical_mod.mod_id == revision.mod_id
                    && logical_mod.origin_revision_id == revision.revision_id
                    && logical_mod.display_revision_id == revision.revision_id,
                "logical Mod and origin revision do not match"
            );
            if catalog
                .mods
                .iter()
                .any(|stored| stored.mod_id == logical_mod.mod_id)
            {
                anyhow::bail!("logical Mod already exists");
            }
            ensure_revision_id_available(catalog, revision)?;
            catalog.mods.push(logical_mod.clone());
            catalog.revisions.push(revision.clone());
            Ok(())
        })
    }

    fn append_revision(&self, revision: &StoredModRevision) -> Result<()> {
        self.mutate_catalog(|catalog| {
            ensure_revision_id_available(catalog, revision)?;
            let logical_mod = catalog
                .mods
                .iter_mut()
                .find(|logical_mod| logical_mod.mod_id == revision.mod_id)
                .ok_or_else(|| anyhow::anyhow!("logical Mod not found"))?;
            logical_mod.display_revision_id = revision.revision_id.clone();
            catalog.revisions.push(revision.clone());
            Ok(())
        })
    }

    fn upsert_many(&self, upserts: &[ModImportCatalogUpsert]) -> Result<()> {
        anyhow::ensure!(
            upserts.len() <= MOD_IMPORT_UPSERT_MAX_ENTRIES,
            "mod import upsert batch exceeds the supported limit"
        );
        if upserts.is_empty() {
            return Ok(());
        }

        self.with_exclusive_lock(|| {
            let loaded = self.load_catalog()?;
            let mut catalog = loaded.catalog;
            let mut migration_pending = loaded.migrated_from_v1;
            for chunk in upserts.chunks(MOD_IMPORT_UPSERT_CHUNK_SIZE) {
                let before_chunk = catalog.clone();
                for upsert in chunk {
                    apply_catalog_upsert(&mut catalog, upsert)?;
                }
                if migration_pending || catalog != before_chunk {
                    validate_catalog(&catalog)?;
                    self.save_catalog(&catalog)?;
                    migration_pending = false;
                }
            }
            Ok(())
        })
    }

    fn get_mod(&self, mod_id: &ModId) -> Result<Option<StoredLogicalMod>> {
        self.read_catalog(|catalog| {
            Ok(catalog
                .mods
                .iter()
                .find(|logical_mod| &logical_mod.mod_id == mod_id)
                .cloned())
        })
    }

    fn list_mods(&self) -> Result<Vec<StoredLogicalMod>> {
        self.read_catalog(|catalog| Ok(catalog.mods.clone()))
    }

    fn get_revision(&self, revision_id: &ModRevisionId) -> Result<Option<StoredModRevision>> {
        self.read_catalog(|catalog| {
            Ok(catalog
                .revisions
                .iter()
                .find(|revision| &revision.revision_id == revision_id)
                .cloned())
        })
    }

    fn list_revisions(&self, mod_id: &ModId) -> Result<Vec<StoredModRevision>> {
        self.read_catalog(|catalog| {
            Ok(catalog
                .revisions
                .iter()
                .filter(|revision| &revision.mod_id == mod_id)
                .cloned()
                .collect())
        })
    }

    fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> Result<()> {
        self.mutate_catalog(|catalog| {
            let mod_id = ModId::new(&analysis.mod_id);
            if let Some(logical_mod) = catalog
                .mods
                .iter()
                .find(|logical_mod| logical_mod.mod_id == mod_id)
            {
                let display_revision = catalog
                    .revisions
                    .iter_mut()
                    .find(|revision| revision.revision_id == logical_mod.display_revision_id)
                    .ok_or_else(|| anyhow::anyhow!("mod revision catalog is corrupted"))?;
                anyhow::ensure!(
                    display_revision.package_id == analysis.package_id,
                    "compatibility analysis update cannot append a revision"
                );
                display_revision.preview_image = analysis.preview_image.clone();
                return Ok(());
            }

            let revision_id = ModRevisionId::new(&analysis.package_id);
            let logical_mod = StoredLogicalMod {
                mod_id: mod_id.clone(),
                origin_revision_id: revision_id.clone(),
                display_revision_id: revision_id.clone(),
                origin_provenance: StoredModOriginProvenance::Imported,
            };
            let revision = revision_from_analysis(revision_id, mod_id, analysis);
            ensure_revision_id_available(catalog, &revision)?;
            catalog.mods.push(logical_mod);
            catalog.revisions.push(revision);
            Ok(())
        })
    }

    fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
        self.read_catalog(projected_analyses)
    }

    fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
        self.read_catalog(|catalog| {
            Ok(projected_analyses(catalog)?
                .into_iter()
                .find(|analysis| analysis.mod_id == mod_id))
        })
    }
}

fn apply_catalog_upsert(
    catalog: &mut ModRevisionCatalogV2,
    upsert: &ModImportCatalogUpsert,
) -> Result<()> {
    anyhow::ensure!(
        upsert.logical_mod.mod_id == upsert.revision.mod_id
            && upsert.logical_mod.display_revision_id == upsert.revision.revision_id,
        "logical Mod and display revision do not match"
    );

    if let Some(existing_revision) = catalog
        .revisions
        .iter()
        .find(|revision| revision.revision_id == upsert.revision.revision_id)
        .cloned()
    {
        anyhow::ensure!(
            existing_revision == upsert.revision,
            "revision upsert conflicts with existing catalog state"
        );
        let existing_mod = catalog
            .mods
            .iter()
            .find(|logical_mod| logical_mod.mod_id == upsert.logical_mod.mod_id)
            .ok_or_else(|| anyhow::anyhow!("mod revision catalog is corrupted"))?;
        anyhow::ensure!(
            existing_mod.origin_revision_id == upsert.logical_mod.origin_revision_id
                && existing_mod.origin_provenance == upsert.logical_mod.origin_provenance,
            "logical Mod origin does not match"
        );
        return Ok(());
    }

    match catalog
        .mods
        .iter_mut()
        .find(|logical_mod| logical_mod.mod_id == upsert.logical_mod.mod_id)
    {
        Some(existing_mod) => {
            anyhow::ensure!(
                existing_mod.origin_revision_id == upsert.logical_mod.origin_revision_id
                    && existing_mod.origin_provenance == upsert.logical_mod.origin_provenance,
                "logical Mod origin does not match"
            );
            existing_mod.display_revision_id = upsert.revision.revision_id.clone();
            catalog.revisions.push(upsert.revision.clone());
        }
        None => {
            anyhow::ensure!(
                upsert.logical_mod.origin_revision_id == upsert.revision.revision_id,
                "new logical Mod origin revision does not match"
            );
            catalog.mods.push(upsert.logical_mod.clone());
            catalog.revisions.push(upsert.revision.clone());
        }
    }
    Ok(())
}

fn migrate_v1_catalog(legacy: ModImportResultsV1) -> Result<ModRevisionCatalogV2> {
    let mut catalog = ModRevisionCatalogV2::default();
    for analysis in legacy.records {
        let mod_id = ModId::new(&analysis.mod_id);
        let revision_id = ModRevisionId::new(&analysis.package_id);
        catalog.mods.push(StoredLogicalMod {
            mod_id: mod_id.clone(),
            origin_revision_id: revision_id.clone(),
            display_revision_id: revision_id.clone(),
            origin_provenance: StoredModOriginProvenance::MigratedV1 {
                legacy_mod_id: analysis.mod_id.clone(),
                legacy_package_id: analysis.package_id.clone(),
            },
        });
        catalog
            .revisions
            .push(revision_from_analysis(revision_id, mod_id, &analysis));
    }
    validate_catalog(&catalog)?;
    Ok(catalog)
}

fn revision_from_analysis(
    revision_id: ModRevisionId,
    mod_id: ModId,
    analysis: &StoredModImportAnalysis,
) -> StoredModRevision {
    StoredModRevision {
        revision_id,
        mod_id,
        import_task_id: analysis.task_id.clone(),
        package_id: analysis.package_id.clone(),
        display_name: analysis.display_name.clone(),
        metadata: analysis.metadata.clone(),
        preview_image: analysis.preview_image.clone(),
    }
}

fn projected_analyses(catalog: &ModRevisionCatalogV2) -> Result<Vec<StoredModImportAnalysis>> {
    let revisions = catalog
        .revisions
        .iter()
        .map(|revision| (revision.revision_id.as_str(), revision))
        .collect::<HashMap<_, _>>();
    catalog
        .mods
        .iter()
        .map(|logical_mod| {
            let revision = revisions
                .get(logical_mod.display_revision_id.as_str())
                .ok_or_else(|| anyhow::anyhow!("mod revision catalog is corrupted"))?;
            anyhow::ensure!(
                revision.mod_id == logical_mod.mod_id,
                "mod revision catalog is corrupted"
            );
            Ok(revision.as_analysis())
        })
        .collect()
}

fn ensure_revision_id_available(
    catalog: &ModRevisionCatalogV2,
    candidate: &StoredModRevision,
) -> Result<()> {
    if let Some(existing) = catalog
        .revisions
        .iter()
        .find(|revision| revision.revision_id == candidate.revision_id)
    {
        if existing.mod_id != candidate.mod_id {
            anyhow::bail!("revision already belongs to another logical Mod");
        }
        anyhow::bail!("revision already exists");
    }
    if catalog
        .revisions
        .iter()
        .any(|revision| revision.package_id == candidate.package_id)
    {
        anyhow::bail!("package already belongs to another revision");
    }
    Ok(())
}

fn validate_catalog(catalog: &ModRevisionCatalogV2) -> Result<()> {
    anyhow::ensure!(
        catalog.version == MOD_REVISION_CATALOG_SCHEMA_V2,
        "mod revision catalog is corrupted"
    );

    let mut mod_ids = HashSet::new();
    for logical_mod in &catalog.mods {
        anyhow::ensure!(
            mod_ids.insert(logical_mod.mod_id.as_str()),
            "mod revision catalog contains duplicate logical Mod ids"
        );
    }

    let mut revisions_by_id = HashMap::new();
    let mut package_revision_ids = HashMap::new();
    for revision in &catalog.revisions {
        if let Some(existing) = revisions_by_id.insert(revision.revision_id.as_str(), revision) {
            if existing.mod_id != revision.mod_id {
                anyhow::bail!("revision already belongs to another logical Mod");
            }
            anyhow::bail!("mod revision catalog contains duplicate revision ids");
        }
        if package_revision_ids
            .insert(&revision.package_id, revision.revision_id.as_str())
            .is_some()
        {
            anyhow::bail!("package already belongs to another revision");
        }
        anyhow::ensure!(
            mod_ids.contains(revision.mod_id.as_str()),
            "mod revision catalog contains an orphan revision"
        );
    }

    for logical_mod in &catalog.mods {
        for revision_id in [
            &logical_mod.origin_revision_id,
            &logical_mod.display_revision_id,
        ] {
            let revision = revisions_by_id
                .get(revision_id.as_str())
                .ok_or_else(|| anyhow::anyhow!("mod revision catalog is corrupted"))?;
            anyhow::ensure!(
                revision.mod_id == logical_mod.mod_id,
                "mod revision catalog contains a revision owner mismatch"
            );
        }
        if let StoredModOriginProvenance::MigratedV1 {
            legacy_mod_id,
            legacy_package_id,
        } = &logical_mod.origin_provenance
        {
            let origin_revision = revisions_by_id
                .get(logical_mod.origin_revision_id.as_str())
                .ok_or_else(|| anyhow::anyhow!("mod revision catalog is corrupted"))?;
            anyhow::ensure!(
                legacy_mod_id == logical_mod.mod_id.as_str()
                    && legacy_package_id == &origin_revision.package_id,
                "migration provenance does not match origin revision"
            );
        }
    }
    Ok(())
}

#[cfg(windows)]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[cfg(not(windows))]
fn open_directory_for_sync(path: &Path) -> std::io::Result<File> {
    File::open(path)
}
