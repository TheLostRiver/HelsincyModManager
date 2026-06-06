use std::path::{Path, PathBuf};

pub trait SteamRootProvider: Send + Sync {
    fn steam_roots(&self) -> Vec<PathBuf>;
}

pub struct PlatformSteamRootProvider;

impl SteamRootProvider for PlatformSteamRootProvider {
    fn steam_roots(&self) -> Vec<PathBuf> {
        platform_steam_roots()
    }
}

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
        push_unique(
            &mut roots,
            PathBuf::from(program_files_x86).join("Steam"),
        );
    }

    roots
}

#[cfg(target_os = "linux")]
fn platform_steam_roots() -> Vec<PathBuf> {
    std::env::var_os("HOME")
        .map(|home| linux_steam_roots_from_home(Path::new(&home)))
        .unwrap_or_default()
}

#[cfg(not(any(windows, target_os = "linux")))]
fn platform_steam_roots() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(windows)]
fn push_unique(roots: &mut Vec<PathBuf>, root: PathBuf) {
    if !roots.iter().any(|existing| existing == &root) {
        roots.push(root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_root_builds_linux_candidate_roots_from_home() {
        let roots = linux_steam_roots_from_home(std::path::Path::new("/home/deck"));

        assert_eq!(roots[0], std::path::PathBuf::from("/home/deck/.steam/steam"));
        assert!(roots.iter().any(|root| {
            root.ends_with(".var/app/com.valvesoftware.Steam/.local/share/Steam")
        }));
    }
}
