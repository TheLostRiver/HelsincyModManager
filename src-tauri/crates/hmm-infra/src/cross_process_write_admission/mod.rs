#[cfg(not(windows))]
mod file_lock;
#[cfg(windows)]
mod windows_mutex;

use hmm_ports::{CrossProcessWriteScope, CrossProcessWriteScopeKind};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, marker::PhantomData, rc::Rc};
use thiserror::Error;

#[cfg(not(windows))]
pub use file_lock::PlatformCrossProcessWriteAdmission;
#[cfg(windows)]
pub use windows_mutex::PlatformCrossProcessWriteAdmission;

const WRITE_ADMISSION_SCHEMA: &str = "hmm.write-admission/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PlatformCrossProcessWriteAdmissionInitError {
    #[error("cross-process write admission namespace is unavailable")]
    NamespaceUnavailable,
    #[error("cross-process write admission user identity is unavailable")]
    IdentityUnavailable,
}

impl PlatformCrossProcessWriteAdmissionInitError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::NamespaceUnavailable => "write_admission_namespace_unavailable",
            Self::IdentityUnavailable => "write_admission_identity_unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ScopeOrderKey {
    rank: u8,
    identity: String,
}

impl ScopeOrderKey {
    fn from_scope(scope: &CrossProcessWriteScope) -> Self {
        let identity = match scope.game_profile_identity() {
            Some((game_id, profile_id)) => {
                format!("{}\0{}", game_id.as_str(), profile_id.as_str())
            }
            None => String::new(),
        };
        Self {
            rank: scope.kind().order_rank(),
            identity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HeldScope {
    namespace: String,
    key: ScopeOrderKey,
    kind: CrossProcessWriteScopeKind,
}

thread_local! {
    static HELD_SCOPES: RefCell<Vec<HeldScope>> = const { RefCell::new(Vec::new()) };
}

struct HeldScopeOrderGuard {
    held: Option<HeldScope>,
    _thread_affinity: PhantomData<Rc<()>>,
}

impl HeldScopeOrderGuard {
    fn validate(namespace: &str, scope: &CrossProcessWriteScope) -> Result<ScopeOrderKey, ()> {
        let key = ScopeOrderKey::from_scope(scope);
        let valid = HELD_SCOPES.with(|held| {
            held.borrow()
                .iter()
                .rev()
                .find(|item| item.namespace == namespace)
                .is_none_or(|current| current.key < key)
        });
        valid.then_some(key).ok_or(())
    }

    fn register(namespace: &str, key: ScopeOrderKey, kind: CrossProcessWriteScopeKind) -> Self {
        let held = HeldScope {
            namespace: namespace.to_owned(),
            key,
            kind,
        };
        HELD_SCOPES.with(|scopes| scopes.borrow_mut().push(held.clone()));
        Self {
            held: Some(held),
            _thread_affinity: PhantomData,
        }
    }
}

impl Drop for HeldScopeOrderGuard {
    fn drop(&mut self) {
        let Some(held) = self.held.take() else {
            return;
        };
        HELD_SCOPES.with(|scopes| {
            let mut scopes = scopes.borrow_mut();
            if scopes.last() == Some(&held) {
                scopes.pop();
                return;
            }
            if let Some(index) = scopes.iter().rposition(|candidate| candidate == &held) {
                scopes.remove(index);
            }
            tracing::error!(
                event = "write_admission_release_order_violation",
                scope = held.kind.as_str(),
                result = "failure"
            );
        });
    }
}

fn scope_digest(scope: &CrossProcessWriteScope) -> String {
    let mut hasher = Sha256::new();
    update_digest_part(&mut hasher, WRITE_ADMISSION_SCHEMA.as_bytes());
    update_digest_part(&mut hasher, scope.kind().as_str().as_bytes());
    if let Some((game_id, profile_id)) = scope.game_profile_identity() {
        update_digest_part(&mut hasher, game_id.as_str().as_bytes());
        update_digest_part(&mut hasher, profile_id.as_str().as_bytes());
    }
    hex_digest(hasher.finalize())
}

fn namespace_digest(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    update_digest_part(&mut hasher, WRITE_ADMISSION_SCHEMA.as_bytes());
    for part in parts {
        update_digest_part(&mut hasher, part);
    }
    hex_digest(hasher.finalize())
}

fn update_digest_part(hasher: &mut Sha256, part: &[u8]) {
    hasher.update((part.len() as u64).to_le_bytes());
    hasher.update(part);
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameId, ProfileId};

    #[test]
    fn scope_digest_does_not_expose_identity() {
        let scope = CrossProcessWriteScope::save_profile(
            &GameId::mhw(),
            &ProfileId::new("private-profile"),
        );
        let digest = scope_digest(&scope);

        assert_eq!(digest.len(), 64);
        assert!(!digest.contains("mhw"));
        assert!(!digest.contains("private-profile"));
    }

    #[test]
    fn fixed_scope_order_accepts_save_then_game_and_rejects_reverse_or_reentry() {
        let namespace = "order-test";
        let game_id = GameId::mhw();
        let profile_id = ProfileId::new("profile-a");
        let save = CrossProcessWriteScope::save_profile(&game_id, &profile_id);
        let game = CrossProcessWriteScope::game_profile(&game_id, &profile_id);

        let save_key = HeldScopeOrderGuard::validate(namespace, &save).expect("save first");
        let save_guard = HeldScopeOrderGuard::register(namespace, save_key, save.kind());
        assert!(HeldScopeOrderGuard::validate(namespace, &save).is_err());
        let game_key = HeldScopeOrderGuard::validate(namespace, &game).expect("game after save");
        let game_guard = HeldScopeOrderGuard::register(namespace, game_key, game.kind());
        drop(game_guard);
        drop(save_guard);

        let game_key = HeldScopeOrderGuard::validate(namespace, &game).expect("game first");
        let game_guard = HeldScopeOrderGuard::register(namespace, game_key, game.kind());
        assert!(HeldScopeOrderGuard::validate(namespace, &save).is_err());
        drop(game_guard);
    }
}
