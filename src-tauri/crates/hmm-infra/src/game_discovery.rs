use hmm_core::GameId;
use hmm_ports::{GameCandidate, GameDiscoveryError, GameDiscoveryService};

pub struct NoopGameDiscoveryService;

impl GameDiscoveryService for NoopGameDiscoveryService {
    fn scan_candidates(
        &self,
        _game_id: &GameId,
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

        let error = service
            .scan_candidates(&GameId::mhw())
            .expect_err("scan is disabled");

        assert_eq!(error, GameDiscoveryError::ScanNotImplemented);
    }
}
