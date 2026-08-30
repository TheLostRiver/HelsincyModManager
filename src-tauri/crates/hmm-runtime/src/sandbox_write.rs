use cap_fs_ext::{DirExt as _, FollowSymlinks, OpenOptionsFollowExt as _};
use cap_std::ambient_authority;
use cap_std::fs::{Dir, Metadata, OpenOptions};
use std::fmt;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};

use crate::game_automation::{is_canonically_within, is_safe_absolute_path};
use crate::RuntimeEnvironment;

pub const SANDBOX_MARKER_FILE_NAME: &str = ".hmm-sandbox.json";
pub const SANDBOX_MARKER_SCHEMA: &str = "{\"kind\":\"hmm.sandbox\",\"schemaVersion\":1}\n";

const MARKER_NAME: &str = SANDBOX_MARKER_FILE_NAME;
#[cfg(windows)]
const REPARSE_POINT_ATTRIBUTE: u32 = 0x0000_0400;

/// A process-local, non-serializable proof that a Sandbox root passed the write admission gate.
///
/// The directory handle is retained so later operations can be revalidated against the same
/// object instead of trusting a path that may have been replaced. Callers cannot construct this
/// type directly because all fields and the constructor are private.
pub struct SandboxWriteCapability {
    root: PathBuf,
    canonical_root: PathBuf,
    identity: DirectoryIdentity,
    directory: Dir,
}

impl fmt::Debug for SandboxWriteCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SandboxWriteCapability")
            .finish_non_exhaustive()
    }
}

impl SandboxWriteCapability {
    pub(crate) fn acquire(
        environment: &RuntimeEnvironment,
    ) -> Result<Self, SandboxWriteCapabilityError> {
        let Some(data_dir) = environment.sandbox_data_dir() else {
            return Err(SandboxWriteCapabilityError::ProductionForbidden);
        };

        let root = data_dir;
        if !is_safe_absolute_path(root) {
            return Err(SandboxWriteCapabilityError::RootUnsafe);
        }
        reject_link_or_reparse_chain(root)?;

        let directory = open_root_nofollow(root)?;
        let root_metadata = directory
            .dir_metadata()
            .map_err(|_| SandboxWriteCapabilityError::RootUnavailable)?;
        ensure_real_directory(&root_metadata)?;

        let canonical_root = root
            .canonicalize()
            .map_err(|_| SandboxWriteCapabilityError::RootUnavailable)?;
        let identity = DirectoryIdentity::from_directory(&directory)
            .ok_or(SandboxWriteCapabilityError::RootIdentityUnavailable)?;
        let path_directory = open_root_nofollow(root)?;
        let path_identity = DirectoryIdentity::from_directory(&path_directory)
            .ok_or(SandboxWriteCapabilityError::RootIdentityUnavailable)?;
        if identity != path_identity {
            return Err(SandboxWriteCapabilityError::RootReplaced);
        }

        initialize_or_validate_marker(&directory)?;

        let capability = Self {
            root: root.to_path_buf(),
            canonical_root,
            identity,
            directory,
        };
        capability.revalidate()?;
        Ok(capability)
    }

    /// Rechecks the path, marker, link state, and directory identity before a write operation.
    pub fn revalidate(&self) -> Result<(), SandboxWriteCapabilityError> {
        reject_link_or_reparse_chain(&self.root)?;
        let canonical_root = self
            .root
            .canonicalize()
            .map_err(|_| SandboxWriteCapabilityError::RootReplaced)?;
        if !same_path(&canonical_root, &self.canonical_root) {
            return Err(SandboxWriteCapabilityError::RootReplaced);
        }

        let path_directory = open_root_nofollow(&self.root)
            .map_err(|_| SandboxWriteCapabilityError::RootReplaced)?;
        let path_identity = DirectoryIdentity::from_directory(&path_directory)
            .ok_or(SandboxWriteCapabilityError::RootIdentityUnavailable)?;
        if path_identity != self.identity {
            return Err(SandboxWriteCapabilityError::RootReplaced);
        }

        let handle_metadata = self
            .directory
            .dir_metadata()
            .map_err(|_| SandboxWriteCapabilityError::RootReplaced)?;
        ensure_real_directory(&handle_metadata)?;
        if DirectoryIdentity::from_directory(&self.directory)
            .is_none_or(|identity| identity != self.identity)
        {
            return Err(SandboxWriteCapabilityError::RootReplaced);
        }
        validate_marker(&self.directory)
    }

