use hmm_core::{GameId, ProfileId};
use hmm_infra::{PlatformCrossProcessWriteAdmission, PlatformCrossProcessWriteAdmissionInitError};
use hmm_ports::{
    CancellationToken, CrossProcessWriteAdmission, CrossProcessWriteAdmissionError,
    CrossProcessWriteRecovery, CrossProcessWriteScope, NeverCancelled,
};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

const HELPER_MODE_ENV: &str = "HMM_WRITE_ADMISSION_TEST_HELPER_MODE";
const HELPER_ROOT_ENV: &str = "HMM_WRITE_ADMISSION_TEST_ROOT";
const HELPER_SCOPE_ENV: &str = "HMM_WRITE_ADMISSION_TEST_SCOPE";
const HELPER_PROFILE_ENV: &str = "HMM_WRITE_ADMISSION_TEST_PROFILE";
const HELPER_READY_ENV: &str = "HMM_WRITE_ADMISSION_TEST_READY";
const HELPER_RELEASE_ENV: &str = "HMM_WRITE_ADMISSION_TEST_RELEASE";
const CHILD_WAIT_LIMIT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

#[test]
fn invalid_namespace_fails_closed() {
    let temp = tempfile::tempdir().expect("temp root");
    let missing = temp.path().join("missing");
    let missing_error = match PlatformCrossProcessWriteAdmission::new(&missing) {
        Ok(_) => panic!("missing namespace must be rejected"),
        Err(error) => error,
    };
    assert_eq!(
        missing_error,
        PlatformCrossProcessWriteAdmissionInitError::NamespaceUnavailable
    );

    let file = temp.path().join("not-a-directory");
    std::fs::write(&file, b"fixture").expect("write namespace fixture");
    let file_error = match PlatformCrossProcessWriteAdmission::new(&file) {
        Ok(_) => panic!("non-directory namespace must be rejected"),
        Err(error) => error,
    };
    assert_eq!(
        file_error,
        PlatformCrossProcessWriteAdmissionInitError::NamespaceUnavailable
    );
}

#[test]
fn same_scope_times_out_while_different_scope_and_profile_remain_available() {
    let fixture = AdmissionFixture::new();
    let mut holder = fixture.spawn_helper("hold", "profile-a");
    wait_for_file(&holder.ready_path, &mut holder.child, CHILD_WAIT_LIMIT);

    let admission = fixture.admission();
    let held_scope = save_scope("profile-a");
    let started_at = Instant::now();
    let error = match admission.acquire(&held_scope, Duration::from_millis(175), &NeverCancelled) {
        Ok(_) => panic!("the competing process must retain the same scope"),
        Err(error) => error,
    };
    assert_eq!(error, CrossProcessWriteAdmissionError::Busy);
    assert!(started_at.elapsed() >= Duration::from_millis(100));
    assert!(started_at.elapsed() < Duration::from_secs(2));

    let other_profile = admission
        .acquire(&save_scope("profile-b"), Duration::ZERO, &NeverCancelled)
        .expect("a different profile must not be blocked");
    drop(other_profile);

    let other_scope = admission
        .acquire(&game_scope("profile-a"), Duration::ZERO, &NeverCancelled)
        .expect("a different scope must not be blocked");
    drop(other_scope);

    holder.finish();
    let released = admission
        .acquire(&held_scope, Duration::ZERO, &NeverCancelled)
        .expect("normal child release must free the scope");
    assert_eq!(released.acquisition().recovery, None);
}

#[test]
fn background_scope_times_out_while_profile_scopes_remain_available() {
    let fixture = AdmissionFixture::new();
    let mut holder = fixture.spawn_background_helper("hold");
    wait_for_file(&holder.ready_path, &mut holder.child, CHILD_WAIT_LIMIT);

    let admission = fixture.admission();
    let held_scope = CrossProcessWriteScope::background_registration();
    let error = match admission.acquire(&held_scope, Duration::from_millis(175), &NeverCancelled) {
        Ok(_) => panic!("the competing process must retain the background scope"),
        Err(error) => error,
    };
    assert_eq!(error, CrossProcessWriteAdmissionError::Busy);

    let save = admission
        .acquire(&save_scope("profile-a"), Duration::ZERO, &NeverCancelled)
        .expect("the background scope must not block save-profile writes");
    drop(save);
    let game = admission
        .acquire(&game_scope("profile-a"), Duration::ZERO, &NeverCancelled)
        .expect("the background scope must not block game-profile writes");
    drop(game);

    holder.finish();
    let released = admission
        .acquire(&held_scope, Duration::ZERO, &NeverCancelled)
        .expect("normal child release must free the background scope");
    assert_eq!(released.acquisition().recovery, None);
}

