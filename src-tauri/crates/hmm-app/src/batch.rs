use hmac::{Hmac, Mac};
use hmm_core::{
    build_batch_plan, BatchPlan, BatchPlanError, BatchPlanRequest, BatchResourceLimits,
    NormalizedBatchPlanRequest, DEFAULT_BATCH_PREVIEW_TOKEN_TTL_MILLIS,
};
use hmm_ports::{AppClock, BatchPlanFactsProvider, BatchSealRepository, BatchSealRequest};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchPlanPreview {
    pub plan: BatchPlan,
    pub preview_token: Option<String>,
    pub expires_at_unix_millis: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchPlanSealResult {
    pub batch_id: String,
    pub status: &'static str,
    pub operation: hmm_core::BatchOperation,
    pub execution_policy: hmm_core::BatchExecutionPolicy,
    pub plan_token: String,
    pub expires_at_unix_millis: u128,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BatchPlanPreviewError {
    #[error("batch request is invalid")]
    InvalidInput,
    #[error("batch facts are unavailable")]
    FactsUnavailable,
    #[error("batch plan could not be built")]
    PlanBuildFailed,
    #[error("batch resource limit exceeded")]
    ResourceLimitExceeded,
    #[error("batch contains a duplicate Mod item")]
    DuplicateItem,
    #[error("batch clock is unavailable")]
    ClockUnavailable,
    #[error("batch preview token could not be issued")]
    TokenIssueFailed,
}

impl BatchPlanPreviewError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput => "batch_input_invalid",
            Self::FactsUnavailable => "batch_evidence_unavailable",
            Self::PlanBuildFailed => "batch_internal_error",
            Self::ResourceLimitExceeded => "batch_resource_limit_exceeded",
            Self::DuplicateItem => "batch_duplicate_item",
            Self::ClockUnavailable => "batch_internal_error",
            Self::TokenIssueFailed => "batch_internal_error",
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BatchPlanSealError {
    #[error("batch request is invalid")]
    InvalidInput,
    #[error("batch facts are unavailable")]
    FactsUnavailable,
    #[error("batch plan could not be built")]
    PlanBuildFailed,
    #[error("batch resource limit exceeded")]
    ResourceLimitExceeded,
    #[error("batch contains a duplicate Mod item")]
    DuplicateItem,
    #[error("batch plan is blocked")]
    PlanBlocked,
    #[error("batch plan is stale")]
    Stale,
    #[error("batch preview token is invalid")]
    InvalidToken,
    #[error("batch preview token is expired")]
    Expired,
    #[error("batch clock is unavailable")]
    ClockUnavailable,
    #[error("batch plan token could not be issued")]
    TokenIssueFailed,
    #[error("batch seal failed")]
    SealFailed,
}

impl BatchPlanSealError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput => "batch_input_invalid",
            Self::FactsUnavailable => "batch_evidence_unavailable",
            Self::PlanBuildFailed => "batch_internal_error",
            Self::ResourceLimitExceeded => "batch_resource_limit_exceeded",
            Self::DuplicateItem => "batch_duplicate_item",
            Self::PlanBlocked => "batch_plan_blocked",
            Self::Stale => "batch_plan_stale",
            Self::InvalidToken => "batch_token_invalid",
            Self::Expired => "batch_plan_expired",
            Self::ClockUnavailable => "batch_internal_error",
            Self::TokenIssueFailed => "batch_internal_error",
            Self::SealFailed => "batch_journal_unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchTokenKind {
    Preview,
    Plan,
}

impl BatchTokenKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Plan => "plan",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchTokenMaterial {
    pub token: String,
    pub verifier: String,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BatchTokenError {
    #[error("batch token is invalid")]
    Invalid,
    #[error("batch token is expired")]
    Expired,
    #[error("batch token does not match the current plan")]
    Mismatch,
}

pub trait BatchTokenCodec: Send + Sync {
    fn issue(
        &self,
        kind: BatchTokenKind,
        digest: &str,
        environment_digest: &str,
        issued_at_unix_millis: u128,
        expires_at_unix_millis: u128,
    ) -> anyhow::Result<BatchTokenMaterial>;

    fn verify(
        &self,
        kind: BatchTokenKind,
        token: &str,
        digest: &str,
        environment_digest: &str,
        now_unix_millis: u128,
    ) -> Result<(), BatchTokenError>;
}

#[derive(Clone)]
pub struct Sha256BatchTokenCodec {
    secret: Arc<Vec<u8>>,
}

impl Sha256BatchTokenCodec {
    pub fn new(secret: impl AsRef<[u8]>) -> anyhow::Result<Self> {
        let secret = secret.as_ref();
        anyhow::ensure!(!secret.is_empty(), "batch token secret is empty");
        Ok(Self {
            secret: Arc::new(secret.to_vec()),
        })
    }
}

impl BatchTokenCodec for Sha256BatchTokenCodec {
    fn issue(
        &self,
        kind: BatchTokenKind,
        digest: &str,
        environment_digest: &str,
        _issued_at_unix_millis: u128,
        expires_at_unix_millis: u128,
    ) -> anyhow::Result<BatchTokenMaterial> {
        anyhow::ensure!(!self.secret.is_empty(), "batch token secret is empty");
        let signature = sign_token(
            &self.secret,
            kind,
            digest,
            environment_digest,
            expires_at_unix_millis,
        );
        let token = format!(
            "hmm-batch-v1.{}.{}.{}",
            kind.as_str(),
            expires_at_unix_millis,
            signature
        );
        Ok(BatchTokenMaterial {
            verifier: sha256_hex(token.as_bytes()),
            token,
        })
    }

    fn verify(
        &self,
        kind: BatchTokenKind,
        token: &str,
        digest: &str,
        environment_digest: &str,
        now_unix_millis: u128,
    ) -> Result<(), BatchTokenError> {
        let parts = token.split('.').collect::<Vec<_>>();
        if parts.len() != 4 || parts[0] != "hmm-batch-v1" || parts[1] != kind.as_str() {
            return Err(BatchTokenError::Invalid);
        }
        let expires_at = parts[2]
            .parse::<u128>()
            .map_err(|_| BatchTokenError::Invalid)?;
        let expected = sign_token(&self.secret, kind, digest, environment_digest, expires_at);
        if !constant_time_eq(parts[3].as_bytes(), expected.as_bytes()) {
            return Err(BatchTokenError::Mismatch);
        }
        if now_unix_millis >= expires_at {
            return Err(BatchTokenError::Expired);
        }
        Ok(())
    }
}

fn sign_token(
    secret: &[u8],
    kind: BatchTokenKind,
    digest: &str,
    environment_digest: &str,
    expires_at_unix_millis: u128,
) -> String {
    let message = format!(
        "hmm-batch-token-v1\0{}\0{}\0{}\0{}",
        kind.as_str(),
        digest,
        environment_digest,
        expires_at_unix_millis
    );
    let mut mac = Hmac::<Sha256>::new_from_slice(secret)
        .expect("token codec validates that the secret is non-empty");
    mac.update(message.as_bytes());
    sha256_hex(&mac.finalize().into_bytes())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |accumulator, (left, right)| {
            accumulator | (left ^ right)
        })
        == 0
}

#[derive(Clone)]
pub struct BatchPlanService {
    facts: Arc<dyn BatchPlanFactsProvider>,
    seal_repository: Arc<dyn BatchSealRepository>,
    clock: Arc<dyn AppClock>,
    token_codec: Arc<dyn BatchTokenCodec>,
    resource_limits: BatchResourceLimits,
}

impl BatchPlanService {
    pub fn new(
        facts: Arc<dyn BatchPlanFactsProvider>,
        seal_repository: Arc<dyn BatchSealRepository>,
        clock: Arc<dyn AppClock>,
        token_codec: Arc<dyn BatchTokenCodec>,
    ) -> Self {
        Self {
            facts,
            seal_repository,
            clock,
            token_codec,
            resource_limits: BatchResourceLimits::default(),
        }
    }

    pub fn with_resource_limits(mut self, resource_limits: BatchResourceLimits) -> Self {
        self.resource_limits = resource_limits;
        self
    }

    pub fn preview(
        &self,
        request: BatchPlanRequest,
    ) -> Result<BatchPlanPreview, BatchPlanPreviewError> {
        let normalized = request
            .normalize_with_max_items(self.resource_limits.max_items)
            .map_err(map_plan_error_to_preview_error)?;
        let plan = self.build_plan(&normalized)?;
        if !plan.is_ready() {
            return Ok(BatchPlanPreview {
                plan,
                preview_token: None,
                expires_at_unix_millis: None,
            });
        }
        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| BatchPlanPreviewError::ClockUnavailable)?;
        let expires_at = now.saturating_add(DEFAULT_BATCH_PREVIEW_TOKEN_TTL_MILLIS);
        let token = self
            .token_codec
            .issue(
                BatchTokenKind::Preview,
                &plan.batch_digest,
                &plan.environment_digest,
                now,
                expires_at,
            )
            .map_err(|_| BatchPlanPreviewError::TokenIssueFailed)?;
        Ok(BatchPlanPreview {
            plan,
            preview_token: Some(token.token),
            expires_at_unix_millis: Some(expires_at),
        })
    }

    pub fn seal(
        &self,
        request: BatchPlanRequest,
        preview_token: &str,
    ) -> Result<BatchPlanSealResult, BatchPlanSealError> {
        let normalized = request
            .normalize_with_max_items(self.resource_limits.max_items)
            .map_err(map_plan_error_to_seal_error)?;
        let plan = self.build_plan_for_seal_internal(&normalized)?;
        let now = self
            .clock
            .now_unix_millis()
            .map_err(|_| BatchPlanSealError::ClockUnavailable)?;
        match self.token_codec.verify(
            BatchTokenKind::Preview,
            preview_token,
            &plan.batch_digest,
            &plan.environment_digest,
            now,
        ) {
            Ok(()) => {}
            Err(BatchTokenError::Expired) => return Err(BatchPlanSealError::Expired),
            Err(BatchTokenError::Mismatch) => return Err(BatchPlanSealError::Stale),
            Err(BatchTokenError::Invalid) => return Err(BatchPlanSealError::InvalidToken),
        }
        if !plan.is_ready() {
            return Err(BatchPlanSealError::PlanBlocked);
        }
        let expires_at = now.saturating_add(DEFAULT_BATCH_PREVIEW_TOKEN_TTL_MILLIS);
        let plan_token = self
            .token_codec
            .issue(
                BatchTokenKind::Plan,
                &plan.batch_digest,
                &plan.environment_digest,
                now,
                expires_at,
            )
            .map_err(|_| BatchPlanSealError::TokenIssueFailed)?;
        let batch_id = self
            .seal_repository
            .seal_batch(BatchSealRequest {
                request: &normalized,
                plan: &plan,
                plan_token_verifier: &plan_token.verifier,
                expires_at_unix_millis: expires_at,
            })
            .map_err(|_| BatchPlanSealError::SealFailed)?;
        Ok(BatchPlanSealResult {
            batch_id,
            status: "sealed",
            operation: plan.operation,
            execution_policy: plan.execution_policy,
            plan_token: plan_token.token,
            expires_at_unix_millis: expires_at,
        })
    }

    fn build_plan(
        &self,
        request: &NormalizedBatchPlanRequest,
    ) -> Result<BatchPlan, BatchPlanPreviewError> {
        let facts = self
            .facts
            .read_batch_plan_facts(request)
            .map_err(|_| BatchPlanPreviewError::FactsUnavailable)?;
        build_batch_plan(request.clone(), facts, self.resource_limits.clone()).map_err(|error| {
            match error {
                BatchPlanError::ResourceLimitExceeded { .. } => {
                    BatchPlanPreviewError::ResourceLimitExceeded
                }
                BatchPlanError::DuplicateItem => BatchPlanPreviewError::DuplicateItem,
                BatchPlanError::InvalidInput | BatchPlanError::InputOperationMismatch => {
                    BatchPlanPreviewError::InvalidInput
                }
                BatchPlanError::FactsMismatch | BatchPlanError::FactsUnavailable => {
                    BatchPlanPreviewError::PlanBuildFailed
                }
            }
        })
    }
}