    /// Validates the app-data, game, save, and backup roots used by a single write operation.
    pub fn admit_roots(
        &self,
        roots: SandboxWriteRoots,
    ) -> Result<SandboxWriteAdmission<'_>, SandboxWriteCapabilityError> {
        self.revalidate()?;
        for path in roots.all_paths() {
            self.admit_root(path)?;
        }
        Ok(SandboxWriteAdmission {
            capability: self,
            roots,
        })
    }

    fn admit_root(&self, path: &Path) -> Result<(), SandboxWriteCapabilityError> {
        if !is_safe_absolute_path(path)
            || !is_canonically_within(path, &self.root)
            || !path_components_are_real(&self.root, path)?
        {
            return Err(SandboxWriteCapabilityError::WriteRootRejected);
        }
        Ok(())
    }
}

/// Roots that a lifecycle operation may write. Optional roots are omitted when the operation does
/// not use save or backup storage; omitted roots are not implicitly guessed.
///
/// `app_data` is optional because the GUI composition admits writes with the app-data root
/// **exempt** from sandbox containment (see `game_root_only`). An omitted root is never
/// guessed, so exempting it must be an explicit choice at the construction site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxWriteRoots {
    app_data: Option<PathBuf>,
    game: PathBuf,
    save: Option<PathBuf>,
    backup: Option<PathBuf>,
}

impl SandboxWriteRoots {
    pub fn new(app_data: PathBuf, game: PathBuf) -> Self {
        Self {
            app_data: Some(app_data),
            game,
            save: None,
            backup: None,
        }
    }

    /// Admits only the game root, leaving the app-data root outside the admission set.
    ///
    /// #273: the GUI resolves its app-data root through Tauri to the OS location
    /// (`%APPDATA%\dev.helsincy.modmanager`) and never relocates it to the sandbox root, so
    /// requiring it to be inside the sandbox made GUI writes structurally impossible — the
    /// sandbox root and the app-data root are disjoint by construction.
    ///
    /// The app-data root holds manager metadata (database, import cache, config), not player
    /// data, and it is the game root that sandbox mode exists to protect. This deliberately
    /// weakens "every write inside sandbox mode goes through containment" and was accepted as
    /// a trade-off over relocating the production data root (see issue #273).
    pub fn game_root_only(game: PathBuf) -> Self {
        Self {
            app_data: None,
            game,
            save: None,
            backup: None,
        }
    }

    pub fn with_save_root(mut self, save: PathBuf) -> Self {
        self.save = Some(save);
        self
    }

    pub fn with_backup_root(mut self, backup: PathBuf) -> Self {
        self.backup = Some(backup);
        self
    }

    pub fn app_data_root(&self) -> Option<&Path> {
        self.app_data.as_deref()
    }

    pub fn game_root(&self) -> &Path {
        &self.game
    }

    pub fn save_root(&self) -> Option<&Path> {
        self.save.as_deref()
    }

    pub fn backup_root(&self) -> Option<&Path> {
        self.backup.as_deref()
    }

    fn all_paths(&self) -> impl Iterator<Item = &Path> {
        self.app_data
            .as_deref()
            .into_iter()
            .chain(std::iter::once(self.game.as_path()))
            .chain(self.save.as_deref())
            .chain(self.backup.as_deref())
    }
}

/// A validated set of roots tied by lifetime to the capability that admitted them.
#[derive(Debug)]
pub struct SandboxWriteAdmission<'a> {
    capability: &'a SandboxWriteCapability,
    roots: SandboxWriteRoots,
}