#[test]
fn cancellation_interrupts_a_cross_process_wait() {
    let fixture = AdmissionFixture::new();
    let mut holder = fixture.spawn_helper("hold", "profile-a");
    wait_for_file(&holder.ready_path, &mut holder.child, CHILD_WAIT_LIMIT);

    let admission = fixture.admission();
    let cancellation = TestCancellation::default();
    let cancellation_signal = cancellation.cancelled.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(80));
        cancellation_signal.store(true, Ordering::Release);
    });
    let started_at = Instant::now();
    let error = match admission.acquire(
        &save_scope("profile-a"),
        Duration::from_secs(5),
        &cancellation,
    ) {
        Ok(_) => panic!("cancelled waiter must not acquire the scope"),
        Err(error) => error,
    };
    canceller.join().expect("canceller thread");
    assert_eq!(error, CrossProcessWriteAdmissionError::Cancelled);
    assert!(started_at.elapsed() < Duration::from_secs(1));

    holder.finish();
}

#[test]
fn owner_exit_without_drop_releases_the_scope_and_reports_recovery() {
    let fixture = AdmissionFixture::new();
    let mut holder = fixture.spawn_helper("exit_without_drop", "profile-a");
    wait_for_file(&holder.ready_path, &mut holder.child, CHILD_WAIT_LIMIT);

    let release_path = holder.release_path.clone();
    let exit_signal = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        std::fs::write(release_path, b"exit").expect("signal child exit");
    });
    let acquired = fixture
        .admission()
        .acquire(
            &save_scope("profile-a"),
            Duration::from_secs(5),
            &NeverCancelled,
        )
        .expect("OS must release a dead owner's scope");
    exit_signal.join().expect("exit signal thread");

    #[cfg(windows)]
    assert_eq!(
        acquired.acquisition().recovery,
        Some(CrossProcessWriteRecovery::AbandonedOwner)
    );
    #[cfg(not(windows))]
    assert_eq!(
        acquired.acquisition().recovery,
        Some(CrossProcessWriteRecovery::StaleOwnerMetadata)
    );
    drop(acquired);
    holder.wait_for_exit();
}

#[test]
#[ignore = "invoked only as a controlled child process by this integration test"]
fn cross_process_write_admission_helper() {
    let Ok(mode) = std::env::var(HELPER_MODE_ENV) else {
        return;
    };
    let root = required_path_env(HELPER_ROOT_ENV);
    let scope_kind = std::env::var(HELPER_SCOPE_ENV).expect("helper scope kind");
    let profile_id = std::env::var(HELPER_PROFILE_ENV).expect("helper profile id");
    let ready_path = required_path_env(HELPER_READY_ENV);
    let release_path = required_path_env(HELPER_RELEASE_ENV);
    let admission = PlatformCrossProcessWriteAdmission::new(&root).expect("helper admission");
    let scope = match scope_kind.as_str() {
        "background" => CrossProcessWriteScope::background_registration(),
        "save" => save_scope(&profile_id),
        _ => panic!("unknown helper scope kind"),
    };
    let guard = admission
        .acquire(&scope, Duration::from_secs(5), &NeverCancelled)
        .expect("helper acquires scope");
    std::fs::write(&ready_path, b"ready").expect("write helper ready marker");
    wait_for_signal(&release_path, CHILD_WAIT_LIMIT);

    match mode.as_str() {
        "hold" => drop(guard),
        "exit_without_drop" => std::process::exit(0),
        _ => panic!("unknown helper mode"),
    }
}

fn save_scope(profile_id: &str) -> CrossProcessWriteScope {
    CrossProcessWriteScope::save_profile(&GameId::mhw(), &ProfileId::new(profile_id))
}

