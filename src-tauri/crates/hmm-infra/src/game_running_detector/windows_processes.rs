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
        if matches_image(&entry.szExeFile, image_name) {
            return Some(GameRunningStatus::Running);
        }
        // SAFETY: snapshot and entry remain valid for the entire enumeration.
        found = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
}

fn exhausted_status(error: u32) -> Option<GameRunningStatus> {
    (error == ERROR_NO_MORE_FILES).then_some(GameRunningStatus::NotRunning)
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
    use windows_sys::Win32::Foundation::{
        ERROR_ACCESS_DENIED, ERROR_BAD_LENGTH, ERROR_INVALID_HANDLE,
    };

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