impl SandboxWriteAdmission<'_> {
    pub fn roots(&self) -> &SandboxWriteRoots {
        &self.roots
    }

    pub fn revalidate(&self) -> Result<(), SandboxWriteCapabilityError> {
        self.capability.revalidate()?;
        for path in self.roots.all_paths() {
            self.capability.admit_root(path)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxWriteCapabilityError {
    ProductionForbidden,
    RootUnsafe,
    RootUnavailable,
    RootNotDirectory,
    RootIdentityUnavailable,
    RootReplaced,
    RootLinkRejected,
    MarkerRequired,
    MarkerInvalid,
    MarkerUnavailable,
    WriteRootRejected,
}

impl SandboxWriteCapabilityError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::ProductionForbidden => "sandbox_write_production_forbidden",
            Self::RootUnsafe => "sandbox_root_unsafe",
            Self::RootUnavailable => "sandbox_root_unavailable",
            Self::RootNotDirectory => "sandbox_root_not_directory",
            Self::RootIdentityUnavailable => "sandbox_root_identity_unavailable",
            Self::RootReplaced => "sandbox_root_replaced",
            Self::RootLinkRejected => "sandbox_root_link_rejected",
            Self::MarkerRequired => "sandbox_marker_required",
            Self::MarkerInvalid => "sandbox_marker_invalid",
            Self::MarkerUnavailable => "sandbox_marker_unavailable",
            Self::WriteRootRejected => "sandbox_write_root_rejected",
        }
    }
}

impl fmt::Display for SandboxWriteCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SandboxWriteCapabilityError {}

fn open_root_nofollow(path: &Path) -> Result<Dir, SandboxWriteCapabilityError> {
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or(SandboxWriteCapabilityError::RootUnavailable)?;
    let name = path
        .file_name()
        .ok_or(SandboxWriteCapabilityError::RootUnavailable)?;
    let parent = Dir::open_ambient_dir(parent_path, ambient_authority())
        .map_err(|_| SandboxWriteCapabilityError::RootUnavailable)?;
    let directory = parent
        .open_dir_nofollow(name)
        .map_err(|_| SandboxWriteCapabilityError::RootLinkRejected)?;
    let metadata = directory
        .dir_metadata()
        .map_err(|_| SandboxWriteCapabilityError::RootUnavailable)?;
    ensure_real_directory(&metadata)?;
    Ok(directory)
}

fn initialize_or_validate_marker(directory: &Dir) -> Result<(), SandboxWriteCapabilityError> {
    let entries = directory
        .entries()
        .map_err(|_| SandboxWriteCapabilityError::RootUnavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| SandboxWriteCapabilityError::RootUnavailable)?;
    let has_marker = entries
        .iter()
        .any(|entry| entry.file_name() == Path::new(MARKER_NAME).as_os_str());

    if entries.is_empty() {
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let mut marker = directory
            .open_with(MARKER_NAME, &options)
            .map_err(|_| SandboxWriteCapabilityError::MarkerUnavailable)?;
        marker
            .write_all(SANDBOX_MARKER_SCHEMA.as_bytes())
            .and_then(|_| marker.sync_all())
            .map_err(|_| SandboxWriteCapabilityError::MarkerUnavailable)?;
    } else if !has_marker {
        return Err(SandboxWriteCapabilityError::MarkerRequired);
    }

    validate_marker(directory)
}

fn validate_marker(directory: &Dir) -> Result<(), SandboxWriteCapabilityError> {
    let metadata = directory.symlink_metadata(MARKER_NAME).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            SandboxWriteCapabilityError::MarkerRequired
        } else {
            SandboxWriteCapabilityError::MarkerUnavailable
        }
    })?;
    ensure_regular_file(&metadata)?;

    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let marker = directory
        .open_with(MARKER_NAME, &options)
        .map_err(|_| SandboxWriteCapabilityError::MarkerInvalid)?;
    let mut contents = Vec::new();
    marker
        .take((SANDBOX_MARKER_SCHEMA.len() + 1) as u64)
        .read_to_end(&mut contents)
        .map_err(|_| SandboxWriteCapabilityError::MarkerInvalid)?;
    if contents == SANDBOX_MARKER_SCHEMA.as_bytes() {
        Ok(())
    } else {
        Err(SandboxWriteCapabilityError::MarkerInvalid)
    }
}