fn map_plan_error_to_preview_error(error: BatchPlanError) -> BatchPlanPreviewError {
    match error {
        BatchPlanError::InvalidInput | BatchPlanError::InputOperationMismatch => {
            BatchPlanPreviewError::InvalidInput
        }
        BatchPlanError::DuplicateItem => BatchPlanPreviewError::DuplicateItem,
        BatchPlanError::ResourceLimitExceeded { .. } => {
            BatchPlanPreviewError::ResourceLimitExceeded
        }
        BatchPlanError::FactsMismatch | BatchPlanError::FactsUnavailable => {
            BatchPlanPreviewError::PlanBuildFailed
        }
    }
}

fn map_plan_error_to_seal_error(error: BatchPlanError) -> BatchPlanSealError {
    match error {
        BatchPlanError::InvalidInput | BatchPlanError::InputOperationMismatch => {
            BatchPlanSealError::InvalidInput
        }
        BatchPlanError::DuplicateItem => BatchPlanSealError::DuplicateItem,
        BatchPlanError::ResourceLimitExceeded { .. } => BatchPlanSealError::ResourceLimitExceeded,
        BatchPlanError::FactsMismatch | BatchPlanError::FactsUnavailable => {
            BatchPlanSealError::PlanBuildFailed
        }
    }
}

