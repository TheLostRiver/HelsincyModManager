use super::{
    namespace_digest, scope_digest, HeldScopeOrderGuard,
    PlatformCrossProcessWriteAdmissionInitError,
};
use crate::controlled_fs::{open_existing_directory_nofollow, open_or_create_child_directory};
use cap_fs_ext::{DirExt as _, FollowSymlinks, OpenOptionsFollowExt as _};
use cap_std::fs::{Dir, OpenOptions, OpenOptionsExt as _};
use fs2::FileExt;
use hmm_ports::{
    CancellationToken, CrossProcessWriteAcquisition, CrossProcessWriteAdmission,
    CrossProcessWriteAdmissionError, CrossProcessWriteAdmissionResult, CrossProcessWriteGuard,
    CrossProcessWriteRecovery, CrossProcessWriteScope,
};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::time::{Duration, Instant};

const LOCK_DIRECTORY_NAME: &str = "write-admission";
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(25);
const MAX_OWNER_RECORD_BYTES: u64 = 512;

pub struct PlatformCrossProcessWriteAdmission {
    app_data_dir: Dir,
    namespace: String,
    #[cfg(test)]
    lock_root_path: PathBuf,
}

impl PlatformCrossProcessWriteAdmission {
    pub fn new(app_data_dir: &Path) -> Result<Self, PlatformCrossProcessWriteAdmissionInitError> {
        let metadata = fs::symlink_metadata(app_data_dir)
            .map_err(|_| PlatformCrossProcessWriteAdmissionInitError::NamespaceUnavailable)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(PlatformCrossProcessWriteAdmissionInitError::NamespaceUnavailable);
        }
        let app_data_dir_path = fs::canonicalize(app_data_dir)
            .map_err(|_| PlatformCrossProcessWriteAdmissionInitError::NamespaceUnavailable)?;
        let app_data_dir = open_existing_directory_nofollow(
            &app_data_dir_path,
            "write admission app-data namespace",
        )
        .map_err(|_| PlatformCrossProcessWriteAdmissionInitError::NamespaceUnavailable)?;
        let lock_root = open_or_create_child_directory(
            &app_data_dir,
            OsStr::new(LOCK_DIRECTORY_NAME),
            "write admission lock directory",
        )
        .map_err(|_| PlatformCrossProcessWriteAdmissionInitError::NamespaceUnavailable)?;
        drop(lock_root);
        let namespace = namespace_digest(&[app_data_dir_path.as_os_str().as_bytes()]);
        Ok(Self {
            app_data_dir,
            namespace,
            #[cfg(test)]
            lock_root_path: app_data_dir_path.join(LOCK_DIRECTORY_NAME),
        })
    }

    fn open_lock_root(&self) -> CrossProcessWriteAdmissionResult<Dir> {
        self.app_data_dir
            .open_dir_nofollow(LOCK_DIRECTORY_NAME)
            .map_err(|_| CrossProcessWriteAdmissionError::Unavailable)
    }

    fn open_lock_file(
        &self,
        scope: &CrossProcessWriteScope,
    ) -> CrossProcessWriteAdmissionResult<File> {
        let lock_root = self.open_lock_root()?;
        open_lock_file_from_root(&lock_root, scope)
    }
}

fn open_lock_file_from_root(
    lock_root: &Dir,
    scope: &CrossProcessWriteScope,
) -> CrossProcessWriteAdmissionResult<File> {
    let mut options = OpenOptions::new();
    options
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_CLOEXEC)
        .follow(FollowSymlinks::No);
    let file = lock_root
        .open_with(format!("{}.lock", scope_digest(scope)), &options)
        .map(cap_std::fs::File::into_std)
        .map_err(|_| CrossProcessWriteAdmissionError::Unavailable)?;
    let metadata = file
        .metadata()
        .map_err(|_| CrossProcessWriteAdmissionError::Unavailable)?;
    if !metadata.is_file() {
        return Err(CrossProcessWriteAdmissionError::Unavailable);
    }
    Ok(file)
}

