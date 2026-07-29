use crate::steam_discovery::{parse_app_manifest, parse_library_folders, SteamRootProvider};
use hmm_ports::{
    GameCandidate, GameCandidateSource, GameDiscoveryError, GameDiscoveryRequest,
    GameDiscoveryService,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};
use std::sync::Arc;

pub struct NoopGameDiscoveryService;

impl GameDiscoveryService for NoopGameDiscoveryService {
    fn scan_candidates(
        &self,
        _request: &GameDiscoveryRequest,
    ) -> Result<Vec<GameCandidate>, GameDiscoveryError> {
        Err(GameDiscoveryError::ScanNotImplemented)
    }
}

pub struct SteamGameDiscoveryService {
    root_provider: Arc<dyn SteamRootProvider>,
    allowed_root: Option<std::path::PathBuf>,
}

impl SteamGameDiscoveryService {
    pub fn new(root_provider: Arc<dyn SteamRootProvider>) -> Self {
        Self {
            root_provider,
            allowed_root: None,
        }
    }

    pub fn new_contained(
        root_provider: Arc<dyn SteamRootProvider>,
        allowed_root: std::path::PathBuf,
    ) -> Self {
        Self {
            root_provider,
            allowed_root: Some(allowed_root),
        }
    }

    fn scan_steam_root(
        &self,
        steam_root: &Path,
        request: &GameDiscoveryRequest,
        app_id: u32,
        seen_roots: &mut BTreeSet<String>,
        candidates: &mut Vec<GameCandidate>,
    ) -> Result<(), GameDiscoveryError> {
        let libraryfolders_path = steam_root.join("steamapps").join("libraryfolders.vdf");
        if !libraryfolders_path.exists() {
            return Ok(());
        }

        let libraryfolders = fs::read_to_string(&libraryfolders_path).map_err(|error| {
            GameDiscoveryError::ScanFailed(format!("failed to read libraryfolders.vdf: {error}"))
        })?;
        let libraries = parse_library_folders(&libraryfolders)
            .map_err(|error| GameDiscoveryError::ScanFailed(error.to_string()))?;

        for library in libraries {
            if !library.app_ids.contains(&app_id) {
                continue;
            }
            let Some(library_path) = self.admit_path(&library.path) else {
                continue;
            };

            self.scan_library(&library_path, request, app_id, seen_roots, candidates)?;
        }

        Ok(())
    }

    fn scan_library(
        &self,
        library_path: &Path,
        request: &GameDiscoveryRequest,
        app_id: u32,
        seen_roots: &mut BTreeSet<String>,
        candidates: &mut Vec<GameCandidate>,
    ) -> Result<(), GameDiscoveryError> {
        let manifest_path = library_path
            .join("steamapps")
            .join(format!("appmanifest_{app_id}.acf"));
        if !manifest_path.exists() {
            return Ok(());
        }

        let manifest_content = fs::read_to_string(&manifest_path).map_err(|error| {
            GameDiscoveryError::ScanFailed(format!("failed to read app manifest: {error}"))
        })?;
        let manifest = parse_app_manifest(&manifest_content)
            .map_err(|error| GameDiscoveryError::ScanFailed(error.to_string()))?;
        if manifest.app_id != app_id {
            return Ok(());
        }

        let install_dir = Path::new(&manifest.install_dir);
        if !is_safe_install_dir(install_dir) {
            return Ok(());
        }

        let common_dir = library_path.join("steamapps").join("common");
        let root_dir = common_dir.join(install_dir);
        if !is_path_within(&root_dir, &common_dir) {
            return Ok(());
        }
        let Some(root_dir) = self.admit_path(&root_dir) else {
            return Ok(());
        };

        let normalized = normalize_path_key(&root_dir);
        if !seen_roots.insert(normalized) {
            return Ok(());
        }

        candidates.push(GameCandidate {
            game_id: request.game_id.clone(),
            display_name: request.display_name.clone(),
            root_dir,
            source: GameCandidateSource::Steam,
            source_label: "Steam".to_owned(),
        });

        Ok(())
    }

    fn admit_path(&self, path: &Path) -> Option<std::path::PathBuf> {
        let Some(allowed_root) = self.allowed_root.as_deref() else {
            return Some(path.to_path_buf());
        };

        canonical_path_within(path, allowed_root)
    }
}

impl GameDiscoveryService for SteamGameDiscoveryService {
    fn scan_candidates(
        &self,
        request: &GameDiscoveryRequest,
    ) -> Result<Vec<GameCandidate>, GameDiscoveryError> {
        let Some(app_id) = request.steam_app_id else {
            return Ok(Vec::new());
        };

        let mut candidates = Vec::new();
        let mut seen_roots = BTreeSet::new();

        for steam_root in self.root_provider.steam_roots() {
            let Some(steam_root) = self.admit_path(&steam_root) else {
                continue;
            };
            self.scan_steam_root(
                &steam_root,
                request,
                app_id,
                &mut seen_roots,
                &mut candidates,
            )?;
        }

        Ok(candidates)
    }
}

