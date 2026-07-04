use crate::steam_discovery::SteamRootProvider;
use anyhow::{bail, Result};
use hmm_core::SaveDirectoryCandidateConfidence;
use hmm_ports::{ScannedSaveDirectoryCandidate, SteamUserdataScanRequest, SteamUserdataScanner};
use std::collections::BTreeSet;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

pub struct SteamUserdataSaveDirectoryScanner {
    root_provider: Arc<dyn SteamRootProvider>,
}

impl SteamUserdataSaveDirectoryScanner {
    pub fn new(root_provider: Arc<dyn SteamRootProvider>) -> Self {
        Self { root_provider }
    }

    fn candidate_from_directory(
        &self,
        request: &SteamUserdataScanRequest,
        account_id_32: u32,
        directory: PathBuf,
    ) -> Result<ScannedSaveDirectoryCandidate> {
        let metadata = fs::metadata(&directory)?;
        if !metadata.is_dir() {
            bail!("save directory candidate is not a directory");
        }

        let known_files = request
            .known_save_file_names
            .iter()
            .filter(|file_name| directory.join(file_name.as_str()).is_file())
            .cloned()
            .collect::<Vec<_>>();
        let confidence = if known_files.is_empty() {
            SaveDirectoryCandidateConfidence::Medium
        } else {
            SaveDirectoryCandidateConfidence::High
        };
        let mut evidence = vec![format!(
            "Found Steam userdata save directory for {}",
            request.game_id.as_str()
        )];
        evidence.extend(
            known_files
                .iter()
                .map(|file_name| format!("Found MHW:I save file {file_name}")),
        );

        let last_modified_at = newest_modified_at(&directory, &known_files, &metadata);

        Ok(ScannedSaveDirectoryCandidate {
            candidate_id: candidate_id_for(account_id_32, &directory),
            account_id_32,
            directory,
            confidence,
            last_modified_at,
            evidence,
            account_label: masked_account_label(account_id_32),
            path_label: request.path_label.clone(),
        })
    }

    fn steam_roots(&self, game_root_hint: Option<&Path>) -> Vec<PathBuf> {
        let mut seen = BTreeSet::new();
        let mut roots = Vec::new();

        if let Some(root) = game_root_hint.and_then(derive_steam_root_from_game_root) {
            push_unique_root(&mut roots, &mut seen, root);
        }

        for root in self.root_provider.steam_roots() {
            push_unique_root(&mut roots, &mut seen, root);
        }

        roots
    }
}

impl SteamUserdataScanner for SteamUserdataSaveDirectoryScanner {
    fn scan_save_directories(
        &self,
        request: &SteamUserdataScanRequest,
    ) -> Result<Vec<ScannedSaveDirectoryCandidate>> {
        let mut candidates = Vec::new();
        let mut seen_directories = BTreeSet::new();

        for steam_root in self.steam_roots(request.game_root_hint.as_deref()) {
            let userdata = steam_root.join("userdata");
            let Ok(accounts) = fs::read_dir(&userdata) else {
                continue;
            };

            for account in accounts {
                let account = account?;
                let account_name = account.file_name().to_string_lossy().to_string();
                let Ok(account_id_32) = account_name.parse::<u32>() else {
                    continue;
                };
                let directory = account.path().join(&request.remote_relative_path);
                if !directory.is_dir() {
                    continue;
                }
                let directory_key = normalize_path_key(&directory);
                if !seen_directories.insert(directory_key) {
                    continue;
                }
                candidates.push(self.candidate_from_directory(
                    request,
                    account_id_32,
                    directory,
                )?);
            }
        }

        candidates.sort_by(|left, right| {
            right
                .last_modified_at
                .cmp(&left.last_modified_at)
                .then_with(|| left.account_label.cmp(&right.account_label))
        });

        Ok(candidates)
    }

    fn validate_save_directory(
        &self,
        request: &SteamUserdataScanRequest,
        directory: &Path,
    ) -> Result<ScannedSaveDirectoryCandidate> {
        let directory_key = normalize_path_key(directory);

        for steam_root in self.steam_roots(request.game_root_hint.as_deref()) {
            let userdata = steam_root.join("userdata");
            let Ok(accounts) = fs::read_dir(&userdata) else {
                continue;
            };

            for account in accounts {
                let account = account?;
                let account_name = account.file_name().to_string_lossy().to_string();
                let Ok(account_id_32) = account_name.parse::<u32>() else {
                    continue;
                };
                let candidate_directory = account.path().join(&request.remote_relative_path);
                if normalize_path_key(&candidate_directory) == directory_key {
                    return self.candidate_from_directory(
                        request,
                        account_id_32,
                        candidate_directory,
                    );
                }
            }
        }

        bail!("save directory candidate is outside known Steam userdata roots");
    }
}

fn newest_modified_at(
    directory: &Path,
    known_files: &[String],
    directory_metadata: &fs::Metadata,
) -> Option<u128> {
    let mut newest = directory_metadata.modified().ok();

    for file_name in known_files {
        let modified = fs::metadata(directory.join(file_name))
            .and_then(|metadata| metadata.modified())
            .ok();
        if modified > newest {
            newest = modified;
        }
    }

    newest
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis())
}