fn game_scope(profile_id: &str) -> CrossProcessWriteScope {
    CrossProcessWriteScope::game_profile(&GameId::mhw(), &ProfileId::new(profile_id))
}

fn required_path_env(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("missing helper path env: {name}"))
}

fn wait_for_signal(path: &Path, timeout: Duration) {
    let started_at = Instant::now();
    while !path.exists() {
        assert!(started_at.elapsed() < timeout, "helper signal timed out");
        std::thread::sleep(POLL_INTERVAL);
    }
}

fn wait_for_file(path: &Path, child: &mut Child, timeout: Duration) {
    let started_at = Instant::now();
    loop {
        if path.exists() {
            return;
        }
        if let Some(status) = child.try_wait().expect("poll helper child") {
            panic!("helper exited before ready marker: {status}");
        }
        assert!(started_at.elapsed() < timeout, "helper ready timed out");
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[derive(Default)]
struct TestCancellation {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken for TestCancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

struct AdmissionFixture {
    _temp: tempfile::TempDir,
    app_data_dir: PathBuf,
    signal_dir: PathBuf,
}

impl AdmissionFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("temp root");
        let app_data_dir = temp.path().join("app-data");
        let signal_dir = temp.path().join("signals");
        std::fs::create_dir(&app_data_dir).expect("create app-data root");
        std::fs::create_dir(&signal_dir).expect("create signal root");
        Self {
            _temp: temp,
            app_data_dir,
            signal_dir,
        }
    }

    fn admission(&self) -> PlatformCrossProcessWriteAdmission {
        PlatformCrossProcessWriteAdmission::new(&self.app_data_dir).expect("parent admission")
    }

    fn spawn_helper(&self, mode: &str, profile_id: &str) -> HelperProcess {
        self.spawn_helper_for_scope(mode, "save", profile_id)
    }

    fn spawn_background_helper(&self, mode: &str) -> HelperProcess {
        self.spawn_helper_for_scope(mode, "background", "unused")
    }

    fn spawn_helper_for_scope(
        &self,
        mode: &str,
        scope_kind: &str,
        profile_id: &str,
    ) -> HelperProcess {
        let run_id = uuid::Uuid::new_v4().to_string();
        let ready_path = self.signal_dir.join(format!("{run_id}.ready"));
        let release_path = self.signal_dir.join(format!("{run_id}.release"));
        let child = Command::new(std::env::current_exe().expect("integration test executable"))
            .args([
                "--exact",
                "cross_process_write_admission_helper",
                "--ignored",
                "--nocapture",
            ])
            .env(HELPER_MODE_ENV, mode)
            .env(HELPER_ROOT_ENV, &self.app_data_dir)
            .env(HELPER_SCOPE_ENV, scope_kind)
            .env(HELPER_PROFILE_ENV, profile_id)
            .env(HELPER_READY_ENV, &ready_path)
            .env(HELPER_RELEASE_ENV, &release_path)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("spawn write admission helper");
        HelperProcess {
            child,
            ready_path,
            release_path,
            finished: false,
        }
    }
}

struct HelperProcess {
    child: Child,
    ready_path: PathBuf,
    release_path: PathBuf,
    finished: bool,
}

impl HelperProcess {
    fn finish(mut self) {
        std::fs::write(&self.release_path, b"release").expect("signal helper release");
        self.wait_for_exit_inner();
    }

    fn wait_for_exit(&mut self) {
        self.wait_for_exit_inner();
    }

    fn wait_for_exit_inner(&mut self) {
        let started_at = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().expect("poll helper exit") {
                assert!(status.success(), "helper failed: {status}");
                self.finished = true;
                return;
            }
            assert!(
                started_at.elapsed() < CHILD_WAIT_LIMIT,
                "helper exit timed out"
            );
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

impl Drop for HelperProcess {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let _ = std::fs::write(&self.release_path, b"cleanup");
        let started_at = Instant::now();
        while started_at.elapsed() < Duration::from_secs(1) {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    self.finished = true;
                    return;
                }
                Ok(None) => std::thread::sleep(POLL_INTERVAL),
                Err(_) => break,
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.finished = true;
    }
}
