use hmm_runtime::{
    LifecycleTaskCancellationHandle, TaskKind, TaskProgressEvent, TaskProgressObserver, TaskStatus,
};
use std::convert::Infallible;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const CANCELLED_PHASE: &str = "install.cancelled";

trait CliTaskCanceller: Send + Sync {
    fn cancel_task(&self, task_id: &str) -> bool;
}

impl CliTaskCanceller for LifecycleTaskCancellationHandle {
    fn cancel_task(&self, task_id: &str) -> bool {
        LifecycleTaskCancellationHandle::cancel_task(self, task_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CancellationSignalAction {
    WaitForTerminal,
    ForceExit,
}

pub(crate) struct CliCancellationCoordinator {
    canceller: Mutex<Option<Arc<dyn CliTaskCanceller>>>,
    signal_count: AtomicUsize,
    active_task_id: Mutex<Option<String>>,
    cancelled_task_id: Mutex<Option<String>>,
}

impl CliCancellationCoordinator {
    pub(crate) fn install() -> Result<Arc<Self>, ctrlc::Error> {
        let coordinator = Arc::new(Self::new(None));
        let handler = Arc::clone(&coordinator);
        ctrlc::set_handler(move || {
            if handler.request_cancel() == CancellationSignalAction::ForceExit {
                eprintln!("task_state_requires_recovery_or_status");
                std::process::exit(130);
            }
        })?;
        Ok(coordinator)
    }

    fn new(canceller: Option<Arc<dyn CliTaskCanceller>>) -> Self {
        Self {
            canceller: Mutex::new(canceller),
            signal_count: AtomicUsize::new(0),
            active_task_id: Mutex::new(None),
            cancelled_task_id: Mutex::new(None),
        }
    }

    pub(crate) fn bind(&self, handle: LifecycleTaskCancellationHandle) {
        self.bind_canceller(Arc::new(handle));
    }

    fn bind_canceller(&self, handle: Arc<dyn CliTaskCanceller>) {
        if let Ok(mut canceller) = self.canceller.lock() {
            *canceller = Some(handle);
        }
        if self.signal_count.load(Ordering::SeqCst) > 0 {
            self.try_cancel_registered_task();
        }
    }

    pub(crate) fn observing<'a, O: TaskProgressObserver + ?Sized>(
        self: &Arc<Self>,
        observer: &'a O,
    ) -> CancellationRegisteringObserver<'a, O> {
        CancellationRegisteringObserver {
            coordinator: Arc::clone(self),
            observer,
        }
    }

    pub(crate) fn cancelled_event(&self) -> Option<TaskProgressEvent> {
        self.cancelled_task_id
            .lock()
            .ok()
            .and_then(|task_id| task_id.clone())
            .map(|task_id| {
                TaskProgressEvent::new(
                    task_id,
                    TaskKind::Install,
                    TaskStatus::Cancelled,
                    CANCELLED_PHASE,
                )
            })
    }

    fn request_cancel(&self) -> CancellationSignalAction {
        if self.signal_count.fetch_add(1, Ordering::SeqCst) > 0 {
            return CancellationSignalAction::ForceExit;
        }
        self.try_cancel_registered_task();
        CancellationSignalAction::WaitForTerminal
    }

    fn register_task(&self, task_id: &str) {
        if let Ok(mut active_task_id) = self.active_task_id.lock() {
            if active_task_id.is_none() {
                *active_task_id = Some(task_id.to_owned());
            }
        }
        if self.signal_count.load(Ordering::SeqCst) > 0 {
            self.try_cancel_registered_task();
        }
    }

    fn try_cancel_registered_task(&self) {
        if self
            .cancelled_task_id
            .lock()
            .is_ok_and(|task_id| task_id.is_some())
        {
            return;
        }
        let task_id = self
            .active_task_id
            .lock()
            .ok()
            .and_then(|task_id| task_id.clone());
        let Some(task_id) = task_id else {
            return;
        };
        let canceller = self
            .canceller
            .lock()
            .ok()
            .and_then(|canceller| canceller.clone());
        if canceller.is_some_and(|canceller| canceller.cancel_task(&task_id)) {
            if let Ok(mut cancelled_task_id) = self.cancelled_task_id.lock() {
                *cancelled_task_id = Some(task_id);
            }
        }
    }
}

pub(crate) struct CancellationRegisteringObserver<'a, O: TaskProgressObserver + ?Sized> {
    coordinator: Arc<CliCancellationCoordinator>,
    observer: &'a O,
}

impl<O: TaskProgressObserver + ?Sized> TaskProgressObserver
    for CancellationRegisteringObserver<'_, O>
{
    type Error = O::Error;

    fn observe(&self, event: &TaskProgressEvent) -> Result<(), Self::Error> {
        self.coordinator.register_task(&event.task_id);
        self.observer.observe(event)
    }
}