fn ensure_real_directory(metadata: &Metadata) -> Result<(), SandboxWriteCapabilityError> {
    if !metadata.is_dir() || is_link_or_reparse(metadata) {
        Err(SandboxWriteCapabilityError::RootNotDirectory)
    } else {
        Ok(())
    }
}

fn ensure_regular_file(metadata: &Metadata) -> Result<(), SandboxWriteCapabilityError> {
    if !metadata.is_file() || is_link_or_reparse(metadata) {
        Err(SandboxWriteCapabilityError::MarkerInvalid)
    } else {
        Ok(())
    }
}

fn reject_link_or_reparse_chain(path: &Path) -> Result<(), SandboxWriteCapabilityError> {
    let mut components = path.ancestors().collect::<Vec<_>>();
    components.reverse();
    for ancestor in components {
        let metadata = fs::symlink_metadata(ancestor)
            .map_err(|_| SandboxWriteCapabilityError::RootUnavailable)?;
        if is_std_link_or_reparse(&metadata) {
            return Err(SandboxWriteCapabilityError::RootLinkRejected);
        }
    }
    Ok(())
}

fn path_components_are_real(root: &Path, path: &Path) -> Result<bool, SandboxWriteCapabilityError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| SandboxWriteCapabilityError::WriteRootRejected)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Ok(false);
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if is_std_link_or_reparse(&metadata) {
                    return Ok(false);
                }
                if !metadata.is_dir() {
                    return Ok(false);
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => break,
            Err(_) => return Err(SandboxWriteCapabilityError::WriteRootRejected),
        }
    }
    Ok(true)
}

