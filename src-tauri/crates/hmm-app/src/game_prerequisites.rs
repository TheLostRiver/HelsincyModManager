use crate::{
    BuildImportedModInstallPlanRequest, GameSetupService, GameSetupServiceError,
    InstallPlanningError, InstallPlanningService, PlannedInitialRetargetInstall,
    PreviewInitialRetargetInstallRequest, ReplacementWorkflowError, ReplacementWorkflowService,
};
use hmm_core::{FileLayer, GameId, GameSetupErrorCode, InstallPlan, ModId, ModRevisionId};
use hmm_ports::{
    GamePrerequisiteIssueCode, GamePrerequisiteReport, GamePrerequisiteReportState,
    GamePrerequisiteSummaryStatus,
};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePrerequisiteDecisionStatus {
    Ready,
    Warning,
    Blocked,
}

impl GamePrerequisiteDecisionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Warning => "warning",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GamePrerequisiteDecisionCode {
    GameNotConfigured,
    GameDirectoryInvalid,
    RulesUnavailable,
    RulesCorrupted,
    StorageUnavailable,
    StorageCorrupted,
    UnsupportedGame,
    MissingRequiredFile,
    SignatureUnverified,
    ConfigReadFailed,
    ConfigInvalidJson,
    ConfigFieldMismatch,
    DecisionInvalid,
}

impl GamePrerequisiteDecisionCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GameNotConfigured => "game_not_configured",
            Self::GameDirectoryInvalid => "game_directory_invalid",
            Self::RulesUnavailable => "rules_unavailable",
            Self::RulesCorrupted => "rules_corrupted",
            Self::StorageUnavailable => "storage_unavailable",
            Self::StorageCorrupted => "storage_corrupted",
            Self::UnsupportedGame => "unsupported_game",
            Self::MissingRequiredFile => "missing_required_file",
            Self::SignatureUnverified => "signature_unverified",
            Self::ConfigReadFailed => "config_read_failed",
            Self::ConfigInvalidJson => "config_invalid_json",
            Self::ConfigFieldMismatch => "config_field_mismatch",
            Self::DecisionInvalid => "prerequisite_decision_invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GamePrerequisiteDecision {
    pub game_id: GameId,
    pub status: GamePrerequisiteDecisionStatus,
    pub rules_version: Option<u32>,
    pub codes: Vec<GamePrerequisiteDecisionCode>,
}

impl GamePrerequisiteDecision {
    pub fn from_report(report: GamePrerequisiteReport) -> Self {
        let mut codes = report
            .items
            .iter()
            .flat_map(|item| item.issues.iter())
            .map(|issue| decision_code_from_issue(issue.code.clone()))
            .collect::<BTreeSet<_>>();

        let status = match report.state {
            GamePrerequisiteReportState::NotConfigured => {
                codes.insert(GamePrerequisiteDecisionCode::GameNotConfigured);
                GamePrerequisiteDecisionStatus::Blocked
            }
            GamePrerequisiteReportState::GameDirectoryInvalid => {
                codes.insert(GamePrerequisiteDecisionCode::GameDirectoryInvalid);
                insert_storage_code(&mut codes, report.error_code.as_ref());
                GamePrerequisiteDecisionStatus::Blocked
            }
            GamePrerequisiteReportState::RulesUnavailable => {
                codes.insert(GamePrerequisiteDecisionCode::RulesUnavailable);
                if report.error_code == Some(GameSetupErrorCode::StorageCorrupted) {
                    codes.insert(GamePrerequisiteDecisionCode::RulesCorrupted);
                }
                insert_storage_code(&mut codes, report.error_code.as_ref());
                GamePrerequisiteDecisionStatus::Blocked
            }
            GamePrerequisiteReportState::Ready => match report.summary_status {
                Some(GamePrerequisiteSummaryStatus::Verified) if codes.is_empty() => {
                    GamePrerequisiteDecisionStatus::Ready
                }
                Some(GamePrerequisiteSummaryStatus::Warning) if !codes.is_empty() => {
                    GamePrerequisiteDecisionStatus::Warning
                }
                Some(GamePrerequisiteSummaryStatus::Error) if !codes.is_empty() => {
                    GamePrerequisiteDecisionStatus::Blocked
                }
                _ => {
                    codes.insert(GamePrerequisiteDecisionCode::DecisionInvalid);
                    GamePrerequisiteDecisionStatus::Blocked
                }
            },
        };

        Self {
            game_id: report.game_id,
            status,
            rules_version: report.rules_version,
            codes: codes.into_iter().collect(),
        }
    }

    pub fn is_blocked(&self) -> bool {
        self.status == GamePrerequisiteDecisionStatus::Blocked
    }
}