pub(crate) struct NoopCliTaskProgressObserver;

impl TaskProgressObserver for NoopCliTaskProgressObserver {
    type Error = Infallible;

    fn observe(&self, _event: &TaskProgressEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingCanceller {
        task_ids: Mutex<Vec<String>>,
    }

    #[derive(Default)]
    struct RecordingObserver {
        events: Mutex<Vec<TaskProgressEvent>>,
    }

    impl TaskProgressObserver for RecordingObserver {
        type Error = Infallible;

        fn observe(&self, event: &TaskProgressEvent) -> Result<(), Self::Error> {
            self.events
                .lock()
                .expect("recording observer")
                .push(event.clone());
            Ok(())
        }
    }

    impl CliTaskCanceller for RecordingCanceller {
        fn cancel_task(&self, task_id: &str) -> bool {
            self.task_ids
                .lock()
                .expect("recording canceller")
                .push(task_id.to_owned());
            true
        }
    }

    #[test]
    fn first_signal_cancels_registered_task_and_builds_one_terminal_event() {
        let canceller = Arc::new(RecordingCanceller::default());
        let coordinator = CliCancellationCoordinator::new(Some(canceller.clone()));
        coordinator.register_task("install-123");

        assert_eq!(
            coordinator.request_cancel(),
            CancellationSignalAction::WaitForTerminal
        );
        assert_eq!(
            canceller.task_ids.lock().expect("recorded ids").as_slice(),
            ["install-123"]
        );
        let event = coordinator.cancelled_event().expect("cancelled event");
        assert_eq!(event.task_id, "install-123");
        assert_eq!(event.kind, TaskKind::Install);
        assert_eq!(event.status, TaskStatus::Cancelled);
        assert_eq!(event.phase, CANCELLED_PHASE);
    }

    #[test]
    fn signal_before_task_registration_cancels_when_queued_event_arrives() {
        let canceller = Arc::new(RecordingCanceller::default());
        let coordinator = CliCancellationCoordinator::new(Some(canceller.clone()));

        assert_eq!(
            coordinator.request_cancel(),
            CancellationSignalAction::WaitForTerminal
        );
        coordinator.register_task("install-late");

        assert_eq!(
            canceller.task_ids.lock().expect("recorded ids").as_slice(),
            ["install-late"]
        );
        assert_eq!(
            coordinator
                .cancelled_event()
                .expect("cancelled event")
                .task_id,
            "install-late"
        );
    }

    #[test]
    fn signal_before_runtime_binding_is_latched_until_task_and_handle_exist() {
        let canceller = Arc::new(RecordingCanceller::default());
        let coordinator = CliCancellationCoordinator::new(None);

        assert_eq!(
            coordinator.request_cancel(),
            CancellationSignalAction::WaitForTerminal
        );
        coordinator.register_task("install-late");
        assert!(coordinator.cancelled_event().is_none());
        coordinator.bind_canceller(canceller.clone());

        assert_eq!(
            canceller.task_ids.lock().expect("recorded ids").as_slice(),
            ["install-late"]
        );
        assert_eq!(
            coordinator
                .cancelled_event()
                .expect("cancelled event")
                .task_id,
            "install-late"
        );
    }

    #[test]
    fn second_signal_requests_force_exit_without_duplicate_cancel() {
        let canceller = Arc::new(RecordingCanceller::default());
        let coordinator = CliCancellationCoordinator::new(Some(canceller.clone()));
        coordinator.register_task("install-123");

        assert_eq!(
            coordinator.request_cancel(),
            CancellationSignalAction::WaitForTerminal
        );
        assert_eq!(
            coordinator.request_cancel(),
            CancellationSignalAction::ForceExit
        );
        assert_eq!(
            canceller.task_ids.lock().expect("recorded ids").as_slice(),
            ["install-123"]
        );
    }

    #[test]
    fn cancellation_observer_forwards_queued_and_one_cancelled_terminal() {
        let canceller = Arc::new(RecordingCanceller::default());
        let coordinator = Arc::new(CliCancellationCoordinator::new(Some(canceller)));
        let observer = RecordingObserver::default();
        let observer = coordinator.observing(&observer);
        observer
            .observe(&TaskProgressEvent::new(
                "install-123",
                TaskKind::Install,
                TaskStatus::Queued,
                "install.reinstall.queued",
            ))
            .expect("observe queued");
        assert_eq!(
            coordinator.request_cancel(),
            CancellationSignalAction::WaitForTerminal
        );
        observer
            .observe(
                &coordinator
                    .cancelled_event()
                    .expect("confirmed cancelled event"),
            )
            .expect("observe cancelled");

        let events = observer.observer.events.lock().expect("recorded events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].phase, "install.reinstall.queued");
        assert_eq!(events[1].phase, CANCELLED_PHASE);
        assert_eq!(events[1].status, TaskStatus::Cancelled);
    }
}