fn is_link_or_reparse(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt as _;
        metadata.file_attributes() & REPARSE_POINT_ATTRIBUTE != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn is_std_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        metadata.file_attributes() & REPARSE_POINT_ATTRIBUTE != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = left.to_string_lossy().replace('\\', "/");
    let right = right.to_string_lossy().replace('\\', "/");
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        left.eq_ignore_ascii_case(&right)
    } else {
        left == right
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DirectoryIdentity {
    #[cfg(windows)]
    Windows {
        volume_serial_number: u32,
        file_index: u64,
    },
    #[cfg(unix)]
    Unix { dev: u64, ino: u64 },
    #[cfg(not(any(windows, unix)))]
    Unsupported,
}

impl DirectoryIdentity {
    fn from_directory(directory: &Dir) -> Option<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle as _;
            use windows_sys::Win32::Storage::FileSystem::{
                GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
            };

            let mut information = BY_HANDLE_FILE_INFORMATION::default();
            let succeeded = unsafe {
                GetFileInformationByHandle(
                    directory.as_raw_handle() as windows_sys::Win32::Foundation::HANDLE,
                    &mut information,
                )
            };
            if succeeded == 0 {
                return None;
            }
            Some(Self::Windows {
                volume_serial_number: information.dwVolumeSerialNumber,
                file_index: (u64::from(information.nFileIndexHigh) << 32)
                    | u64::from(information.nFileIndexLow),
            })
        }
        #[cfg(unix)]
        {
            use cap_std::fs::MetadataExt as _;
            let metadata = directory.dir_metadata().ok()?;
            Some(Self::Unix {
                dev: metadata.dev(),
                ino: metadata.ino(),
            })
        }
        #[cfg(not(any(windows, unix)))]
        {
            Some(Self::Unsupported)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeEnvironmentKind;
    use std::fs;
    use tempfile::TempDir;

    fn environment(root: &Path) -> RuntimeEnvironment {
        RuntimeEnvironment::sandbox(root.to_path_buf()).expect("sandbox environment")
    }

    fn empty_sandbox() -> TempDir {
        tempfile::tempdir().expect("sandbox temp dir")
    }

    #[cfg(unix)]
    fn create_directory_link(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create directory symlink");
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
            .expect("mklink");
        assert!(
            output.status.success(),
            "mklink failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[cfg(unix)]
    fn remove_directory_link(link: &Path) {
        fs::remove_file(link).expect("remove directory symlink");
    }

    #[cfg(windows)]
    fn remove_directory_link(link: &Path) {
        fs::remove_dir(link).expect("remove directory junction");
    }

    #[test]
    fn empty_root_gets_a_versioned_marker_only_when_write_capability_is_requested() {
        let sandbox = empty_sandbox();
        assert!(!sandbox.path().join(SANDBOX_MARKER_FILE_NAME).exists());

        let capability = environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect("sandbox write capability");

        assert_eq!(
            fs::read_to_string(sandbox.path().join(SANDBOX_MARKER_FILE_NAME)).expect("read marker"),
            SANDBOX_MARKER_SCHEMA
        );
        capability.revalidate().expect("revalidate capability");
    }

    #[test]
    fn production_never_creates_or_acquires_a_write_capability() {
        assert_eq!(
            RuntimeEnvironment::from_options(RuntimeEnvironmentKind::Production, None)
                .expect("production")
                .acquire_sandbox_write_capability()
                .expect_err("production must be rejected")
                .code(),
            "sandbox_write_production_forbidden"
        );
    }

    #[test]
    fn nonempty_unmarked_root_fails_closed_without_touching_sentinel() {
        let sandbox = empty_sandbox();
        let sentinel = sandbox.path().join("sentinel");
        fs::write(&sentinel, b"untouched").expect("sentinel");

        let error = environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect_err("unmarked root must be rejected");

        assert_eq!(error, SandboxWriteCapabilityError::MarkerRequired);
        assert_eq!(fs::read(&sentinel).expect("sentinel"), b"untouched");
        assert!(!sandbox.path().join(SANDBOX_MARKER_FILE_NAME).exists());
    }

    #[test]
    fn malformed_marker_fails_closed() {
        let sandbox = empty_sandbox();
        fs::write(
            sandbox.path().join(SANDBOX_MARKER_FILE_NAME),
            b"{\"kind\":\"other\"}\n",
        )
        .expect("marker");

        let error = environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect_err("malformed marker must be rejected");

        assert_eq!(error, SandboxWriteCapabilityError::MarkerInvalid);
    }

    #[test]
    fn existing_marker_allows_a_nonempty_artificial_fixture() {
        let sandbox = empty_sandbox();
        drop(
            environment(sandbox.path())
                .acquire_sandbox_write_capability()
                .expect("initialize marker"),
        );
        fs::create_dir(sandbox.path().join("fixture")).expect("fixture");

        environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect("reacquire marked sandbox");
    }

    #[test]
    fn marker_replacement_invalidates_the_existing_capability() {
        let sandbox = empty_sandbox();
        let outside = tempfile::tempdir().expect("outside");
        let sentinel = outside.path().join("sentinel");
        fs::write(&sentinel, b"untouched").expect("sentinel");
        let capability = environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect("capability");
        fs::write(
            sandbox.path().join(SANDBOX_MARKER_FILE_NAME),
            b"{\"kind\":\"other\"}\n",
        )
        .expect("replace marker");

        assert_eq!(
            capability.revalidate(),
            Err(SandboxWriteCapabilityError::MarkerInvalid)
        );
        assert_eq!(fs::read(&sentinel).expect("sentinel"), b"untouched");
    }

    #[test]
    fn linked_marker_is_rejected_without_touching_outside_sentinel() {
        let sandbox = empty_sandbox();
        let outside = tempfile::tempdir().expect("outside");
        let sentinel = outside.path().join("sentinel");
        let linked_marker = sandbox.path().join(SANDBOX_MARKER_FILE_NAME);
        fs::write(&sentinel, b"untouched").expect("sentinel");
        create_directory_link(outside.path(), &linked_marker);

        let error = environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect_err("linked marker must be rejected");

        assert_eq!(error, SandboxWriteCapabilityError::MarkerInvalid);
        assert_eq!(fs::read(&sentinel).expect("sentinel"), b"untouched");
        remove_directory_link(&linked_marker);
    }

    #[test]
    fn linked_sandbox_root_is_rejected_without_touching_outside_sentinel() {
        let host = tempfile::tempdir().expect("host");
        let outside = tempfile::tempdir().expect("outside");
        let sentinel = outside.path().join("sentinel");
        let linked_root = host.path().join("sandbox");
        fs::write(&sentinel, b"untouched").expect("sentinel");
        create_directory_link(outside.path(), &linked_root);

        let error = environment(&linked_root)
            .acquire_sandbox_write_capability()
            .expect_err("linked sandbox root must be rejected");

        assert_eq!(error, SandboxWriteCapabilityError::RootLinkRejected);
        assert_eq!(fs::read(&sentinel).expect("sentinel"), b"untouched");
        remove_directory_link(&linked_root);
    }

    #[test]
    fn all_declared_write_roots_must_stay_inside_the_capability_root() {
        let sandbox = empty_sandbox();
        let capability = environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect("capability");
        let game = sandbox.path().join("game");
        let save = sandbox.path().join("save");
        let backup = sandbox.path().join("backup");
        fs::create_dir_all(&game).expect("game");
        fs::create_dir_all(&save).expect("save");
        fs::create_dir_all(&backup).expect("backup");

        capability
            .admit_roots(
                SandboxWriteRoots::new(sandbox.path().to_path_buf(), game)
                    .with_save_root(save)
                    .with_backup_root(backup),
            )
            .expect("roots inside sandbox");

        let outside = tempfile::tempdir().expect("outside");
        let error = capability
            .admit_roots(SandboxWriteRoots::new(
                sandbox.path().to_path_buf(),
                outside.path().to_path_buf(),
            ))
            .expect_err("outside game root must be rejected");
        assert_eq!(error, SandboxWriteCapabilityError::WriteRootRejected);

        let lexical_escape = sandbox.path().join("game").join("..").join("escape");
        let error = capability
            .admit_roots(SandboxWriteRoots::new(
                sandbox.path().to_path_buf(),
                lexical_escape,
            ))
            .expect_err("lexical escape must be rejected");
        assert_eq!(error, SandboxWriteCapabilityError::WriteRootRejected);
    }

    #[test]
    fn game_root_only_leaves_the_app_data_root_out_of_the_admitted_set() {
        let sandbox = empty_sandbox();
        let game = sandbox.path().join("game");
        let roots = SandboxWriteRoots::game_root_only(game.clone());
        assert_eq!(roots.app_data_root(), None);
        assert_eq!(roots.game_root(), game.as_path());

        let with_app_data = SandboxWriteRoots::new(sandbox.path().to_path_buf(), game);
        assert_eq!(with_app_data.app_data_root(), Some(sandbox.path()));
    }

    #[test]
    fn gui_composition_admission_survives_an_app_data_root_outside_the_sandbox() {
        // #273: the GUI resolves its app-data root through Tauri to the OS location and never
        // relocates it, so the sandbox root and the app-data root are disjoint by construction.
        // Requiring the app-data root to be contained rejected every GUI install with
        // `install_retarget_failed:write_safety_rejected`.
        //
        // The two shapes are compared against the same capability so the control group lives
        // inside this test: the relaxation is exactly the difference between them, not a
        // blanket disabling of the gate (see the next test for that).
        let sandbox = empty_sandbox();
        let capability = environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect("capability");
        let game = sandbox.path().join("game");
        fs::create_dir_all(&game).expect("game");
        let system_app_data = tempfile::tempdir().expect("system app data root");
        assert!(
            !system_app_data.path().starts_with(sandbox.path()),
            "fixture must place the app-data root outside the sandbox"
        );

        let rejected = capability
            .admit_roots(SandboxWriteRoots::new(
                system_app_data.path().to_path_buf(),
                game.clone(),
            ))
            .expect_err("the batch/CLI shape must still reject an outside app-data root");
        assert_eq!(rejected, SandboxWriteCapabilityError::WriteRootRejected);

        capability
            .admit_roots(SandboxWriteRoots::game_root_only(game))
            .expect("the GUI shape must not need the app-data root inside the sandbox")
            .revalidate()
            .expect("revalidation stays within the admitted set");
    }

    #[test]
    fn game_root_only_admission_still_rejects_a_game_root_outside_the_sandbox() {
        // Control group for the #273 relaxation: exempting the app-data root must not turn the
        // gate off. The game root is what sandbox mode exists to protect.
        let sandbox = empty_sandbox();
        let capability = environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect("capability");
        let outside_game = tempfile::tempdir().expect("outside game root");

        let error = capability
            .admit_roots(SandboxWriteRoots::game_root_only(
                outside_game.path().to_path_buf(),
            ))
            .expect_err("a game root outside the sandbox must still be rejected");
        assert_eq!(error, SandboxWriteCapabilityError::WriteRootRejected);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_write_root_is_rejected_without_touching_outside_sentinel() {
        let sandbox = empty_sandbox();
        let capability = environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect("capability");
        let outside = tempfile::tempdir().expect("outside");
        let sentinel = outside.path().join("sentinel");
        fs::write(&sentinel, b"untouched").expect("sentinel");
        let link = sandbox.path().join("game");
        std::os::unix::fs::symlink(outside.path(), &link).expect("game symlink");

        let error = capability
            .admit_roots(SandboxWriteRoots::new(sandbox.path().to_path_buf(), link))
            .expect_err("symlink game root must be rejected");

        assert_eq!(error, SandboxWriteCapabilityError::WriteRootRejected);
        assert_eq!(fs::read(&sentinel).expect("sentinel"), b"untouched");
    }

    #[cfg(windows)]
    #[test]
    fn junction_write_root_is_rejected_without_touching_outside_sentinel() {
        let sandbox = empty_sandbox();
        let capability = environment(sandbox.path())
            .acquire_sandbox_write_capability()
            .expect("capability");
        let outside = tempfile::tempdir().expect("outside");
        let sentinel = outside.path().join("sentinel");
        fs::write(&sentinel, b"untouched").expect("sentinel");
        let link = sandbox.path().join("game");
        create_directory_link(outside.path(), &link);

        let error = capability
            .admit_roots(SandboxWriteRoots::new(
                sandbox.path().to_path_buf(),
                link.clone(),
            ))
            .expect_err("junction game root must be rejected");

        assert_eq!(error, SandboxWriteCapabilityError::WriteRootRejected);
        assert_eq!(fs::read(&sentinel).expect("sentinel"), b"untouched");
        remove_directory_link(&link);
    }

    #[test]
    fn ancestor_replacement_is_blocked_or_invalidates_the_existing_capability() {
        let parent = tempfile::tempdir().expect("parent");
        let ancestor = parent.path().join("ancestor");
        let sandbox = ancestor.join("sandbox");
        let moved = parent.path().join("moved-ancestor");
        let outside_sentinel = parent.path().join("outside-sentinel");
        fs::create_dir(&ancestor).expect("ancestor");
        fs::create_dir(&sandbox).expect("sandbox");
        fs::write(&outside_sentinel, b"untouched").expect("outside sentinel");
        let capability = environment(&sandbox)
            .acquire_sandbox_write_capability()
            .expect("capability");

        let replacement_result = fs::rename(&ancestor, &moved);

        #[cfg(windows)]
        {
            replacement_result.expect_err("open capability must block ancestor replacement");
            capability
                .revalidate()
                .expect("original capability remains valid");
        }
        #[cfg(not(windows))]
        {
            replacement_result.expect("move sandbox ancestor");
            fs::create_dir(&ancestor).expect("replacement ancestor");
            fs::create_dir(&sandbox).expect("replacement sandbox");
            fs::write(
                sandbox.join(SANDBOX_MARKER_FILE_NAME),
                SANDBOX_MARKER_SCHEMA,
            )
            .expect("replacement marker");
            assert_eq!(
                capability.revalidate(),
                Err(SandboxWriteCapabilityError::RootReplaced)
            );
        }
        assert_eq!(
            fs::read(&outside_sentinel).expect("outside sentinel"),
            b"untouched"
        );
    }
}
