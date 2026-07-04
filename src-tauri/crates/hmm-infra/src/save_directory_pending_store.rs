use anyhow::Result;
use hmm_ports::{
    PendingSaveDirectoryCandidate, PendingSaveDirectoryCandidateStore,
    PendingSaveDirectoryDiscovery,
};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
pub struct InMemoryPendingSaveDirectoryCandidateStore {
    discoveries: Mutex<HashMap<String, PendingSaveDirectoryDiscovery>>,
}

impl PendingSaveDirectoryCandidateStore for InMemoryPendingSaveDirectoryCandidateStore {
    fn put(&self, discovery: PendingSaveDirectoryDiscovery) -> Result<()> {
        self.discoveries
            .lock()
            .expect("pending save directory store lock")
            .insert(discovery.discovery_id.clone(), discovery);
        Ok(())
    }

    fn get_candidate(
        &self,
        discovery_id: &str,
        candidate_id: &str,
        now_unix_millis: u128,
    ) -> Result<Option<PendingSaveDirectoryCandidate>> {
        let mut discoveries = self
            .discoveries
            .lock()
            .expect("pending save directory store lock");

        discoveries.retain(|_, discovery| discovery.expires_at_unix_millis > now_unix_millis);

        Ok(discoveries
            .get(discovery_id)
            .and_then(|discovery| {
                discovery
                    .candidates
                    .iter()
                    .find(|candidate| candidate.summary.candidate_id == candidate_id)
            })
            .cloned())
    }
}

#[cfg(test)]
mod pending_save_directory_tests {
    use super::*;
    use hmm_core::{
        GameId, ProfileId, SaveDirectoryCandidateConfidence, SaveDirectoryCandidateSource,
        SaveDirectoryCandidateSummary,
    };
    use std::path::PathBuf;

    #[test]
    fn store_returns_candidate_before_expiry_and_removes_expired_entries() {
        let store = InMemoryPendingSaveDirectoryCandidateStore::default();
        store
            .put(PendingSaveDirectoryDiscovery {
                discovery_id: "discovery-a".to_owned(),
                game_id: GameId::mhw(),
                profile_id: ProfileId::new("default"),
                expires_at_unix_millis: 1_500,
                candidates: vec![candidate("candidate-a")],
            })
            .expect("put");

        assert!(store
            .get_candidate("discovery-a", "candidate-a", 1_000)
            .expect("get")
            .is_some());
        assert!(store
            .get_candidate("discovery-a", "candidate-a", 2_000)
            .expect("expired")
            .is_none());
    }

    #[test]
    fn put_replaces_existing_discovery() {
        let store = InMemoryPendingSaveDirectoryCandidateStore::default();
        store
            .put(discovery(
                "discovery-a",
                3_000,
                vec![candidate("candidate-a")],
            ))
            .expect("put");
        store
            .put(discovery(
                "discovery-a",
                3_000,
                vec![candidate("candidate-b")],
            ))
            .expect("replace");

        assert!(store
            .get_candidate("discovery-a", "candidate-a", 1_000)
            .expect("old candidate")
            .is_none());
        assert!(store
            .get_candidate("discovery-a", "candidate-b", 1_000)
            .expect("new candidate")
            .is_some());
    }

    fn discovery(
        discovery_id: &str,
        expires_at_unix_millis: u128,
        candidates: Vec<PendingSaveDirectoryCandidate>,
    ) -> PendingSaveDirectoryDiscovery {
        PendingSaveDirectoryDiscovery {
            discovery_id: discovery_id.to_owned(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            expires_at_unix_millis,
            candidates,
        }
    }

    fn candidate(candidate_id: &str) -> PendingSaveDirectoryCandidate {
        PendingSaveDirectoryCandidate {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            summary: SaveDirectoryCandidateSummary {
                candidate_id: candidate_id.to_owned(),
                source: SaveDirectoryCandidateSource::SteamUserdata,
                confidence: SaveDirectoryCandidateConfidence::High,
                recommended: true,
                account_name: None,
                avatar_url: None,
                account_label: "Steam user ****1234".to_owned(),
                path_label: "Steam/userdata/<account>/582010/remote".to_owned(),
                last_modified_at: Some(1_000),
                evidence: vec!["Found MHW:I save file".to_owned()],
            },
            account_id_32: 1234,
            directory: PathBuf::from("C:/Synthetic/Steam/userdata/1234/582010/remote"),
        }
    }
}
