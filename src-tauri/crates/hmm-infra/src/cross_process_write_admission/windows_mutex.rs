use super::{
    namespace_digest, scope_digest, HeldScopeOrderGuard,
    PlatformCrossProcessWriteAdmissionInitError,
};
use crate::windows_identity::current_process_user_sid;
use hmm_ports::{
    CancellationToken, CrossProcessWriteAcquisition, CrossProcessWriteAdmission,
    CrossProcessWriteAdmissionError, CrossProcessWriteAdmissionResult, CrossProcessWriteGuard,
    CrossProcessWriteRecovery, CrossProcessWriteScope,
};
use std::path::Path;
use std::time::{Duration, Instant};
use windows::core::HSTRING;
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};

const MUTEX_PREFIX: &str = "Global\\HelsincyModManager.WriteAdmission.v1";
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub struct PlatformCrossProcessWriteAdmission {
    namespace: String,
}

impl PlatformCrossProcessWriteAdmission {
    pub fn new(app_data_dir: &Path) -> Result<Self, PlatformCrossProcessWriteAdmissionInitError> {
        let metadata = std::fs::symlink_metadata(app_data_dir)
            .map_err(|_| PlatformCrossProcessWriteAdmissionInitError::NamespaceUnavailable)?;
        if !metadata.is_dir() {
            return Err(PlatformCrossProcessWriteAdmissionInitError::NamespaceUnavailable);
        }
        let canonical_root = std::fs::canonicalize(app_data_dir)
            .map_err(|_| PlatformCrossProcessWriteAdmissionInitError::NamespaceUnavailable)?;
        let user_sid = current_process_user_sid()
            .map_err(|_| PlatformCrossProcessWriteAdmissionInitError::IdentityUnavailable)?;
        let namespace = namespace_digest(&[
            canonical_root
                .to_string_lossy()
                .to_ascii_lowercase()
                .as_bytes(),
            user_sid.as_bytes(),
        ]);
        Ok(Self { namespace })
    }

    fn mutex_name(&self, scope: &CrossProcessWriteScope) -> String {
        format!("{MUTEX_PREFIX}.{}.{}", self.namespace, scope_digest(scope))
    }
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
        let name = HSTRING::from(self.mutex_name(scope));
        let handle = unsafe { CreateMutexW(None, false, &name) }
            .map_err(|_| CrossProcessWriteAdmissionError::Unavailable)?;
        let started_at = Instant::now();
        let recovery = loop {
            if cancellation.is_cancelled() {
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Err(CrossProcessWriteAdmissionError::Cancelled);
            }
            let wait_millis = wait_slice_millis(timeout, started_at.elapsed());
            let result = unsafe { WaitForSingleObject(handle, wait_millis) };
            if result == WAIT_OBJECT_0 {
                break None;
            }
            if result == WAIT_ABANDONED {
                break Some(CrossProcessWriteRecovery::AbandonedOwner);
            }
            if result != WAIT_TIMEOUT {
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Err(CrossProcessWriteAdmissionError::Unavailable);
            }
            if started_at.elapsed() >= timeout {
                unsafe {
                    let _ = CloseHandle(handle);
                }
                return Err(CrossProcessWriteAdmissionError::Busy);
            }
        };

        let order_guard = HeldScopeOrderGuard::register(&self.namespace, order_key);
        if let Some(recovery) = recovery {
            tracing::warn!(
                event = "write_admission_owner_recovered",
                scope = scope.kind().as_str(),
                recovery = recovery.as_str(),
                result = "success"
            );
        }
        Ok(Box::new(WindowsMutexWriteGuard {
            handle,
            scope: scope.clone(),
            acquisition: CrossProcessWriteAcquisition { recovery },
            order_guard: Some(order_guard),
        }))
    }
}

fn wait_slice_millis(timeout: Duration, elapsed: Duration) -> u32 {
    let remaining = timeout.saturating_sub(elapsed);
    if remaining.is_zero() {
        return 0;
    }
    WAIT_POLL_INTERVAL
        .min(remaining)
        .as_millis()
        .try_into()
        .unwrap_or(u32::MAX)
}

struct WindowsMutexWriteGuard {
    handle: HANDLE,
    scope: CrossProcessWriteScope,
    acquisition: CrossProcessWriteAcquisition,
    order_guard: Option<HeldScopeOrderGuard>,
}

impl CrossProcessWriteGuard for WindowsMutexWriteGuard {
    fn scope(&self) -> &CrossProcessWriteScope {
        &self.scope
    }

    fn acquisition(&self) -> CrossProcessWriteAcquisition {
        self.acquisition
    }
}

impl Drop for WindowsMutexWriteGuard {
    fn drop(&mut self) {
        if unsafe { ReleaseMutex(self.handle) }.is_err() {
            tracing::error!(
                event = "write_admission_release_failed",
                scope = self.scope.kind().as_str(),
                stage = "platform_unlock",
                result = "failure"
            );
        }
        if unsafe { CloseHandle(self.handle) }.is_err() {
            tracing::error!(
                event = "write_admission_release_failed",
                scope = self.scope.kind().as_str(),
                stage = "platform_handle_close",
                result = "failure"
            );
        }
        drop(self.order_guard.take());
    }
}
