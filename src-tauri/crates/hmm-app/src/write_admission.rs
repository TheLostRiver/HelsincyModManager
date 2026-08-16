use crate::{TaskManager, TaskStatus};
use hmm_core::{GameId, ProfileId};
use hmm_ports::{
    CancellationToken, CrossProcessWriteAcquisition, CrossProcessWriteAdmission,
    CrossProcessWriteAdmissionError, CrossProcessWriteAdmissionResult, CrossProcessWriteGuard,
    CrossProcessWriteScope, NeverCancelled,
};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const DEFAULT_CROSS_PROCESS_WRITE_ADMISSION_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct CrossProcessWriteAdmissionCoordinator {
    admission: Arc<dyn CrossProcessWriteAdmission>,
    timeout: Duration,
}

impl fmt::Debug for CrossProcessWriteAdmissionCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CrossProcessWriteAdmissionCoordinator")
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl CrossProcessWriteAdmissionCoordinator {
    pub fn new(admission: Arc<dyn CrossProcessWriteAdmission>) -> Self {
        Self::with_timeout(admission, DEFAULT_CROSS_PROCESS_WRITE_ADMISSION_TIMEOUT)
    }

    pub fn with_timeout(admission: Arc<dyn CrossProcessWriteAdmission>, timeout: Duration) -> Self {
        Self { admission, timeout }
    }

    pub(crate) fn process_local_compatibility() -> Self {
        Self::with_timeout(Arc::new(ProcessLocalCompatibilityAdmission), Duration::ZERO)
    }

    pub fn acquire_background_registration(
        &self,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        self.acquire(
            &CrossProcessWriteScope::background_registration(),
            &NeverCancelled,
        )
    }

    pub fn acquire_save_profile(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        cancellation: &dyn CancellationToken,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        self.acquire(
            &CrossProcessWriteScope::save_profile(game_id, profile_id),
            cancellation,
        )
    }

    pub fn acquire_game_profile(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        cancellation: &dyn CancellationToken,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        self.acquire(
            &CrossProcessWriteScope::game_profile(game_id, profile_id),
            cancellation,
        )
    }

    pub fn acquire_save_profile_for_task(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        task_manager: &TaskManager,
        task_id: &str,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        self.acquire_save_profile(
            game_id,
            profile_id,
            &TaskCancellationToken {
                task_manager,
                task_id,
            },
        )
    }

    pub fn acquire_game_profile_for_task(
        &self,
        game_id: &GameId,
        profile_id: &ProfileId,
        task_manager: &TaskManager,
        task_id: &str,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        self.acquire_game_profile(
            game_id,
            profile_id,
            &TaskCancellationToken {
                task_manager,
                task_id,
            },
        )
    }

    fn acquire(
        &self,
        scope: &CrossProcessWriteScope,
        cancellation: &dyn CancellationToken,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        let started_at = Instant::now();
        let result = self.admission.acquire(scope, self.timeout, cancellation);
        let wait_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
        match &result {
            Ok(guard) => tracing::debug!(
                event = "write_admission_acquired",
                scope = scope.kind().as_str(),
                wait_ms,
                recovery = guard
                    .acquisition()
                    .recovery
                    .map(|recovery| recovery.as_str())
                    .unwrap_or("none"),
                result = "success"
            ),
            Err(error) => tracing::warn!(
                event = "write_admission_acquire_failed",
                scope = scope.kind().as_str(),
                wait_ms,
                error_code = error.code(),
                result = "failure"
            ),
        }
        result
    }
}

struct TaskCancellationToken<'a> {
    task_manager: &'a TaskManager,
    task_id: &'a str,
}

impl CancellationToken for TaskCancellationToken<'_> {
    fn is_cancelled(&self) -> bool {
        self.task_manager.task_status(self.task_id) == Some(TaskStatus::Cancelled)
    }
}

struct ProcessLocalCompatibilityAdmission;

impl CrossProcessWriteAdmission for ProcessLocalCompatibilityAdmission {
    fn acquire(
        &self,
        scope: &CrossProcessWriteScope,
        _timeout: Duration,
        cancellation: &dyn CancellationToken,
    ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
        if cancellation.is_cancelled() {
            return Err(CrossProcessWriteAdmissionError::Cancelled);
        }
        Ok(Box::new(ProcessLocalCompatibilityGuard {
            scope: scope.clone(),
        }))
    }
}

struct ProcessLocalCompatibilityGuard {
    scope: CrossProcessWriteScope,
}

impl CrossProcessWriteGuard for ProcessLocalCompatibilityGuard {
    fn scope(&self) -> &CrossProcessWriteScope {
        &self.scope
    }

    fn acquisition(&self) -> CrossProcessWriteAcquisition {
        CrossProcessWriteAcquisition::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingAdmission {
        calls: Mutex<Vec<(CrossProcessWriteScope, Duration)>>,
        error: Option<CrossProcessWriteAdmissionError>,
    }

    impl CrossProcessWriteAdmission for RecordingAdmission {
        fn acquire(
            &self,
            scope: &CrossProcessWriteScope,
            timeout: Duration,
            _cancellation: &dyn CancellationToken,
        ) -> CrossProcessWriteAdmissionResult<Box<dyn CrossProcessWriteGuard>> {
            self.calls
                .lock()
                .expect("recording calls")
                .push((scope.clone(), timeout));
            if let Some(error) = self.error {
                return Err(error);
            }
            Ok(Box::new(ProcessLocalCompatibilityGuard {
                scope: scope.clone(),
            }))
        }
    }

    #[test]
    fn coordinator_uses_stable_scope_and_injected_timeout() {
        let admission = Arc::new(RecordingAdmission::default());
        let coordinator = CrossProcessWriteAdmissionCoordinator::with_timeout(
            admission.clone(),
            Duration::from_millis(42),
        );
        let game_id = GameId::mhw();
        let profile_id = ProfileId::new("profile-a");

        let guard = coordinator
            .acquire_game_profile(&game_id, &profile_id, &NeverCancelled)
            .expect("acquire game scope");
        assert_eq!(
            guard.scope(),
            &CrossProcessWriteScope::game_profile(&game_id, &profile_id)
        );
        assert_eq!(
            *admission.calls.lock().expect("recording calls"),
            vec![(
                CrossProcessWriteScope::game_profile(&game_id, &profile_id),
                Duration::from_millis(42),
            )]
        );
    }

    #[test]
    fn task_cancellation_is_forwarded_before_compatibility_acquisition() {
        let task_manager = TaskManager::new();
        let task = task_manager
            .create_task(crate::TaskKind::Install)
            .expect("create task");
        task_manager
            .cancel_task(&task.task_id)
            .expect("cancel task");
        let coordinator = CrossProcessWriteAdmissionCoordinator::process_local_compatibility();

        let error = match coordinator.acquire_game_profile_for_task(
            &GameId::mhw(),
            &ProfileId::new("profile-a"),
            &task_manager,
            &task.task_id,
        ) {
            Ok(_) => panic!("cancelled task must not acquire"),
            Err(error) => error,
        };
        assert_eq!(error, CrossProcessWriteAdmissionError::Cancelled);
    }
}