pub trait GamePrerequisiteDecisionProvider: Send + Sync {
    fn prerequisite_decision(&self, game_id: &GameId) -> GamePrerequisiteDecision;
}

impl GamePrerequisiteDecisionProvider for GameSetupService {
    fn prerequisite_decision(&self, game_id: &GameId) -> GamePrerequisiteDecision {
        match self.get_prerequisite_status(game_id.clone()) {
            Ok(report) => GamePrerequisiteDecision::from_report(report),
            Err(error) => decision_from_service_error(game_id.clone(), error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedModInstallPreflight {
    pub plan: InstallPlan,
    pub prerequisite_decision: GamePrerequisiteDecision,
}

pub struct ImportedModInstallPreflightService {
    planning: Arc<InstallPlanningService>,
    prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
}

impl ImportedModInstallPreflightService {
    pub fn new(
        planning: Arc<InstallPlanningService>,
        prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
    ) -> Self {
        Self {
            planning,
            prerequisites,
        }
    }

    pub fn preview(
        &self,
        request: BuildImportedModInstallPlanRequest,
    ) -> Result<ImportedModInstallPreflight, InstallPlanningError> {
        let prerequisite_decision = self.prerequisites.prerequisite_decision(&request.game_id);
        let plan = self.planning.build_plan_from_imported_mod(request)?;

        Ok(ImportedModInstallPreflight {
            plan,
            prerequisite_decision,
        })
    }

    pub(crate) fn preview_revision(
        &self,
        game_id: &GameId,
        mod_id: &ModId,
        revision_id: &ModRevisionId,
        layer: &FileLayer,
    ) -> Result<ImportedModInstallPreflight, InstallPlanningError> {
        let prerequisite_decision = self.prerequisites.prerequisite_decision(game_id);
        let plan = self.planning.build_plan_from_imported_revision_id(
            game_id,
            mod_id,
            revision_id,
            layer,
        )?;

        Ok(ImportedModInstallPreflight {
            plan,
            prerequisite_decision,
        })
    }

    pub fn prerequisite_decision(&self, game_id: &GameId) -> GamePrerequisiteDecision {
        self.prerequisites.prerequisite_decision(game_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialRetargetInstallPreflight {
    pub planned: PlannedInitialRetargetInstall,
    pub prerequisite_decision: GamePrerequisiteDecision,
}

pub struct InitialRetargetInstallPreflightService {
    workflow: Arc<ReplacementWorkflowService>,
    prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
}

impl InitialRetargetInstallPreflightService {
    pub fn new(
        workflow: Arc<ReplacementWorkflowService>,
        prerequisites: Arc<dyn GamePrerequisiteDecisionProvider>,
    ) -> Self {
        Self {
            workflow,
            prerequisites,
        }
    }

    pub fn preview(
        &self,
        request: PreviewInitialRetargetInstallRequest,
    ) -> Result<InitialRetargetInstallPreflight, ReplacementWorkflowError> {
        let prerequisite_decision = self.prerequisites.prerequisite_decision(&request.game_id);
        let planned = self.workflow.preview_initial_install(request)?;

        Ok(InitialRetargetInstallPreflight {
            planned,
            prerequisite_decision,
        })
    }
}

fn decision_from_service_error(
    game_id: GameId,
    error: GameSetupServiceError,
) -> GamePrerequisiteDecision {
    let code = match error {
        GameSetupServiceError::UnsupportedGame => GamePrerequisiteDecisionCode::UnsupportedGame,
        GameSetupServiceError::StorageCorrupted => GamePrerequisiteDecisionCode::StorageCorrupted,
        GameSetupServiceError::StorageFailed(_) => GamePrerequisiteDecisionCode::StorageUnavailable,
        GameSetupServiceError::ValidationFailed(_)
        | GameSetupServiceError::ScanFailed(_)
        | GameSetupServiceError::ScanNotImplemented
        | GameSetupServiceError::ClockFailed(_) => GamePrerequisiteDecisionCode::DecisionInvalid,
    };

    GamePrerequisiteDecision {
        game_id,
        status: GamePrerequisiteDecisionStatus::Blocked,
        rules_version: None,
        codes: vec![code],
    }
}

fn decision_code_from_issue(code: GamePrerequisiteIssueCode) -> GamePrerequisiteDecisionCode {
    match code {
        GamePrerequisiteIssueCode::MissingRequiredFile => {
            GamePrerequisiteDecisionCode::MissingRequiredFile
        }
        GamePrerequisiteIssueCode::SignatureUnverified => {
            GamePrerequisiteDecisionCode::SignatureUnverified
        }
        GamePrerequisiteIssueCode::ConfigReadFailed => {
            GamePrerequisiteDecisionCode::ConfigReadFailed
        }
        GamePrerequisiteIssueCode::ConfigInvalidJson => {
            GamePrerequisiteDecisionCode::ConfigInvalidJson
        }
        GamePrerequisiteIssueCode::ConfigFieldMismatch => {
            GamePrerequisiteDecisionCode::ConfigFieldMismatch
        }
        GamePrerequisiteIssueCode::RulesUnavailable => {
            GamePrerequisiteDecisionCode::RulesUnavailable
        }
        GamePrerequisiteIssueCode::RulesCorrupted => GamePrerequisiteDecisionCode::RulesCorrupted,
    }
}

fn insert_storage_code(
    codes: &mut BTreeSet<GamePrerequisiteDecisionCode>,
    error_code: Option<&GameSetupErrorCode>,
) {
    match error_code {
        Some(GameSetupErrorCode::StorageFailed) => {
            codes.insert(GamePrerequisiteDecisionCode::StorageUnavailable);
        }
        Some(GameSetupErrorCode::StorageCorrupted) => {
            codes.insert(GamePrerequisiteDecisionCode::StorageCorrupted);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::{GamePrerequisiteIssue, GamePrerequisiteItem, GamePrerequisiteItemStatus};

    #[test]
    fn missing_required_file_is_a_versioned_blocking_decision() {
        let mut item =
            GamePrerequisiteItem::new("loader", "Loader", GamePrerequisiteItemStatus::Missing);
        item.issues.push(GamePrerequisiteIssue::new(
            GamePrerequisiteIssueCode::MissingRequiredFile,
            "dinput8.dll",
        ));

        let decision = GamePrerequisiteDecision::from_report(
            GamePrerequisiteReport::ready_with_rules_version(
                GameId::mhw(),
                Some(7),
                GamePrerequisiteSummaryStatus::Error,
                vec![item],
            ),
        );

        assert_eq!(decision.status, GamePrerequisiteDecisionStatus::Blocked);
        assert_eq!(decision.rules_version, Some(7));
        assert_eq!(
            decision.codes,
            vec![GamePrerequisiteDecisionCode::MissingRequiredFile]
        );
    }

    #[test]
    fn unverified_signature_remains_an_explicit_warning() {
        let mut item = GamePrerequisiteItem::new(
            "loader",
            "Loader",
            GamePrerequisiteItemStatus::InstalledUnverified,
        );
        item.issues.push(GamePrerequisiteIssue::new(
            GamePrerequisiteIssueCode::SignatureUnverified,
            "dinput8.dll",
        ));

        let decision = GamePrerequisiteDecision::from_report(
            GamePrerequisiteReport::ready_with_rules_version(
                GameId::mhw(),
                Some(3),
                GamePrerequisiteSummaryStatus::Warning,
                vec![item],
            ),
        );

        assert_eq!(decision.status, GamePrerequisiteDecisionStatus::Warning);
        assert_eq!(
            decision.codes,
            vec![GamePrerequisiteDecisionCode::SignatureUnverified]
        );
    }

    #[test]
    fn verified_report_is_ready_without_codes() {
        let decision = GamePrerequisiteDecision::from_report(
            GamePrerequisiteReport::ready_with_rules_version(
                GameId::mhw(),
                Some(1),
                GamePrerequisiteSummaryStatus::Verified,
                vec![GamePrerequisiteItem::new(
                    "loader",
                    "Loader",
                    GamePrerequisiteItemStatus::InstalledVerified,
                )],
            ),
        );

        assert_eq!(decision.status, GamePrerequisiteDecisionStatus::Ready);
        assert!(decision.codes.is_empty());
    }

    #[test]
    fn corrupted_rules_fail_closed_with_stable_codes() {
        let decision = GamePrerequisiteDecision::from_report(GamePrerequisiteReport {
            game_id: GameId::mhw(),
            state: GamePrerequisiteReportState::RulesUnavailable,
            rules_version: None,
            summary_status: None,
            items: Vec::new(),
            error_code: Some(GameSetupErrorCode::StorageCorrupted),
            message: Some("must not cross the decision boundary".to_owned()),
        });

        assert_eq!(decision.status, GamePrerequisiteDecisionStatus::Blocked);
        assert_eq!(
            decision.codes,
            vec![
                GamePrerequisiteDecisionCode::RulesUnavailable,
                GamePrerequisiteDecisionCode::RulesCorrupted,
                GamePrerequisiteDecisionCode::StorageCorrupted,
            ]
        );
    }
}