fn masked_account_label(account_id_32: u32) -> String {
    let account = account_id_32.to_string();
    let visible = if account.len() > 4 {
        &account[account.len() - 4..]
    } else {
        account.as_str()
    };

    format!("Steam user ****{visible}")
}

fn candidate_id_for(account_id_32: u32, directory: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    account_id_32.hash(&mut hasher);
    normalize_path_key(directory).hash(&mut hasher);
    format!("steam-userdata-{:016x}", hasher.finish())
}

fn push_unique_root(roots: &mut Vec<PathBuf>, seen: &mut BTreeSet<String>, root: PathBuf) {
    if seen.insert(normalize_path_key(&root)) {
        roots.push(root);
    }
}

fn derive_steam_root_from_game_root(game_root: &Path) -> Option<PathBuf> {
    let common = game_root.parent()?;
    if !path_file_name_eq(common, "common") {
        return None;
    }
    let steamapps = common.parent()?;
    if !path_file_name_eq(steamapps, "steamapps") {
        return None;
    }

    steamapps.parent().map(Path::to_path_buf)
}

fn path_file_name_eq(path: &Path, expected: &str) -> bool {
    path.file_name()
        .map(|name| name.to_string_lossy().eq_ignore_ascii_case(expected))
        .unwrap_or(false)
}

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
    use hmm_core::GameId;

    struct FakeSteamRootProvider {
        roots: Vec<PathBuf>,
    }

    impl SteamRootProvider for FakeSteamRootProvider {
        fn steam_roots(&self) -> Vec<PathBuf> {
            self.roots.clone()
        }
    }

    #[test]
    fn scanner_finds_high_confidence_mhw_save_with_known_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let remote = temp
            .path()
            .join("userdata")
            .join("1234")
            .join("582010")
            .join("remote");
        fs::create_dir_all(&remote).expect("create remote");
        fs::write(remote.join("SAVEDATA1000"), b"save").expect("write save");

        let scanner = SteamUserdataSaveDirectoryScanner::new(Arc::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));
        let candidates = scanner
            .scan_save_directories(&mhw_request(None))
            .expect("scan");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].account_id_32, 1234);
        assert_eq!(
            candidates[0].confidence,
            SaveDirectoryCandidateConfidence::High
        );
        assert_eq!(
            candidates[0].path_label,
            "Steam/userdata/<account>/582010/remote"
        );
        assert!(candidates[0]
            .evidence
            .iter()
            .any(|item| item.contains("SAVEDATA1000")));
        assert!(!candidates[0].candidate_id.contains("1234"));
    }

    #[test]
    fn scanner_ignores_non_numeric_account_directories() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(
            temp.path()
                .join("userdata")
                .join("not-an-id")
                .join("582010")
                .join("remote"),
        )
        .expect("create remote");

        let scanner = SteamUserdataSaveDirectoryScanner::new(Arc::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));

        assert!(scanner
            .scan_save_directories(&mhw_request(None))
            .expect("scan")
            .is_empty());
    }

    #[test]
    fn scanner_uses_game_root_hint_to_derive_steam_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let game_root = temp
            .path()
            .join("steamapps")
            .join("common")
            .join("Monster Hunter World");
        fs::create_dir_all(&game_root).expect("create game root");
        let remote = temp
            .path()
            .join("userdata")
            .join("2222")
            .join("582010")
            .join("remote");
        fs::create_dir_all(&remote).expect("create remote");
        fs::write(remote.join("SAVEDATA1000"), b"save").expect("write save");

        let scanner = SteamUserdataSaveDirectoryScanner::new(Arc::new(FakeSteamRootProvider {
            roots: Vec::new(),
        }));
        let candidates = scanner
            .scan_save_directories(&mhw_request(Some(game_root)))
            .expect("scan");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].account_id_32, 2222);
    }

    #[test]
    fn scanner_validates_cached_candidate_under_known_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let remote = temp
            .path()
            .join("userdata")
            .join("3333")
            .join("582010")
            .join("remote");
        fs::create_dir_all(&remote).expect("create remote");
        fs::write(remote.join("SAVEDATA1000"), b"save").expect("write save");

        let scanner = SteamUserdataSaveDirectoryScanner::new(Arc::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));
        let candidate = scanner
            .validate_save_directory(&mhw_request(None), &remote)
            .expect("validate");

        assert_eq!(candidate.account_id_32, 3333);
        assert_eq!(candidate.confidence, SaveDirectoryCandidateConfidence::High);
    }

    #[test]
    fn scanner_rejects_validation_outside_known_roots() {
        let scanner = SteamUserdataSaveDirectoryScanner::new(Arc::new(FakeSteamRootProvider {
            roots: Vec::new(),
        }));

        assert!(scanner
            .validate_save_directory(
                &mhw_request(None),
                Path::new("Z:/userdata/3333/582010/remote")
            )
            .is_err());
    }

    fn mhw_request(game_root_hint: Option<PathBuf>) -> SteamUserdataScanRequest {
        SteamUserdataScanRequest {
            game_id: GameId::mhw(),
            game_root_hint,
            steam_app_id: 582010,
            remote_relative_path: "582010/remote".to_owned(),
            known_save_file_names: vec!["SAVEDATA1000".to_owned()],
            path_label: "Steam/userdata/<account>/582010/remote".to_owned(),
        }
    }
}
