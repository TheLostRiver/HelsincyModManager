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
    fn put(&self, discovery: PendingSaveDirectoryDiscovery, now_unix_millis: u128) -> Result<()> {
        let mut discoveries = self
            .discoveries
            .lock()
            .expect("pending save directory store lock");
        sweep_expired(&mut discoveries, now_unix_millis);
        discoveries.insert(discovery.discovery_id.clone(), discovery);
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

        sweep_expired(&mut discoveries, now_unix_millis);

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

    fn consume_candidate(
        &self,
        discovery_id: &str,
        candidate_id: &str,
        now_unix_millis: u128,
    ) -> Result<Option<PendingSaveDirectoryCandidate>> {
        let mut discoveries = self
            .discoveries
            .lock()
            .expect("pending save directory store lock");
        sweep_expired(&mut discoveries, now_unix_millis);

        let candidate = discoveries.get(discovery_id).and_then(|discovery| {
            discovery
                .candidates
                .iter()
                .find(|candidate| candidate.summary.candidate_id == candidate_id)
                .cloned()
        });

        if candidate.is_some() {
            discoveries.remove(discovery_id);
        }

        Ok(candidate)
    }
}

fn sweep_expired(
    discoveries: &mut HashMap<String, PendingSaveDirectoryDiscovery>,
    now_unix_millis: u128,
) {
    discoveries.retain(|_, discovery| discovery.expires_at_unix_millis > now_unix_millis);
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
            .put(
                PendingSaveDirectoryDiscovery {
                    discovery_id: "discovery-a".to_owned(),
                    game_id: GameId::mhw(),
                    profile_id: ProfileId::new("default"),
                    expires_at_unix_millis: 1_500,
                    candidates: vec![candidate("candidate-a")],
                },
                1_000,
            )
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
            .put(
                discovery("discovery-a", 3_000, vec![candidate("candidate-a")]),
                1_000,
            )
            .expect("put");
        store
            .put(
                discovery("discovery-a", 3_000, vec![candidate("candidate-b")]),
                1_000,
            )
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

    #[test]
    fn consume_candidate_returns_candidate_once() {
        let store = InMemoryPendingSaveDirectoryCandidateStore::default();
        store
            .put(
                discovery("discovery-a", 3_000, vec![candidate("candidate-a")]),
                1_000,
            )
            .expect("put");

        assert!(store
            .consume_candidate("discovery-a", "candidate-a", 1_000)
            .expect("first consume")
            .is_some());
        assert!(store
            .consume_candidate("discovery-a", "candidate-a", 1_000)
            .expect("second consume")
            .is_none());
    }

    #[test]
    fn consume_candidate_invalidates_sibling_candidates_in_same_discovery() {
        let store = InMemoryPendingSaveDirectoryCandidateStore::default();
        store
            .put(
                discovery(
                    "discovery-a",
                    3_000,
                    vec![candidate("candidate-a"), candidate("candidate-b")],
                ),
                1_000,
            )
            .expect("put");

        assert!(store
            .consume_candidate("discovery-a", "candidate-a", 1_000)
            .expect("consume a")
            .is_some());
        assert!(store
            .get_candidate("discovery-a", "candidate-b", 1_000)
            .expect("sibling lookup")
            .is_none());
    }

    #[test]
    fn put_sweeps_expired_entries_using_supplied_now() {
        let store = InMemoryPendingSaveDirectoryCandidateStore::default();
        store
            .put(
                discovery("expired-before-next", 1_000, vec![candidate("candidate-a")]),
                500,
            )
            .expect("put old");
        store
            .put(
                discovery("new-discovery", 2_000, vec![candidate("candidate-b")]),
                1_500,
            )
            .expect("put new");

        assert!(store
            .get_candidate("expired-before-next", "candidate-a", 1_500)
            .expect("old discovery was swept on put")
            .is_none());
        assert!(store
            .get_candidate("new-discovery", "candidate-b", 999)
            .expect("new discovery")
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