impl CrossProcessWriteAdmission for PlatformCrossProcessWriteAdmission {
    fn acquire(
        &self,
        scope: &CrossProcessWriteScope,
        timeout: Duration,
        cancellation: &dyn CancellationToken,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        let order_key = HeldScopeOrderGuard::validate(&self.namespace, scope)
            .map_err(|_| CrossProcessWriteAdmissionError::OrderViolation)?;
        let mut file = self.open_lock_file(scope)?;
        let started_at = Instant::now();
        loop {
            if cancellation.is_cancelled() {
                return Err(CrossProcessWriteAdmissionError::Cancelled);
            }
            match file.try_lock_exclusive() {
                Ok(()) => break,
                Err(error) if is_lock_contended(&error) => {
                    if started_at.elapsed() >= timeout {
                        return Err(CrossProcessWriteAdmissionError::Busy);
                    }
                    std::thread::sleep(
                        LOCK_POLL_INTERVAL.min(timeout.saturating_sub(started_at.elapsed())),
                    );
                }
                Err(_) => return Err(CrossProcessWriteAdmissionError::Unavailable),
            }
        }

        let recovery = read_stale_owner_evidence(&mut file)?;
        write_owner_record(&mut file).map_err(|_| {
            let _ = FileExt::unlock(&file);
            CrossProcessWriteAdmissionError::Unavailable
        })?;
        let order_guard = HeldScopeOrderGuard::register(&self.namespace, order_key);
        if let Some(recovery) = recovery {
            tracing::warn!(
                event = "write_admission_owner_recovered",
                scope = scope.kind().as_str(),
                recovery = recovery.as_str(),
                result = "success"
            );
        }
        Ok(Box::new(FileLockWriteGuard {
            file,
            scope: scope.clone(),
            acquisition: CrossProcessWriteAcquisition { recovery },
            order_guard: Some(order_guard),
        }))
    }
}

struct FileLockWriteGuard {
    file: File,
    scope: CrossProcessWriteScope,
    acquisition: CrossProcessWriteAcquisition,
    order_guard: Option<HeldScopeOrderGuard>,
}

impl CrossProcessWriteGuard for FileLockWriteGuard {
    fn scope(&self) -> &CrossProcessWriteScope {
        &self.scope
    }

    fn acquisition(&self) -> CrossProcessWriteAcquisition {
        self.acquisition
    }
}

impl Drop for FileLockWriteGuard {
    fn drop(&mut self) {
        let cleared = self
            .file
            .set_len(0)
            .and_then(|_| self.file.seek(SeekFrom::Start(0)).map(|_| ()))
            .and_then(|_| self.file.sync_data());
        if cleared.is_err() {
            tracing::error!(
                event = "write_admission_release_failed",
                scope = self.scope.kind().as_str(),
                stage = "owner_metadata",
                result = "failure"
            );
        }
        if FileExt::unlock(&self.file).is_err() {
            tracing::error!(
                event = "write_admission_release_failed",
                scope = self.scope.kind().as_str(),
                stage = "platform_unlock",
                result = "failure"
            );
        }
        drop(self.order_guard.take());
    }
}

fn read_stale_owner_evidence(
    file: &mut File,
) -> CrossProcessWriteAdmissionResult<Option<CrossProcessWriteRecovery>> {
    file.seek(SeekFrom::Start(0))
        .map_err(|_| CrossProcessWriteAdmissionError::Unavailable)?;
    let mut bytes = Vec::new();
    file.take(MAX_OWNER_RECORD_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CrossProcessWriteAdmissionError::Unavailable)?;
    Ok((!bytes.is_empty()).then_some(CrossProcessWriteRecovery::StaleOwnerMetadata))
}

