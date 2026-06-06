use hmm_core::GameId;
use hmm_ports::{GameCandidate, GameDiscoveryError, GameDiscoveryRequest, GameDiscoveryService};

pub struct NoopGameDiscoveryService;

impl GameDiscoveryService for NoopGameDiscoveryService {
    fn scan_candidates(
        &self,
        _request: &GameDiscoveryRequest,
    ) -> Result<Vec<GameCandidate>, GameDiscoveryError> {
        Err(GameDiscoveryError::ScanNotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
