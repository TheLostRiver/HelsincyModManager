use hmm_ports::GameRunningStatus;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_NO_MORE_FILES, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

struct ProcessSnapshot(HANDLE);

impl Drop for ProcessSnapshot {
    fn drop(&mut self) {
        // SAFETY: this guard exclusively owns a valid Tool Help snapshot handle.
        unsafe { CloseHandle(self.0) };
    }
}

/// Each call takes a fresh process snapshot. Absence is never cached across writes.
pub(super) fn query_once(image_name: &str) -> Option<GameRunningStatus> {
    if image_name.trim().is_empty() || image_name.contains(['\0', '/', '\\']) {
        return None;
    }
    // SAFETY: TH32CS_SNAPPROCESS snapshots all processes; no target process handle is used.
    let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if handle == INVALID_HANDLE_VALUE {
        return None;
    }
    let snapshot = ProcessSnapshot(handle);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: the snapshot is valid and entry has the required size and writable storage.
    let mut found = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    loop {
        if found == 0 {
            // SAFETY: capture the enumeration error before any other Windows call.
            return exhausted_status(unsafe { GetLastError() });
        }
        if is_live_instance(&entry, image_name) {
            return Some(GameRunningStatus::Running);
        }
        // SAFETY: snapshot and entry remain valid for the entire enumeration.
        found = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
}

fn exhausted_status(error: u32) -> Option<GameRunningStatus> {
    (error == ERROR_NO_MORE_FILES).then_some(GameRunningStatus::NotRunning)
}

/// A matching entry with zero threads is not a running game. A leftover seen in the
/// field carried the game's image name days after the game had exited: zero threads,
/// zero handles, parent gone. A process-snapshot VA clone (`PssCaptureSnapshot`, the
/// kind dump collectors take) reproduces exactly that, and stays listed for as long as
/// whoever took it keeps the snapshot open. It runs no code and holds no file, yet
/// counting it blocks every game write until its taker exits or the machine reboots.
/// Opening it does not tell it apart either: `OpenProcess` on the field leftover was
/// measured to fail with access denied.
///
/// Enumeration must continue past it: a relaunched game may be listed after it.
fn is_live_instance(entry: &PROCESSENTRY32W, image_name: &str) -> bool {
    entry.cntThreads > 0 && matches_image(&entry.szExeFile, image_name)
}

fn matches_image(name: &[u16], expected: &str) -> bool {
    let length = name
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(name.len());
    String::from_utf16_lossy(&name[..length]).eq_ignore_ascii_case(expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::io::AsRawHandle;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{
        ERROR_ACCESS_DENIED, ERROR_BAD_LENGTH, ERROR_INVALID_HANDLE,
    };
    use windows_sys::Win32::System::Diagnostics::ProcessSnapshotting::{
        PssCaptureSnapshot, PssFreeSnapshot, HPSS, PSS_CAPTURE_VA_CLONE,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    const PROBE_CHILD_TEST: &str =
        "game_running_detector::windows_processes::tests::game_running_probe_child";
    const PROBE_CHILD_HOLD_ENV: &str = "HMM_GAME_RUNNING_PROBE_CHILD_HOLD";

    #[test]
    #[ignore = "invoked only as a controlled child process by the snapshot tests below"]
    fn game_running_probe_child() {
        if std::env::var_os(PROBE_CHILD_HOLD_ENV).is_some() {
            // The parent kills this child; the bound only matters if it never does.
            std::thread::sleep(Duration::from_secs(60));
        }
    }

    /// This test binary under a fresh name, so snapshot matches can only come from
    /// children spawned by the test itself.
    struct UniqueImage {
        _dir: tempfile::TempDir,
        path: PathBuf,
        name: String,
    }

    fn unique_image() -> UniqueImage {
        let current = std::env::current_exe().unwrap();
        // Same volume as the test binary, so a hard link normally works.
        let dir = tempfile::tempdir_in(current.parent().unwrap()).unwrap();
        let name = format!("hmm-probe-{}.exe", uuid::Uuid::new_v4().simple());
        let path = dir.path().join(&name);
        if std::fs::hard_link(&current, &path).is_err() {
            std::fs::copy(&current, &path).expect("copy test binary under a unique name");
        }
        UniqueImage {
            _dir: dir,
            path,
            name,
        }
    }

    /// Kills and reaps the child when dropped, so no probe outlives its test.
    struct ProbeChild(Child);

    impl Drop for ProbeChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn spawn_probe_child(image: &UniqueImage) -> ProbeChild {
        let child = Command::new(&image.path)
            .args(["--exact", PROBE_CHILD_TEST, "--ignored"])
            .env(PROBE_CHILD_HOLD_ENV, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn probe child");
        ProbeChild(child)
    }

    /// A process-snapshot VA clone of a probe child, freed when dropped.
    struct VaClone(HPSS);

    impl VaClone {
        fn capture(source: &ProbeChild) -> Self {
            let mut snapshot: HPSS = std::ptr::null_mut();
            // SAFETY: the child handle is open with full access and snapshot is writable.
            let error = unsafe {
                PssCaptureSnapshot(
                    source.0.as_raw_handle(),
                    PSS_CAPTURE_VA_CLONE,
                    0,
                    &mut snapshot,
                )
            };
            assert_eq!(error, 0, "PssCaptureSnapshot(PSS_CAPTURE_VA_CLONE)");
            Self(snapshot)
        }
    }

    impl Drop for VaClone {
        fn drop(&mut self) {
            // SAFETY: this guard exclusively owns a snapshot captured by this process.
            unsafe { PssFreeSnapshot(GetCurrentProcess(), self.0) };
        }
    }

    /// Thread counts of every snapshot entry listed under `image_name`.
    fn listed_thread_counts(image_name: &str) -> Vec<u32> {
        // SAFETY: TH32CS_SNAPPROCESS snapshots all processes; no target process handle is used.
        let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        assert_ne!(handle, INVALID_HANDLE_VALUE, "process snapshot");
        let snapshot = ProcessSnapshot(handle);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut threads = Vec::new();
        // SAFETY: the snapshot is valid and entry has the required size and writable storage.
        let mut found = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        while found != 0 {
            if matches_image(&entry.szExeFile, image_name) {
                threads.push(entry.cntThreads);
            }
            // SAFETY: snapshot and entry remain valid for the entire enumeration.
            found = unsafe { Process32NextW(snapshot.0, &mut entry) };
        }
        threads
    }

    /// Reproduces the field leftover: clones a probe child, kills the source, and waits
    /// until the clone is the only entry left under the name. Asserting that state is
    /// what makes a later `NotRunning` mean "leftover skipped" rather than "nothing listed".
    fn leftover_clone(image: &UniqueImage) -> VaClone {
        let source = spawn_probe_child(image);
        let clone = VaClone::capture(&source);
        drop(source);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let listed = listed_thread_counts(&image.name);
            if listed == [0] {
                return clone;
            }
            assert!(
                Instant::now() < deadline,
                "precondition: only the zero-thread clone may remain listed, got {listed:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn entry_named(name: &str, threads: u32) -> PROCESSENTRY32W {
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            cntThreads: threads,
            ..Default::default()
        };
        for (slot, unit) in entry.szExeFile.iter_mut().zip(name.encode_utf16()) {
            *slot = unit;
        }
        entry
    }

    #[test]
    fn live_instance_requires_a_matching_name_and_at_least_one_thread() {
        assert!(is_live_instance(
            &entry_named("MonsterHunterWorld.exe", 1),
            "MonsterHunterWorld.exe"
        ));
        assert!(!is_live_instance(
            &entry_named("MonsterHunterWorld.exe", 0),
            "MonsterHunterWorld.exe"
        ));
        assert!(!is_live_instance(
            &entry_named("Other.exe", 1),
            "MonsterHunterWorld.exe"
        ));
    }

    #[test]
    fn leftover_snapshot_clone_of_an_exited_game_is_not_running() {
        let image = unique_image();
        let clone = leftover_clone(&image);

        assert_eq!(query_once(&image.name), Some(GameRunningStatus::NotRunning));
        drop(clone);
    }

    #[test]
    fn leftover_snapshot_clone_does_not_mask_a_relaunched_game() {
        let image = unique_image();
        let clone = leftover_clone(&image);
        // Started after the clone, so a creation-ordered snapshot lists the clone first.
        let relaunched = spawn_probe_child(&image);
        assert!(
            listed_thread_counts(&image.name)
                .iter()
                .any(|threads| *threads > 0),
            "precondition: the relaunched child must be listed as live"
        );

        assert_eq!(query_once(&image.name), Some(GameRunningStatus::Running));
        drop(relaunched);
        drop(clone);
    }

    #[test]
    fn detects_current_process_and_case_variant() {
        let executable = std::env::current_exe().unwrap();
        let name = executable.file_name().unwrap().to_str().unwrap();
        assert_eq!(query_once(name), Some(GameRunningStatus::Running));
        assert_eq!(
            query_once(&name.to_ascii_uppercase()),
            Some(GameRunningStatus::Running)
        );
    }

    #[test]
    fn absence_requires_a_valid_image_name() {
        let name = format!("hmm-missing-{}.exe", uuid::Uuid::new_v4());
        assert_eq!(query_once(&name), Some(GameRunningStatus::NotRunning));
        for invalid in ["", "  ", "game\0.exe", "dir/game.exe", "dir\\game.exe"] {
            assert_eq!(query_once(invalid), None);
        }
    }

    #[test]
    fn enumeration_errors_do_not_prove_absence() {
        assert_eq!(
            exhausted_status(ERROR_NO_MORE_FILES),
            Some(GameRunningStatus::NotRunning)
        );
        for error in [
            0,
            ERROR_ACCESS_DENIED,
            ERROR_BAD_LENGTH,
            ERROR_INVALID_HANDLE,
        ] {
            assert_eq!(exhausted_status(error), None);
        }
    }

    #[test]
    fn image_match_is_exact_and_stops_at_the_utf16_terminator() {
        let name: Vec<u16> = "MonsterHunterWorld.exe\0ignored".encode_utf16().collect();
        assert!(matches_image(&name, "monsterhunterworld.EXE"));
        assert!(!matches_image(&name, "HunterWorld.exe"));
        assert!(!matches_image(&name, "MonsterHunterWorld.exe.bak"));
    }
}