fn write_owner_record(file: &mut File) -> std::io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    writeln!(
        file,
        "v1 pid={} owner={}",
        std::process::id(),
        uuid::Uuid::new_v4()
    )?;
    file.sync_data()
}

fn is_lock_contended(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
        || error.raw_os_error() == fs2::lock_contended_error().raw_os_error()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameId, ProfileId};
    use hmm_ports::NeverCancelled;
    use std::os::unix::fs::symlink;

    #[test]
    fn same_process_reentry_is_rejected_before_platform_wait() {
        let temp = tempfile::tempdir().expect("temp dir");
        let admission = PlatformCrossProcessWriteAdmission::new(temp.path()).expect("admission");
        let scope =
            CrossProcessWriteScope::save_profile(&GameId::mhw(), &ProfileId::new("profile-a"));
        let guard = admission
            .acquire(&scope, Duration::ZERO, &NeverCancelled)
            .expect("first guard");
        let error = match admission.acquire(&scope, Duration::ZERO, &NeverCancelled) {
            Ok(_) => panic!("reentry must be rejected"),
            Err(error) => error,
        };
        assert_eq!(error, CrossProcessWriteAdmissionError::OrderViolation);
        drop(guard);
    }

    #[test]
    fn replaced_lock_root_symlink_fails_closed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let external = temp.path().join("external");
        fs::create_dir(&external).expect("external directory");
        let admission = PlatformCrossProcessWriteAdmission::new(temp.path()).expect("admission");
        fs::remove_dir(&admission.lock_root_path).expect("remove original lock root");
        symlink(&external, &admission.lock_root_path).expect("replace lock root with symlink");

        let error = match admission.acquire(
            &CrossProcessWriteScope::background_registration(),
            Duration::ZERO,
            &NeverCancelled,
        ) {
            Ok(_) => panic!("symlinked lock root must be rejected"),
            Err(error) => error,
        };
        assert_eq!(error, CrossProcessWriteAdmissionError::Unavailable);
    }

    #[test]
    fn replaced_lock_file_symlink_fails_closed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let admission = PlatformCrossProcessWriteAdmission::new(temp.path()).expect("admission");
        let scope = CrossProcessWriteScope::background_registration();
        let guard = admission
            .acquire(&scope, Duration::ZERO, &NeverCancelled)
            .expect("create owned lock file");
        drop(guard);

        let lock_path = fs::read_dir(&admission.lock_root_path)
            .expect("list lock root")
            .next()
            .expect("lock file exists")
            .expect("lock entry")
            .path();
        let external = temp.path().join("external-file");
        fs::write(&external, b"external").expect("external file");
        fs::remove_file(&lock_path).expect("remove owned lock file");
        symlink(&external, &lock_path).expect("replace lock file with symlink");

        let error = match admission.acquire(&scope, Duration::ZERO, &NeverCancelled) {
            Ok(_) => panic!("symlinked lock file must be rejected"),
            Err(error) => error,
        };
        assert_eq!(error, CrossProcessWriteAdmissionError::Unavailable);
    }

    #[test]
    fn opened_lock_root_capability_cannot_escape_after_path_replacement() {
        let temp = tempfile::tempdir().expect("temp dir");
        let external = temp.path().join("external");
        fs::create_dir(&external).expect("external directory");
        let admission = PlatformCrossProcessWriteAdmission::new(temp.path()).expect("admission");
        let lock_root = admission.open_lock_root().expect("opened lock root");

        fs::remove_dir(&admission.lock_root_path).expect("remove original lock root");
        symlink(&external, &admission.lock_root_path).expect("replace lock root with symlink");

        let file = open_lock_file_from_root(
            &lock_root,
            &CrossProcessWriteScope::background_registration(),
        )
        .expect("capability-relative lock file");
        drop(file);

        assert_eq!(
            fs::read_dir(&external)
                .expect("external directory remains readable")
                .count(),
            0
        );
    }
}
