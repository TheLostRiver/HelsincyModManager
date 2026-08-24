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
/// 狩技盒子把 MHW(Steam AppId 582010)的 Mod 库放在安装根目录下的 `Mods_582010`。
/// 该常量归 adapter 所有:`hunting_box_directory_v1` 本就是 MHW 形态的来源契约。
const HUNTING_BOX_MHW_LIBRARY_DIRECTORY: &str = "Mods_582010";
/// 有效库根探测的枚举上限:防止误选巨型目录(如盘符根)时无界遍历。
/// 超限时放弃下潜、按所选目录原样注册,行为不劣于未探测。
const EFFECTIVE_ROOT_PROBE_MAX_ENTRIES: usize = 4096;

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
        // 在注册层一次性解析有效库根:选狩技盒子安装根与直接选 `Mods_582010`
        // 得到同一 fingerprint identity,scanner/materializer 契约零改动。
        let canonical_root = resolve_effective_library_root(canonical_root);
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
        self.resolve_registered(|registration| &registration.source.source_id == source_id)
    }

    fn resolve_registered(
        &self,
        predicate: impl Fn(&RegisteredHuntingBoxSource) -> bool,
    ) -> Result<Option<RegisteredHuntingBoxSource>> {
        let now = now_unix_millis()?;
        let mut registrations = self
            .registrations
            .lock()
            .map_err(|_| anyhow!("external import source registry lock poisoned"))?;
        registrations.retain(|_, registration| registration.source.expires_at_unix_millis > now);
        Ok(registrations
            .values()
            .find(|registration| predicate(registration))
            .cloned())
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

    fn resolve_matching_source(
        &self,
        source_fingerprint: &str,
    ) -> Result<Option<ExternalImportSourceRegistration>> {
        Ok(self
            .resolve_registered(|registration| {
                registration.source_fingerprint == source_fingerprint
            })?
            .map(|registration| ExternalImportSourceRegistration {
                source: registration.source,
                source_fingerprint: registration.source_fingerprint,
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

/// 玩家常会选狩技盒子安装根而不是 `Mods_582010`。规则是确定性的:
/// 所选目录已有直接数字子目录 → 以所选目录为准;没有数字子目录但存在常规的
/// `Mods_582010` 子目录 → 下潜一层;其余情况(含探测超限、子目录是链接/重解析点)
/// 原样注册,交给扫描与预览文案兜底。
fn resolve_effective_library_root(canonical_root: PathBuf) -> PathBuf {
    let Ok(entries) = fs::read_dir(&canonical_root) else {
        return canonical_root;
    };
    let mut library_child: Option<PathBuf> = None;
    for (seen, entry) in entries.enumerate() {
        if seen >= EFFECTIVE_ROOT_PROBE_MAX_ENTRIES {
            return canonical_root;
        }
        let Ok(entry) = entry else {
            return canonical_root;
        };
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if is_symlink_or_reparse_point(&metadata) || !metadata.is_dir() {
            continue;
        }
        if is_numeric_directory_name(name) {
            return canonical_root;
        }
        // Windows 大小写不敏感,按 ASCII 忽略大小写匹配库目录名。
        if name.eq_ignore_ascii_case(HUNTING_BOX_MHW_LIBRARY_DIRECTORY) {
            library_child = Some(entry.path());
        }
    }
    library_child.unwrap_or(canonical_root)
}

fn is_numeric_directory_name(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
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
    fn registering_the_hunting_box_root_descends_into_the_mhw_library_directory() {
        let app_data = tempfile::tempdir().expect("app data directory");
        let box_root = tempfile::tempdir().expect("hunting box root");
        let library = box_root.path().join("Mods_582010");
        fs::create_dir_all(library.join("1001")).expect("create library candidate");
        fs::create_dir_all(box_root.path().join("Saves_582010")).expect("create saves directory");
        fs::write(box_root.path().join("config.ini"), b"[settings]").expect("write config file");
        let registry = HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("registry");

        let from_root = registry
            .register_directory(box_root.path().to_path_buf())
            .expect("register hunting box root");
        let from_root = registry
            .resolve_directory(&from_root.source_id)
            .expect("resolve root registration")
            .expect("root registration exists");
        let from_library = registry
            .register_directory(library.clone())
            .expect("register library directly");
        let from_library = registry
            .resolve_directory(&from_library.source_id)
            .expect("resolve library registration")
            .expect("library registration exists");

        assert_eq!(
            from_root.root_directory,
            library.canonicalize().expect("canonical library path")
        );
        // 选安装根与直接选 Mods_582010 必须得到同一 identity:
        // retry 的 resolve_matching_source 两种选法都要能匹配同一批次。
        assert_eq!(
            from_root.source_fingerprint,
            from_library.source_fingerprint
        );
    }

    #[test]
    fn root_with_direct_numeric_children_never_descends() {
        let app_data = tempfile::tempdir().expect("app data directory");
        let source_root = tempfile::tempdir().expect("source directory");
        fs::create_dir_all(source_root.path().join("1001")).expect("create numeric candidate");
        fs::create_dir_all(source_root.path().join("Mods_582010").join("2001"))
            .expect("create nested library");
        let registry = HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("registry");

        let source = registry
            .register_directory(source_root.path().to_path_buf())
            .expect("register source");
        let registration = registry
            .resolve_directory(&source.source_id)
            .expect("resolve source")
            .expect("source exists");

        assert_eq!(
            registration.root_directory,
            source_root
                .path()
                .canonicalize()
                .expect("canonical source path")
        );
    }

    #[test]
    fn root_with_only_another_game_library_stays_as_selected() {
        let app_data = tempfile::tempdir().expect("app data directory");
        let source_root = tempfile::tempdir().expect("source directory");
        // 只有崛起(1446780)的库:HMM 当前只管 MHW,不下潜、按所选目录原样注册。
        fs::create_dir_all(source_root.path().join("Mods_1446780").join("1001"))
            .expect("create other-game library");
        let registry = HuntingBoxDirectorySourceRegistry::new(app_data.path()).expect("registry");

        let source = registry
            .register_directory(source_root.path().to_path_buf())
            .expect("register source");
        let registration = registry
            .resolve_directory(&source.source_id)
            .expect("resolve source")
            .expect("source exists");

        assert_eq!(
            registration.root_directory,
            source_root
                .path()
                .canonicalize()
                .expect("canonical source path")
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
