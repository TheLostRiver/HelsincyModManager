//! "Move instead of copy" for zip imports (#275 slice 4).
//!
//! HMM never keeps the archive — an import unpacks it into the storage root — so moving means
//! unpacking and then deleting the user's source file. The policy lives here: the runner asks
//! for a fingerprint before it starts reading and hands it back once the catalog write is
//! durable; the consumer only deletes the very file that was fingerprinted, and never one that
//! lies inside a game root, the Mod storage root or app-data. External (HuntingBox) imports do
//! not go through this service at all: those archives belong to another manager.

use hmm_core::GameId;
use hmm_ports::{
    GameConfigRepository, ModImportArchiveConsumeError, ModImportArchiveConsumer,
    ModImportArchiveFingerprint,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct ModImportArchiveConsumptionService {
    consumer: Arc<dyn ModImportArchiveConsumer>,
    game_config: Arc<dyn GameConfigRepository>,
    game_ids: Vec<GameId>,
    /// Roots that are protected regardless of game configuration (app-data, storage root).
    protected_roots: Vec<PathBuf>,
}

impl ModImportArchiveConsumptionService {
    pub fn new(
        consumer: Arc<dyn ModImportArchiveConsumer>,
        game_config: Arc<dyn GameConfigRepository>,
        game_ids: Vec<GameId>,
        protected_roots: Vec<PathBuf>,
    ) -> Self {
        Self {
            consumer,
            game_config,
            game_ids,
            protected_roots,
        }
    }

    /// Taken before the archive is read so a later swap at the same path is detected.
    pub fn fingerprint(
        &self,
        archive_path: &Path,
    ) -> Result<ModImportArchiveFingerprint, ModImportArchiveConsumeError> {
        self.consumer.fingerprint(archive_path)
    }

    /// Deletes the archive once the import is durable. Game roots are read now, not at
    /// construction, so a directory configured during the session is protected too. When the
    /// configuration cannot be read the archive is kept — deleting on incomplete facts is the
    /// one thing this feature must never do.
    pub fn consume(
        &self,
        archive_path: &Path,
        fingerprint: &ModImportArchiveFingerprint,
    ) -> Result<(), ModImportArchiveConsumeError> {
        let mut roots = self.protected_roots.clone();
        for game_id in &self.game_ids {
            match self.game_config.load_game_instance(game_id) {
                Ok(Some(instance)) => roots.push(instance.root_dir),
                Ok(None) => {}
                Err(_) => return Err(ModImportArchiveConsumeError::Unavailable),
            }
        }
        self.consumer.consume(archive_path, fingerprint, &roots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameDirectoryStatus, GameInstance};
    use hmm_ports::{GameConfigRepositoryError, GameConfigRepositoryResult};
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeConsumer {
        consumed: Mutex<Vec<(PathBuf, Vec<PathBuf>)>>,
    }

    impl ModImportArchiveConsumer for FakeConsumer {
        fn fingerprint(
            &self,
            _archive_path: &Path,
        ) -> Result<ModImportArchiveFingerprint, ModImportArchiveConsumeError> {
            Ok(fingerprint())
        }

        fn consume(
            &self,
            archive_path: &Path,
            expected: &ModImportArchiveFingerprint,
            protected_roots: &[PathBuf],
        ) -> Result<(), ModImportArchiveConsumeError> {
            assert_eq!(*expected, fingerprint());
            self.consumed
                .lock()
                .expect("consumed lock")
                .push((archive_path.to_path_buf(), protected_roots.to_vec()));
            Ok(())
        }
    }

    struct FakeGameConfig {
        instance: Option<GameInstance>,
        fail: bool,
    }

    impl GameConfigRepository for FakeGameConfig {
        fn load_game_instance(
            &self,
            _game_id: &GameId,
        ) -> GameConfigRepositoryResult<Option<GameInstance>> {
            if self.fail {
                return Err(GameConfigRepositoryError::StorageFailed(
                    "fixture".to_owned(),
                ));
            }
            Ok(self.instance.clone())
        }

        fn save_game_instance(&self, _instance: &GameInstance) -> GameConfigRepositoryResult<()> {
            Ok(())
        }
    }

    fn fingerprint() -> ModImportArchiveFingerprint {
        ModImportArchiveFingerprint {
            len: 42,
            modified_unix_millis: Some(1),
            identity: None,
        }
    }

    fn path(windows: &str, unix: &str) -> PathBuf {
        PathBuf::from(if cfg!(windows) { windows } else { unix })
    }

    fn game_instance() -> GameInstance {
        GameInstance {
            id: "mhw-default".to_owned(),
            game_id: GameId::mhw(),
            display_name: "MHW".to_owned(),
            root_dir: path("D:\\Games\\MHW", "/games/mhw"),
            status: GameDirectoryStatus::Configured,
            configured_at_unix_millis: 1,
        }
    }

    #[test]
    fn consume_protects_fixed_roots_and_the_currently_configured_game_root() {
        let consumer = Arc::new(FakeConsumer::default());
        let service = ModImportArchiveConsumptionService::new(
            consumer.clone(),
            Arc::new(FakeGameConfig {
                instance: Some(game_instance()),
                fail: false,
            }),
            vec![GameId::mhw()],
            vec![
                path("C:\\app-data", "/app-data"),
                path("E:\\HMMMods", "/srv/hmm-mods"),
            ],
        );
        let archive = path("C:\\Downloads\\mod.zip", "/downloads/mod.zip");

        service
            .consume(&archive, &fingerprint())
            .expect("consume delegates");

        assert_eq!(
            consumer.consumed.lock().expect("consumed").as_slice(),
            [(
                archive,
                vec![
                    path("C:\\app-data", "/app-data"),
                    path("E:\\HMMMods", "/srv/hmm-mods"),
                    path("D:\\Games\\MHW", "/games/mhw"),
                ]
            )]
        );
    }

    #[test]
    fn consume_keeps_the_archive_when_the_game_configuration_cannot_be_read() {
        let consumer = Arc::new(FakeConsumer::default());
        let service = ModImportArchiveConsumptionService::new(
            consumer.clone(),
            Arc::new(FakeGameConfig {
                instance: None,
                fail: true,
            }),
            vec![GameId::mhw()],
            Vec::new(),
        );

        assert_eq!(
            service.consume(
                &path("C:\\Downloads\\mod.zip", "/downloads/mod.zip"),
                &fingerprint()
            ),
            Err(ModImportArchiveConsumeError::Unavailable)
        );
        assert!(consumer.consumed.lock().expect("consumed").is_empty());
    }
}
