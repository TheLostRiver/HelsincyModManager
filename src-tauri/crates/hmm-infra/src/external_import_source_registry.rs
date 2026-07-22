use anyhow::{anyhow, bail, Context, Result};
use hmac::{Hmac, Mac};
use hmm_core::{ExternalImportAdapterId, ExternalImportSource, ExternalImportSourceId};
use hmm_ports::{ExternalImportSourceRegistration, ExternalImportSourceRegistry};
use sha2::Sha256;
use std::collections::HashMap;
#[cfg(not(windows))]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID: &str = "hunting_box_directory_v1";
const SOURCE_REGISTRATION_TTL_MILLIS: u64 = 30 * 60 * 1000;
const SOURCE_KEY_BYTES: usize = 32;
const SOURCE_KEY_FILE_NAME: &str = "source-fingerprint-key-v1.bin";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub(crate) struct RegisteredHuntingBoxSource {
    pub source: ExternalImportSource,
    pub root_directory: PathBuf,
    pub source_fingerprint: String,
}

pub struct HuntingBoxDirectorySourceRegistry {
    source_key: [u8; SOURCE_KEY_BYTES],
    registrations: Mutex<HashMap<ExternalImportSourceId, RegisteredHuntingBoxSource>>,
}

impl HuntingBoxDirectorySourceRegistry {
    pub fn new(app_data_directory: &Path) -> Result<Self> {
        let source_key = load_or_create_source_key(&app_data_directory.join(SOURCE_KEY_FILE_NAME))?;
        Ok(Self {
            source_key,
            registrations: Mutex::new(HashMap::new()),
        })
    }

    pub fn register_directory(&self, root_directory: PathBuf) -> Result<ExternalImportSource> {
        validate_source_root(&root_directory)?;
        let canonical_root = root_directory
            .canonicalize()
            .context("failed to resolve external import source")?;
        let source_fingerprint = self.source_fingerprint_for_root(&canonical_root);
        let now = now_unix_millis()?;
        let expires_at_unix_millis = now
            .checked_add(SOURCE_REGISTRATION_TTL_MILLIS)
            .ok_or_else(|| anyhow!("external import source expiry overflow"))?;
        let source = ExternalImportSource {
            source_id: ExternalImportSourceId::new(format!(
                "external-import-source-{}",
                Uuid::new_v4()
            )),
            adapter_id: ExternalImportAdapterId::new(HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID),
            display_label: "Hunting Box directory".to_owned(),
            expires_at_unix_millis,
        };
        let registered = RegisteredHuntingBoxSource {
            source: source.clone(),
            root_directory: canonical_root,
            source_fingerprint,
        };
        self.registrations
            .lock()
            .map_err(|_| anyhow!("external import source registry lock poisoned"))?
            .insert(source.source_id.clone(), registered);
        Ok(source)
    }

    pub(crate) fn resolve_directory(
        &self,
        source_id: &ExternalImportSourceId,
    ) -> Result<Option<RegisteredHuntingBoxSource>> {
        let now = now_unix_millis()?;
        let mut registrations = self
            .registrations
            .lock()
            .map_err(|_| anyhow!("external import source registry lock poisoned"))?;
        registrations.retain(|_, registration| registration.source.expires_at_unix_millis > now);
        Ok(registrations.get(source_id).cloned())
    }

    pub(crate) fn source_item_key_hash(
        &self,
        registration: &RegisteredHuntingBoxSource,
        item_identity: &[u8],
    ) -> String {
        keyed_digest_hex(
            &self.source_key,
            b"hmm.external-import.source-item-key.v1",
            &[registration.source_fingerprint.as_bytes(), item_identity],
        )
    }

    fn source_fingerprint_for_root(&self, canonical_root: &Path) -> String {
        let normalized_root = normalize_source_identity(canonical_root);
        keyed_digest_hex(
            &self.source_key,
            b"hmm.external-import.source-fingerprint.v1",
            &[
                HUNTING_BOX_DIRECTORY_V1_ADAPTER_ID.as_bytes(),
                normalized_root.as_bytes(),
            ],
        )
    }
}

impl ExternalImportSourceRegistry for HuntingBoxDirectorySourceRegistry {
    fn resolve_source(
        &self,
        source_id: &ExternalImportSourceId,
    ) -> Result<Option<ExternalImportSourceRegistration>> {
        Ok(self.resolve_directory(source_id)?.map(|registration| {
            ExternalImportSourceRegistration {
                source: registration.source,
                source_fingerprint: registration.source_fingerprint,
            }
        }))
    }
}

pub(crate) fn is_symlink_or_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || is_reparse_point(metadata)
}