fn normalize_path_key(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

fn is_safe_install_dir(path: &Path) -> bool {
    let mut has_component = false;

    !path.is_absolute()
        && path.components().all(|component| {
            if matches!(component, Component::Normal(_)) {
                has_component = true;
                true
            } else {
                false
            }
        })
        && has_component
}

fn is_path_within(path: &Path, parent: &Path) -> bool {
    let normalized_path = normalize_path_key(path);
    let mut normalized_parent = normalize_path_key(parent);
    if !normalized_parent.ends_with('/') {
        normalized_parent.push('/');
    }

    normalized_path.starts_with(&normalized_parent)
}

fn canonical_path_within(path: &Path, parent: &Path) -> Option<std::path::PathBuf> {
    let canonical_parent = parent.canonicalize().ok()?;
    let canonical_path = path.canonicalize().ok()?;

    is_path_within_or_equal(&canonical_path, &canonical_parent).then_some(canonical_path)
}

fn is_path_within_or_equal(path: &Path, parent: &Path) -> bool {
    normalize_path_key(path) == normalize_path_key(parent) || is_path_within(path, parent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::GameId;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct FakeSteamRootProvider {
        roots: Vec<PathBuf>,
    }

    impl SteamRootProvider for FakeSteamRootProvider {
        fn steam_roots(&self) -> Vec<PathBuf> {
            self.roots.clone()
        }
    }

    struct TestSteamRoot {
        root: PathBuf,
    }

    impl TestSteamRoot {
        fn new() -> Self {
            let millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_millis();
            let root = std::env::temp_dir().join(format!(
                "hmm-steam-discovery-{}-{millis}-{}",
                std::process::id(),
                TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("steamapps")).expect("create steamapps");
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for TestSteamRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn scan_returns_explicit_not_implemented() {
        let service = NoopGameDiscoveryService;
        let request = GameDiscoveryRequest {
            game_id: GameId::mhw(),
            display_name: "Monster Hunter: World - Iceborne".to_owned(),
            steam_app_id: Some(582010),
        };

        let error = service
            .scan_candidates(&request)
            .expect_err("scan is disabled");

        assert_eq!(error, GameDiscoveryError::ScanNotImplemented);
    }

    #[test]
    fn steam_discovery_returns_candidate_from_app_manifest() {
        let temp = create_temp_steam_root();
        write_libraryfolders_with_mhw(&temp);
        write_mhw_manifest(&temp, "Monster Hunter World");

        let service = SteamGameDiscoveryService::new(Arc::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));

        let candidates = service
            .scan_candidates(&mhw_request(Some(582010)))
            .expect("scan");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].game_id, GameId::mhw());
        assert_eq!(
            candidates[0].display_name,
            "Monster Hunter: World - Iceborne"
        );
        assert!(candidates[0]
            .root_dir
            .ends_with("steamapps/common/Monster Hunter World"));
    }

    #[test]
    fn steam_discovery_returns_empty_when_steam_root_missing() {
        let service = SteamGameDiscoveryService::new(Arc::new(FakeSteamRootProvider {
            roots: vec![std::env::temp_dir().join("hmm-missing-steam-root")],
        }));

        let candidates = service
            .scan_candidates(&mhw_request(Some(582010)))
            .expect("scan");

        assert!(candidates.is_empty());
    }

    #[test]
    fn steam_discovery_returns_empty_when_app_manifest_missing() {
        let temp = create_temp_steam_root();
        write_libraryfolders_with_mhw(&temp);

        let service = SteamGameDiscoveryService::new(Arc::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));

        let candidates = service
            .scan_candidates(&mhw_request(Some(582010)))
            .expect("scan");

        assert!(candidates.is_empty());
    }

    #[test]
    fn steam_discovery_returns_empty_without_steam_app_id() {
        let temp = create_temp_steam_root();
        write_libraryfolders_with_mhw(&temp);
        write_mhw_manifest(&temp, "Monster Hunter World");

        let service = SteamGameDiscoveryService::new(Arc::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));

        let candidates = service.scan_candidates(&mhw_request(None)).expect("scan");

        assert!(candidates.is_empty());
    }

    #[test]
    fn steam_discovery_deduplicates_duplicate_libraries() {
        let temp = create_temp_steam_root();
        write_duplicate_libraryfolders_with_mhw(&temp);
        write_mhw_manifest(&temp, "Monster Hunter World");

        let service = SteamGameDiscoveryService::new(Arc::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));

        let candidates = service
            .scan_candidates(&mhw_request(Some(582010)))
            .expect("scan");

        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn steam_discovery_rejects_install_dir_with_parent_segments() {
        let temp = create_temp_steam_root();
        write_libraryfolders_with_mhw(&temp);
        write_mhw_manifest(&temp, "../Monster Hunter World");

        let service = SteamGameDiscoveryService::new(Arc::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));

        let candidates = service
            .scan_candidates(&mhw_request(Some(582010)))
            .expect("scan");

        assert!(candidates.is_empty());
    }

    #[test]
    fn steam_discovery_rejects_absolute_install_dir() {
        let temp = create_temp_steam_root();
        write_libraryfolders_with_mhw(&temp);
        let absolute_install_dir = std::env::temp_dir()
            .join("Monster Hunter World")
            .to_string_lossy()
            .replace('\\', "/");
        write_mhw_manifest(&temp, &absolute_install_dir);

        let service = SteamGameDiscoveryService::new(Arc::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));

        let candidates = service
            .scan_candidates(&mhw_request(Some(582010)))
            .expect("scan");

        assert!(candidates.is_empty());
    }

    #[test]
    fn contained_discovery_accepts_library_inside_allowed_root() {
        let temp = create_temp_steam_root();
        write_libraryfolders_with_mhw(&temp);
        write_mhw_manifest(&temp, "Monster Hunter World");
        fs::create_dir_all(
            temp.path()
                .join("steamapps")
                .join("common")
                .join("Monster Hunter World"),
        )
        .expect("create contained game root");

        let service = SteamGameDiscoveryService::new_contained(
            Arc::new(FakeSteamRootProvider {
                roots: vec![temp.path().to_path_buf()],
            }),
            temp.path().to_path_buf(),
        );

        let candidates = service
            .scan_candidates(&mhw_request(Some(582010)))
            .expect("scan");

        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].root_dir,
            temp.path()
                .join("steamapps")
                .join("common")
                .join("Monster Hunter World")
                .canonicalize()
                .expect("canonical game root")
        );
    }

    #[test]
    fn contained_discovery_does_not_read_library_outside_allowed_root() {
        let temp = create_temp_steam_root();
        let external = create_temp_steam_root();
        write_libraryfolders(
            &temp,
            format!(
                r#"
                "libraryfolders"
                {{
                    "0"
                    {{
                        "path" "{}"
                        "apps"
                        {{
                            "582010" "123456"
                        }}
                    }}
                }}
                "#,
                external.path().display()
            ),
        );
        fs::write(
            external
                .path()
                .join("steamapps")
                .join("appmanifest_582010.acf"),
            "{ invalid manifest",
        )
        .expect("write external invalid manifest");

        let service = SteamGameDiscoveryService::new_contained(
            Arc::new(FakeSteamRootProvider {
                roots: vec![temp.path().to_path_buf()],
            }),
            temp.path().to_path_buf(),
        );

        let candidates = service
            .scan_candidates(&mhw_request(Some(582010)))
            .expect("external library must be rejected before manifest read");

        assert!(candidates.is_empty());
    }

    fn create_temp_steam_root() -> TestSteamRoot {
        TestSteamRoot::new()
    }

    fn mhw_request(steam_app_id: Option<u32>) -> GameDiscoveryRequest {
        GameDiscoveryRequest {
            game_id: GameId::mhw(),
            display_name: "Monster Hunter: World - Iceborne".to_owned(),
            steam_app_id,
        }
    }

    fn write_libraryfolders_with_mhw(temp: &TestSteamRoot) {
        write_libraryfolders(
            temp,
            format!(
                r#"
                "libraryfolders"
                {{
                    "0"
                    {{
                        "path" "{}"
                        "apps"
                        {{
                            "582010" "123456"
                        }}
                    }}
                }}
                "#,
                temp.path().display()
            ),
        );
    }

    fn write_duplicate_libraryfolders_with_mhw(temp: &TestSteamRoot) {
        write_libraryfolders(
            temp,
            format!(
                r#"
                "libraryfolders"
                {{
                    "0"
                    {{
                        "path" "{}"
                        "apps"
                        {{
                            "582010" "123456"
                        }}
                    }}
                    "1"
                    {{
                        "path" "{}"
                        "apps"
                        {{
                            "582010" "123456"
                        }}
                    }}
                }}
                "#,
                temp.path().display(),
                temp.path().display()
            ),
        );
    }

    fn write_libraryfolders(temp: &TestSteamRoot, content: String) {
        fs::write(
            temp.path().join("steamapps").join("libraryfolders.vdf"),
            content,
        )
        .expect("write libraryfolders");
    }

    fn write_mhw_manifest(temp: &TestSteamRoot, install_dir: &str) {
        fs::write(
            temp.path().join("steamapps").join("appmanifest_582010.acf"),
            format!(
                r#"
                "AppState"
                {{
                    "appid" "582010"
                    "installdir" "{install_dir}"
                }}
                "#
            ),
        )
        .expect("write app manifest");
    }
}
