#![cfg(target_os = "windows")]

fn assert_gui_subsystem(binary: &str) {
    // Inspect Cargo's real binary without executing a production backup or cleanup.
    let bytes = std::fs::read(binary).expect("read compiled sidecar");
    assert_eq!(&bytes[..2], b"MZ", "DOS signature");
    let pe_offset = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
    assert_eq!(&bytes[pe_offset..pe_offset + 4], b"PE\0\0", "PE signature");
    let optional_offset = pe_offset + 24;
    let magic = u16::from_le_bytes(
        bytes[optional_offset..optional_offset + 2]
            .try_into()
            .unwrap(),
    );
    assert!(matches!(magic, 0x10b | 0x20b), "PE32 or PE32+ header");
    let subsystem = u16::from_le_bytes(
        bytes[optional_offset + 68..optional_offset + 70]
            .try_into()
            .unwrap(),
    );
    assert_eq!(subsystem, 2, "sidecar must not allocate a Windows console");
}

#[test]
fn worker_uses_windows_gui_subsystem() {
    assert_gui_subsystem(env!("CARGO_BIN_EXE_hmm-save-backup-worker"));
}

#[test]
fn installer_cleanup_uses_windows_gui_subsystem() {
    assert_gui_subsystem(env!("CARGO_BIN_EXE_hmm-save-backup-installer-cleanup"));
}