fn validate_source_root(root_directory: &Path) -> Result<()> {
    let metadata =
        fs::symlink_metadata(root_directory).context("failed to inspect external import source")?;
    if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
        bail!("external import source is not a regular directory");
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn normalize_source_identity(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn keyed_digest_hex(key: &[u8; SOURCE_KEY_BYTES], domain: &[u8], parts: &[&[u8]]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("fixed HMAC key length is valid");
    mac.update(domain);
    for part in parts {
        mac.update(&(part.len() as u64).to_be_bytes());
        mac.update(part);
    }
    hex_encode(&mac.finalize().into_bytes())
}

fn load_or_create_source_key(path: &Path) -> Result<[u8; SOURCE_KEY_BYTES]> {
    match fs::read(path) {
        Ok(bytes) => source_key_from_bytes(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => create_source_key(path),
        Err(error) => Err(error).context("failed to read external import source key"),
    }
}

fn create_source_key(path: &Path) -> Result<[u8; SOURCE_KEY_BYTES]> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create external import key directory")?;
    }

    let key = generate_source_key();
    let temporary_path = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .context("failed to create external import source key")?;
        file.write_all(&key)
            .context("failed to write external import source key")?;
        file.sync_all()
            .context("failed to sync external import source key")?;
        fs::rename(&temporary_path, path)
            .context("failed to persist external import source key")?;
        sync_parent_directory(path)?;
        Ok(())
    })();

    match write_result {
        Ok(()) => Ok(key),
        Err(error) => {
            let _ = fs::remove_file(&temporary_path);
            match fs::read(path) {
                Ok(existing) => source_key_from_bytes(existing),
                Err(_) => Err(error),
            }
        }
    }
}

fn generate_source_key() -> [u8; SOURCE_KEY_BYTES] {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let mut key = [0_u8; SOURCE_KEY_BYTES];
    key[..16].copy_from_slice(first.as_bytes());
    key[16..].copy_from_slice(second.as_bytes());
    key
}

fn source_key_from_bytes(bytes: Vec<u8>) -> Result<[u8; SOURCE_KEY_BYTES]> {
    let key: [u8; SOURCE_KEY_BYTES] = bytes
        .try_into()
        .map_err(|_| anyhow!("external import source key has invalid length"))?;
    Ok(key)
}

#[cfg(windows)]
fn sync_parent_directory(path: &Path) -> Result<()> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("external import source key has no parent directory"))?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(parent)
        .context("failed to open external import key directory")?
        .sync_all()
        .context("failed to sync external import key directory")?;
    Ok(())
}

#[cfg(not(windows))]
fn sync_parent_directory(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("external import source key has no parent directory"))?;
    File::open(parent)
        .context("failed to open external import key directory")?
        .sync_all()
        .context("failed to sync external import key directory")?;
    Ok(())
}

fn now_unix_millis() -> Result<u64> {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("external import system clock is unavailable")?
        .as_millis();
    u64::try_from(milliseconds).context("external import system clock value is too large")
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

    #[test]
    fn source_fingerprint_survives_registry_restart_without_exposing_the_source_path() {
        let app_data = tempfile::tempdir().expect("app data directory");
        let source_root = tempfile::tempdir().expect("source directory");
        let first_registry =
            HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("first registry");
        let first_source = first_registry
            .register_directory(source_root.path().to_path_buf())
            .expect("register source");
        let first_fingerprint = first_registry
            .resolve_directory(&first_source.source_id)
            .expect("resolve first source")
            .expect("first source exists")
            .source_fingerprint;

        drop(first_registry);

        let second_registry =
            HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("second registry");
        let second_source = second_registry
            .register_directory(source_root.path().to_path_buf())
            .expect("register source after restart");
        let second_fingerprint = second_registry
            .resolve_directory(&second_source.source_id)
            .expect("resolve second source")
            .expect("second source exists")
            .source_fingerprint;
        let source_path = source_root.path().to_string_lossy();

        assert_eq!(first_fingerprint, second_fingerprint);
        assert!(!first_fingerprint.contains(source_path.as_ref()));
        assert!(!first_source.source_id.as_str().contains(['/', '\\']));
        assert!(!second_source.source_id.as_str().contains(['/', '\\']));
        assert_eq!(
            first_source.display_label, "Hunting Box directory",
            "the public source label is not derived from the selected path"
        );
    }

    #[test]
    fn registry_rejects_a_non_directory_source_without_creating_a_registration() {
        let app_data = tempfile::tempdir().expect("app data directory");
        let fixture = tempfile::tempdir().expect("fixture directory");
        let source_file = fixture.path().join("not-a-directory.txt");
        fs::write(&source_file, b"fixture").expect("write source file fixture");
        let registry = HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("registry");

        assert!(registry.register_directory(source_file).is_err());
    }
}