impl BatchPlanService {
    fn build_plan_for_seal_internal(
        &self,
        request: &NormalizedBatchPlanRequest,
    ) -> Result<BatchPlan, BatchPlanSealError> {
        let facts = self
            .facts
            .read_batch_plan_facts(request)
            .map_err(|_| BatchPlanSealError::FactsUnavailable)?;
        build_batch_plan(request.clone(), facts, self.resource_limits.clone()).map_err(|error| {
            match error {
                BatchPlanError::ResourceLimitExceeded { .. } => {
                    BatchPlanSealError::ResourceLimitExceeded
                }
                BatchPlanError::DuplicateItem => BatchPlanSealError::DuplicateItem,
                BatchPlanError::InvalidInput | BatchPlanError::InputOperationMismatch => {
                    BatchPlanSealError::InvalidInput
                }
                BatchPlanError::FactsMismatch | BatchPlanError::FactsUnavailable => {
                    BatchPlanSealError::PlanBuildFailed
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        BatchActionSummary, BatchExecutionPolicy, BatchItemFacts, BatchItemInput, BatchOperation,
        BatchPlanFacts, BatchPreflightDecision, BatchPreflightStatus, BatchTargetClaim,
        BatchTargetWriteKind, FileLayer, GameId, InstallBatchItemInput, InstallTargetPath, ModId,
        ModRevisionId, ProfileId,
    };
    use std::sync::Mutex;

    struct FakeFacts {
        facts: Mutex<BatchPlanFacts>,
        reads: Mutex<usize>,
    }

    impl BatchPlanFactsProvider for FakeFacts {
        fn read_batch_plan_facts(
            &self,
            _request: &NormalizedBatchPlanRequest,
        ) -> anyhow::Result<BatchPlanFacts> {
            *self.reads.lock().expect("reads") += 1;
            Ok(self.facts.lock().expect("facts").clone())
        }
    }

    #[derive(Default)]
    struct FakeSeal {
        calls: Mutex<usize>,
    }

    impl BatchSealRepository for FakeSeal {
        fn seal_batch(&self, _request: BatchSealRequest<'_>) -> anyhow::Result<String> {
            *self.calls.lock().expect("calls") += 1;
            Ok("batch-1".to_owned())
        }
    }

    struct FixedClock(u128);

    impl AppClock for FixedClock {
        fn now_unix_millis(&self) -> anyhow::Result<u128> {
            Ok(self.0)
        }
    }

    fn request() -> BatchPlanRequest {
        BatchPlanRequest {
            schema_version: hmm_core::BATCH_PLAN_SCHEMA_VERSION,
            operation: BatchOperation::Install,
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            execution_policy: BatchExecutionPolicy::StopOnFailure,
            items: vec![BatchItemInput::Install(InstallBatchItemInput {
                mod_id: ModId::new("mod-a"),
                revision_id: ModRevisionId::new("revision-a"),
                layer: FileLayer::new("default", 1),
                replacement_binding_snapshot: None,
            })],
        }
    }

    fn facts() -> BatchPlanFacts {
        BatchPlanFacts {
            environment_digest: "env-a".to_owned(),
            prerequisite_rules_version: Some(1),
            items: vec![BatchItemFacts {
                mod_id: ModId::new("mod-a"),
                source_revision_id: Some(ModRevisionId::new("revision-a")),
                installed_revision_id: None,
                fact_digest: "fact-a".to_owned(),
                single_plan_digest: "plan-a".to_owned(),
                target_claims: vec![BatchTargetClaim {
                    target_path: InstallTargetPath::parse("nativepc/a", ["nativepc"])
                        .expect("target"),
                    kind: BatchTargetWriteKind::Install,
                }],
                action_summary: BatchActionSummary {
                    actions: 1,
                    ..Default::default()
                },
                prerequisite: BatchPreflightDecision {
                    status: BatchPreflightStatus::Ready,
                    rules_version: Some(1),
                    codes: Vec::new(),
                },
                blocking_reasons: Vec::new(),
                warning_codes: Vec::new(),
            }],
        }
    }

    fn service(facts: Arc<FakeFacts>, seal: Arc<FakeSeal>, clock: u128) -> BatchPlanService {
        BatchPlanService::new(
            facts,
            seal,
            Arc::new(FixedClock(clock)),
            Arc::new(Sha256BatchTokenCodec::new("test-secret").expect("secret")),
        )
    }

    #[test]
    fn token_codec_rejects_empty_secrets() {
        assert!(Sha256BatchTokenCodec::new([]).is_err());
    }

    #[test]
    fn preview_is_read_only_and_returns_token_only_when_ready() {
        let facts = Arc::new(FakeFacts {
            facts: Mutex::new(facts()),
            reads: Mutex::new(0),
        });
        let seal = Arc::new(FakeSeal::default());
        let preview = service(Arc::clone(&facts), Arc::clone(&seal), 100).preview(request());
        assert!(preview.as_ref().expect("preview").preview_token.is_some());
        assert_eq!(*seal.calls.lock().expect("calls"), 0);
        assert_eq!(*facts.reads.lock().expect("reads"), 1);
    }

    #[test]
    fn blocked_preview_does_not_issue_token_or_write() {
        let mut blocked_facts = facts();
        blocked_facts.items[0]
            .blocking_reasons
            .push("source_revision_changed".to_owned());
        let facts = Arc::new(FakeFacts {
            facts: Mutex::new(blocked_facts),
            reads: Mutex::new(0),
        });
        let seal = Arc::new(FakeSeal::default());
        let service = service(Arc::clone(&facts), Arc::clone(&seal), 100);
        let preview = service.preview(request()).expect("preview");
        assert_eq!(preview.plan.status(), hmm_core::BatchPlanStatus::Blocked);
        assert!(preview.preview_token.is_none());
        assert_eq!(*seal.calls.lock().expect("calls"), 0);
    }

    #[test]
    fn valid_token_cannot_seal_a_blocked_plan() {
        let mut blocked_facts = facts();
        blocked_facts.items[0]
            .blocking_reasons
            .push("source_revision_changed".to_owned());
        let normalized = request().normalize().expect("request");
        let plan = hmm_core::build_batch_plan(
            normalized,
            blocked_facts.clone(),
            BatchResourceLimits::default(),
        )
        .expect("plan");
        let codec = Sha256BatchTokenCodec::new("test-secret").expect("secret");
        let token = codec
            .issue(
                BatchTokenKind::Preview,
                &plan.batch_digest,
                &plan.environment_digest,
                100,
                200,
            )
            .expect("token")
            .token;
        let facts = Arc::new(FakeFacts {
            facts: Mutex::new(blocked_facts),
            reads: Mutex::new(0),
        });
        let seal = Arc::new(FakeSeal::default());
        let service = service(Arc::clone(&facts), Arc::clone(&seal), 100);
        assert_eq!(
            service.seal(request(), &token),
            Err(BatchPlanSealError::PlanBlocked)
        );
        assert_eq!(*seal.calls.lock().expect("calls"), 0);
    }

    #[test]
    fn seal_re_reads_facts_and_rejects_drift_without_repository_write() {
        let facts = Arc::new(FakeFacts {
            facts: Mutex::new(facts()),
            reads: Mutex::new(0),
        });
        let seal = Arc::new(FakeSeal::default());
        let service = service(Arc::clone(&facts), Arc::clone(&seal), 100);
        let preview = service.preview(request()).expect("preview");
        facts.facts.lock().expect("facts").environment_digest = "env-b".to_owned();
        let result = service.seal(request(), preview.preview_token.as_deref().expect("token"));
        assert_eq!(result, Err(BatchPlanSealError::Stale));
        assert_eq!(*seal.calls.lock().expect("calls"), 0);
    }

    #[test]
    fn seal_rejects_request_drift_without_repository_write() {
        let facts = Arc::new(FakeFacts {
            facts: Mutex::new(facts()),
            reads: Mutex::new(0),
        });
        let seal = Arc::new(FakeSeal::default());
        let service = service(Arc::clone(&facts), Arc::clone(&seal), 100);
        let preview = service.preview(request()).expect("preview");
        let mut changed_request = request();
        if let BatchItemInput::Install(input) = &mut changed_request.items[0] {
            input.revision_id = ModRevisionId::new("revision-b");
        }
        let result = service.seal(
            changed_request,
            preview.preview_token.as_deref().expect("token"),
        );
        assert_eq!(result, Err(BatchPlanSealError::Stale));
        assert_eq!(*seal.calls.lock().expect("calls"), 0);
    }

    #[test]
    fn seal_persists_once_and_returns_opaque_plan_token() {
        let facts = Arc::new(FakeFacts {
            facts: Mutex::new(facts()),
            reads: Mutex::new(0),
        });
        let seal = Arc::new(FakeSeal::default());
        let service = service(Arc::clone(&facts), Arc::clone(&seal), 100);
        let preview = service.preview(request()).expect("preview");
        let sealed = service
            .seal(request(), preview.preview_token.as_deref().expect("token"))
            .expect("seal");
        assert_eq!(sealed.batch_id, "batch-1");
        assert_eq!(sealed.status, "sealed");
        assert_eq!(*seal.calls.lock().expect("calls"), 1);
        assert!(!sealed.plan_token.contains(&preview.plan.batch_digest));
    }

    #[test]
    fn expired_preview_token_fails_closed() {
        let facts = Arc::new(FakeFacts {
            facts: Mutex::new(facts()),
            reads: Mutex::new(0),
        });
        let seal = Arc::new(FakeSeal::default());
        let initial_service = service(Arc::clone(&facts), Arc::clone(&seal), 100);
        let preview = initial_service.preview(request()).expect("preview");
        let expired_service = service(
            Arc::clone(&facts),
            Arc::clone(&seal),
            100 + DEFAULT_BATCH_PREVIEW_TOKEN_TTL_MILLIS,
        );
        let result =
            expired_service.seal(request(), preview.preview_token.as_deref().expect("token"));
        assert_eq!(result, Err(BatchPlanSealError::Expired));
        assert_eq!(*seal.calls.lock().expect("calls"), 0);
    }

    #[test]
    fn tampered_expiry_is_not_reported_as_expired() {
        let facts = Arc::new(FakeFacts {
            facts: Mutex::new(facts()),
            reads: Mutex::new(0),
        });
        let seal = Arc::new(FakeSeal::default());
        let service = service(Arc::clone(&facts), Arc::clone(&seal), 100);
        let preview = service.preview(request()).expect("preview");
        let token = preview.preview_token.expect("token");
        let mut parts = token.split('.').map(str::to_owned).collect::<Vec<_>>();
        parts[2] = "0".to_owned();
        let tampered = parts.join(".");
        let result = service.seal(request(), &tampered);
        assert_eq!(result, Err(BatchPlanSealError::Stale));
        assert_eq!(*seal.calls.lock().expect("calls"), 0);
    }

    #[test]
    fn malformed_preview_token_is_rejected_without_repository_write() {
        let facts = Arc::new(FakeFacts {
            facts: Mutex::new(facts()),
            reads: Mutex::new(0),
        });
        let seal = Arc::new(FakeSeal::default());
        let service = service(Arc::clone(&facts), Arc::clone(&seal), 100);
        let result = service.seal(request(), "not-a-batch-token");
        assert_eq!(result, Err(BatchPlanSealError::InvalidToken));
        assert_eq!(*seal.calls.lock().expect("calls"), 0);
    }
}
