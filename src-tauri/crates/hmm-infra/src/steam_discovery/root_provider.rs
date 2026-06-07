#[cfg(any(target_os = "linux", test))]
use std::collections::BTreeSet;
#[cfg(any(windows, target_os = "linux", test))]
use std::path::Path;
use std::path::PathBuf;

pub trait SteamRootProvider: Send + Sync {
    fn steam_roots(&self) -> Vec<PathBuf>;
}

pub struct PlatformSteamRootProvider;

impl SteamRootProvider for PlatformSteamRootProvider {
    fn steam_roots(&self) -> Vec<PathBuf> {
        platform_steam_roots()
    }
}

#[cfg(any(target_os = "linux", test))]
pub fn linux_steam_roots_from_home(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".steam").join("steam"),
        home.join(".local").join("share").join("Steam"),
        home.join(".var")
            .join("app")
            .join("com.valvesoftware.Steam")
            .join(".local")
            .join("share")
            .join("Steam"),
    ]
}

#[cfg(windows)]
fn platform_steam_roots() -> Vec<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let mut roots = Vec::new();

    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let key = RegKey::predef(hive);
        if let Ok(steam) = key.open_subkey("Software\\Valve\\Steam") {
            if let Ok(path) = steam.get_value::<String, _>("SteamPath") {
                push_unique(&mut roots, PathBuf::from(path));
            }
            if let Ok(path) = steam.get_value::<String, _>("InstallPath") {
                push_unique(&mut roots, PathBuf::from(path));
            }
        }
    }

    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        push_unique(&mut roots, PathBuf::from(program_files_x86).join("Steam"));
    }

    roots
}

#[cfg(target_os = "linux")]
fn platform_steam_roots() -> Vec<PathBuf> {
    let roots = std::env::var_os("HOME")
        .map(|home| linux_steam_roots_from_home(Path::new(&home)))
        .unwrap_or_default();

    dedupe_paths(roots)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn platform_steam_roots() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(windows)]
fn push_unique(roots: &mut Vec<PathBuf>, root: PathBuf) {
    let root_key = normalize_windows_path_key(&root);
    if !roots
        .iter()
        .any(|existing| normalize_windows_path_key(existing) == root_key)
    {
        roots.push(root);
    }
}

#[cfg(windows)]
fn normalize_windows_path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

#[cfg(any(target_os = "linux", test))]
fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();

    for path in paths {
        let key_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        let key = normalize_path_key(&key_path);
        if seen.insert(key) {
            unique.push(path);
        }
    }

    unique
}

#[cfg(any(target_os = "linux", test))]
fn normalize_path_key(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_root_builds_linux_candidate_roots_from_home() {
        let roots = linux_steam_roots_from_home(std::path::Path::new("/home/deck"));

        assert_eq!(
            roots[0],
            std::path::PathBuf::from("/home/deck/.steam/steam")
        );
        assert!(roots
            .iter()
            .any(|root| { root.ends_with(".var/app/com.valvesoftware.Steam/.local/share/Steam") }));
    }

    #[test]
    fn steam_root_deduplicates_equivalent_paths() {
        let root = std::env::temp_dir().join(format!(
            "hmm-steam-root-dedupe-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create root");

        let paths = dedupe_paths(vec![root.clone(), root.clone()]);

        assert_eq!(paths, vec![root.clone()]);

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn steam_root_push_unique_deduplicates_windows_paths_case_insensitively() {
        let mut roots = vec![PathBuf::from("C:\\Program Files (x86)\\Steam")];

        push_unique(&mut roots, PathBuf::from("c:\\program files (x86)\\steam"));

        assert_eq!(roots, vec![PathBuf::from("C:\\Program Files (x86)\\Steam")]);
    }
}
