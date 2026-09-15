use super::*;
use std::{fs, path::Path, sync::Mutex};

#[derive(Default)]
struct RecordingOpener(Mutex<Vec<PathBuf>>);
impl SystemDirectoryOpener for RecordingOpener {
    fn open_directory(&self, path: &Path) -> Result<()> {
        self.0.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

#[test]
fn opens_existing_package_below_custom_storage_without_creating_files() {
    let fixture = tempfile::tempdir().unwrap();
    let recorder = Arc::new(RecordingOpener::default());
    let opener = SandboxModDirectoryOpener::new(fixture.path().to_path_buf(), recorder.clone());
    let package = opener
        .locator
        .sandbox_root_for_package("revision-b")
        .unwrap();
    fs::create_dir_all(&package).unwrap();
    fs::write(package.join("sentinel"), b"original").unwrap();
    opener.open_package_directory("revision-b").unwrap();
    assert_eq!(*recorder.0.lock().unwrap(), vec![package.clone()]);
    assert_eq!(fs::read(package.join("sentinel")).unwrap(), b"original");
    assert_eq!(fs::read_dir(package).unwrap().count(), 1);
}

#[test]
fn missing_files_and_traversal_never_reach_the_system_opener() {
    let fixture = tempfile::tempdir().unwrap();
    let recorder = Arc::new(RecordingOpener::default());
    let opener = SandboxModDirectoryOpener::new(fixture.path().to_path_buf(), recorder.clone());
    assert!(opener.open_package_directory("missing").is_err());
    assert_eq!(fs::read_dir(fixture.path()).unwrap().count(), 0);
    let package = opener.locator.sandbox_root_for_package("file").unwrap();
    fs::create_dir_all(package.parent().unwrap()).unwrap();
    fs::write(&package, b"not a folder").unwrap();
    for id in ["file", "missing", "..", "../escape", "C:\\escape", "a/b"] {
        assert!(opener.open_package_directory(id).is_err(), "{id}");
    }
    assert!(recorder.0.lock().unwrap().is_empty());
    assert_eq!(fs::read(package).unwrap(), b"not a folder");
}

#[cfg(any(windows, unix))]
#[test]
fn package_and_parent_links_cannot_redirect_the_shortcut() {
    let fixture = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let recorder = Arc::new(RecordingOpener::default());
    let opener = SandboxModDirectoryOpener::new(fixture.path().to_path_buf(), recorder.clone());
    let package = opener.locator.sandbox_root_for_package("linked").unwrap();
    fs::create_dir_all(package.parent().unwrap()).unwrap();
    link_directory(outside.path(), package.as_path());
    assert!(opener.open_package_directory("linked").is_err());
    #[cfg(windows)]
    fs::remove_dir(&package).unwrap();
    #[cfg(unix)]
    fs::remove_file(&package).unwrap();
    fs::remove_dir(package.parent().unwrap()).unwrap();
    link_directory(outside.path(), package.parent().unwrap());
    fs::create_dir(outside.path().join("linked")).unwrap();
    assert!(opener.open_package_directory("linked").is_err());
    assert!(recorder.0.lock().unwrap().is_empty());
}

#[cfg(windows)]
fn link_directory(target: &Path, link: &Path) {
    // Directory junctions exercise Windows reparse-point rejection without symlink privilege.
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[cfg(unix)]
fn link_directory(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}
