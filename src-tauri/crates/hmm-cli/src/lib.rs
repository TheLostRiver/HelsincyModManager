mod cancellation;
mod contract;
mod task_events;

use cancellation::{CliCancellationCoordinator, NoopCliTaskProgressObserver};
use clap::{Args, Parser, Subcommand, ValueEnum};
use hmm_core::{
    BatchExecutionPolicy, BatchItemInput, BatchOperation, BatchPlanRequest, BatchPlanStatus,
    FileLayer, GameId, ModId, ModRevisionId, ProfileId, ReinstallBatchItemInput,
    ReplacementTargetId, UninstallBatchItemInput, BATCH_PLAN_SCHEMA_VERSION,
};
use hmm_runtime::{
    BackupBackgroundStatusSnapshot, BackupListSnapshot,
    BatchAttemptSnapshot as RuntimeBatchAttemptSnapshot, CliLifecycleAutomation,
    CliLifecycleAutomationError, DiagnosticsSnapshot, GamePrerequisiteSnapshot, GameScanSnapshot,
    GameStatusSnapshot, GameValidationSnapshot, InstallPlanSnapshot,
    InstallRecoveryPreviewSnapshot, InstallRecoveryScanSnapshot, InstallStatusSnapshot,
    LifecycleTaskOutcome, ReadOnlyBackupAutomation, ReadOnlyBackupAutomationError,
    ReadOnlyDiagnosticsAutomation, ReadOnlyDiagnosticsAutomationError, ReadOnlyGameAutomation,
    ReadOnlyGameAutomationError, ReadOnlyInstallAutomation, ReadOnlyInstallAutomationError,
    ReadOnlyInstallRecoveryAction, ReinstallPlanSnapshot, RuntimeEnvironment,
    RuntimeEnvironmentError, RuntimeEnvironmentKind, BatchAutomationError,
    BatchAutomationErrorClass, BatchLifecycleAutomation, BatchLifecyclePlanRequest,
    TaskProgressEvent, TaskProgressObserver, UninstallPlanSnapshot,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub use contract::{
    CliErrorCategory, CliErrorEnvelope, CliExitCode, CliTaskStatus, CommandEnvelope,
    TaskEventEnvelope, TaskEventError, TaskEventType, CLI_SCHEMA_VERSION,
};
pub use task_events::{CliTaskProgressObserver, CliTaskProgressObserverError};

const RUNTIME_STATUS_COMMAND: &str = "runtime.status";
const GAME_STATUS_COMMAND: &str = "game.status";
const GAME_SCAN_COMMAND: &str = "game.scan";
const GAME_VALIDATE_COMMAND: &str = "game.validate";
const GAME_PREREQUISITES_COMMAND: &str = "game.prerequisites";
const INSTALL_PLAN_COMMAND: &str = "install.plan";
const INSTALL_APPLY_COMMAND: &str = "install.apply";
const INSTALL_UNINSTALL_COMMAND: &str = "install.uninstall";
const INSTALL_REINSTALL_COMMAND: &str = "install.reinstall";
const INSTALL_STATUS_COMMAND: &str = "install.status";
const INSTALL_RECOVERY_SCAN_COMMAND: &str = "install.recovery.scan";
const INSTALL_RECOVERY_PREVIEW_COMMAND: &str = "install.recovery.preview";
const INSTALL_RECOVERY_APPLY_COMMAND: &str = "install.recovery.apply";
const INSTALL_BATCH_PLAN_COMMAND: &str = "install.batch.plan";
const INSTALL_BATCH_APPLY_COMMAND: &str = "install.batch.apply";
const INSTALL_BATCH_RESULT_COMMAND: &str = "install.batch.result";
const INSTALL_BATCH_RETRY_COMMAND: &str = "install.batch.retry";
const BACKUP_LIST_COMMAND: &str = "backup.list";
const BACKUP_BACKGROUND_STATUS_COMMAND: &str = "backup.background.status";
const DIAGNOSTICS_SNAPSHOT_COMMAND: &str = "diagnostics.snapshot";
const CLI_PARSE_COMMAND: &str = "cli.parse";
const CLI_USAGE_ERROR: &str = "cli_usage_error";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Jsonl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum EnvironmentOption {
    Production,
    Sandbox,
}

impl From<EnvironmentOption> for RuntimeEnvironmentKind {
    fn from(value: EnvironmentOption) -> Self {
        match value {
            EnvironmentOption::Production => Self::Production,
            EnvironmentOption::Sandbox => Self::Sandbox,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "hmm",
    version,
    about = "Helsincy Mod Manager command-line interface",
    arg_required_else_help = true,
    color = clap::ColorChoice::Never
)]
struct Cli {
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, global = true)]
    format: OutputFormat,

    #[arg(
        long,
        value_enum,
        default_value_t = EnvironmentOption::Production,
        global = true
    )]
    environment: EnvironmentOption,

    #[arg(long, value_name = "PATH", global = true)]
    data_dir: Option<PathBuf>,

    #[arg(long, global = true)]
    no_color: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommand,
    },
    Game {
        #[command(subcommand)]
        command: GameCommand,
    },
    Install {
        #[command(subcommand)]
        command: InstallCommand,
    },
    Backup {
        #[command(subcommand)]
        command: BackupCommand,
    },
    Diagnostics {
        #[command(subcommand)]
        command: DiagnosticsCommand,
    },
}

#[derive(Debug, Subcommand)]
enum RuntimeCommand {
    Status,
}

#[derive(Debug, Subcommand)]
enum GameCommand {
    Status(GameOptions),
    Scan(GameOptions),
    Validate(GameOptions),
    Prerequisites(GameOptions),
}

#[derive(Debug, Subcommand)]
enum InstallCommand {
    Plan(InstallPlanOptions),
    Apply(InstallApplyOptions),
    Batch {
        #[command(subcommand)]
        command: InstallBatchCommand,
    },
    Uninstall(InstallUninstallOptions),
    Reinstall(InstallReinstallOptions),
    Status(InstallStatusOptions),
    Recovery {
        #[command(subcommand)]
        command: InstallRecoveryCommand,
    },
}

#[derive(Debug, Subcommand)]
enum InstallBatchCommand {
    Plan(InstallBatchPlanOptions),
    Apply(InstallBatchApplyOptions),
    Result(InstallBatchResultOptions),
    Retry(InstallBatchRetryOptions),
}

#[derive(Debug, Subcommand)]
enum InstallRecoveryCommand {
    Scan(InstallRecoveryScanOptions),
    Preview(InstallRecoveryPreviewOptions),
    Apply(InstallRecoveryApplyOptions),
}

#[derive(Debug, Subcommand)]
enum BackupCommand {
    List(BackupListOptions),
    Background {
        #[command(subcommand)]
        command: BackupBackgroundCommand,
    },
}

#[derive(Debug, Subcommand)]
enum BackupBackgroundCommand {
    Status(BackupBackgroundStatusOptions),
}

#[derive(Debug, Subcommand)]
enum DiagnosticsCommand {
    Snapshot,
}

#[derive(Debug, Args)]
struct GameOptions {
    #[arg(long, default_value = "mhw")]
    game: String,
}

#[derive(Debug, Args)]
struct InstallPlanOptions {
    #[arg(long, default_value = "mhw")]
    game: String,

    #[arg(long, default_value = "default")]
    profile: String,

    #[arg(long = "mod")]
    mod_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum BatchExecutionPolicyOption {
    StopOnFailure,
    ContinueOnItemFailure,
}

impl From<BatchExecutionPolicyOption> for BatchExecutionPolicy {
    fn from(value: BatchExecutionPolicyOption) -> Self {
        match value {
            BatchExecutionPolicyOption::StopOnFailure => Self::StopOnFailure,
            BatchExecutionPolicyOption::ContinueOnItemFailure => Self::ContinueOnItemFailure,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum BatchOperationOption {
    Install,
    Uninstall,
    Reinstall,
}

impl From<BatchOperationOption> for BatchOperation {
    fn from(value: BatchOperationOption) -> Self {
        match value {
            BatchOperationOption::Install => Self::Install,
            BatchOperationOption::Uninstall => Self::Uninstall,
            BatchOperationOption::Reinstall => Self::Reinstall,
        }
    }
}

#[derive(Debug, Args)]
struct InstallBatchRequestOptions {
    #[arg(long, value_enum, default_value_t = BatchOperationOption::Install)]
    operation: BatchOperationOption,

    #[arg(long, default_value = "mhw")]
    game: String,

    #[arg(long, default_value = "default")]
    profile: String,

    #[arg(long = "item", value_name = "OPERATION_SPECIFIC_ITEM", required = true)]
    items: Vec<String>,

    #[arg(long = "replacement-target", value_name = "MOD=TARGET_ID")]
    replacement_targets: Vec<String>,

    #[arg(long, value_enum, default_value_t = BatchExecutionPolicyOption::StopOnFailure)]
    policy: BatchExecutionPolicyOption,
}

#[derive(Debug, Args)]
struct InstallBatchPlanOptions {
    #[command(flatten)]
    request: InstallBatchRequestOptions,
}

#[derive(Debug, Args)]
struct InstallBatchApplyOptions {
    #[command(flatten)]
    request: InstallBatchRequestOptions,

    #[arg(long)]
    preview_token: Option<String>,

    #[command(flatten)]
    commit: BatchCommitOptions,
}

#[derive(Debug, Args)]
struct InstallBatchResultOptions {
    #[arg(long)]
    batch_id: String,

    #[arg(long, required = true)]
    attempt: u32,
}

#[derive(Debug, Args)]
struct InstallBatchRetryOptions {
    #[arg(long)]
    batch_id: String,

    #[arg(long, required = true)]
    attempt: u32,

    #[command(flatten)]
    commit: BatchCommitOptions,
}

#[derive(Debug, Args)]
struct BatchCommitOptions {
    #[arg(long)]
    commit: bool,

    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct InstallApplyOptions {
    #[arg(long, default_value = "mhw")]
    game: String,

    #[arg(long, default_value = "default")]
    profile: String,

    #[arg(long = "mod")]
    mod_id: String,

    #[command(flatten)]
    lifecycle: LifecycleCommitOptions,
}

#[derive(Debug, Args)]
struct LifecycleCommitOptions {
    #[arg(long)]
    plan_token: Option<String>,

    #[arg(long)]
    commit: bool,

    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct InstallUninstallOptions {
    #[arg(long, default_value = "mhw")]
    game: String,

    #[arg(long, default_value = "default")]
    profile: String,

    #[arg(long = "mod")]
    mod_id: String,

    #[command(flatten)]
    lifecycle: LifecycleCommitOptions,
}

#[derive(Debug, Args)]
struct InstallReinstallOptions {
    #[arg(long, default_value = "mhw")]
    game: String,

    #[arg(long, default_value = "default")]
    profile: String,

    #[arg(long = "mod")]
    mod_id: String,

    #[arg(long = "candidate-revision")]
    candidate_revision_id: String,

    #[command(flatten)]
    lifecycle: LifecycleCommitOptions,
}

#[derive(Debug, Args)]
struct InstallStatusOptions {
    #[arg(long)]
    game: Option<String>,

    #[arg(long)]
    profile: String,

    #[arg(long = "mod", required = true)]
    mod_ids: Vec<String>,
}

#[derive(Debug, Args)]
struct InstallRecoveryScanOptions {
    #[arg(long, default_value = "mhw")]
    game: String,

    #[arg(long)]
    profile: String,

    #[arg(long = "mod")]
    mod_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum InstallRecoveryActionOption {
    RollbackInstall,
    ReconcileReinstall,
}

impl From<InstallRecoveryActionOption> for ReadOnlyInstallRecoveryAction {
    fn from(value: InstallRecoveryActionOption) -> Self {
        match value {
            InstallRecoveryActionOption::RollbackInstall => Self::RollbackInstall,
            InstallRecoveryActionOption::ReconcileReinstall => Self::ReconcileReinstall,
        }
    }
}

#[derive(Debug, Args)]
struct InstallRecoveryPreviewOptions {
    #[arg(long, default_value = "mhw")]
    game: String,

    #[arg(long)]
    profile: String,

    #[arg(long = "mod")]
    mod_id: String,

    #[arg(long, value_enum)]
    action: InstallRecoveryActionOption,
}

#[derive(Debug, Args)]
struct InstallRecoveryApplyOptions {
    #[arg(long, default_value = "mhw")]
    game: String,

    #[arg(long)]
    profile: String,

    #[arg(long = "mod")]
    mod_id: String,

    #[arg(long, value_enum)]
    action: InstallRecoveryActionOption,

    #[command(flatten)]
    lifecycle: LifecycleCommitOptions,
}

#[derive(Debug, Args)]
struct BackupListOptions {
    #[arg(long, default_value = "mhw")]
    game: String,

    #[arg(long)]
    profile: String,

    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Debug, Args)]
struct BackupBackgroundStatusOptions {
    #[arg(long, default_value = "mhw")]
    game: String,

    #[arg(long)]
    profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatusResult {
    environment: &'static str,
    data_root_mode: &'static str,
    write_command_policy: &'static str,
    production_writes_allowed: bool,
    business_commands_available: bool,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum GameCommandResult {
    Status(GameStatusSnapshot),
    Scan(GameScanSnapshot),
    Validation(GameValidationSnapshot),
    Prerequisites(GamePrerequisiteSnapshot),
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum InstallCommandResult {
    Plan(InstallPlanSnapshot),
    BatchPlan(BatchPlanResult),
    BatchAttempt(BatchAttemptResult),
    Uninstall(UninstallPlanSnapshot),
    Reinstall(ReinstallPlanSnapshot),
    Status(InstallStatusSnapshot),
    RecoveryScan(InstallRecoveryScanSnapshot),
    RecoveryPreview(InstallRecoveryPreviewSnapshot),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleApplyResult {
    status: &'static str,
    event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchApplyResult {
    batch_id: String,
    operation: BatchOperation,
    attempt: u32,
    task_id: String,
    status: hmm_core::BatchAttemptStatus,
    summary: BatchResultSummarySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchAttemptResult {
    batch_id: String,
    operation: BatchOperation,
    attempt_number: u32,
    status: hmm_core::BatchAttemptStatus,
    task_id: Option<String>,
    evidence_health_degraded: bool,
    summary: BatchResultSummarySnapshot,
    items: Vec<BatchItemResultSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchResultSummarySnapshot {
    item_count: usize,
    succeeded_count: usize,
    blocked_count: usize,
    failed_count: usize,
    cancelled_count: usize,
    skipped_count: usize,
    recovery_required_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchItemResultSnapshot {
    batch_id: String,
    attempt_number: u32,
    item_id: String,
    ordinal: usize,
    mod_id: String,
    status: hmm_core::BatchItemStatus,
    reason_code: Option<String>,
    retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchActionSummarySnapshot {
    actions: usize,
    retained: usize,
    replaced: usize,
    added: usize,
    stale: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchPreflightDecisionSnapshot {
    status: hmm_core::BatchPreflightStatus,
    rules_version: Option<u32>,
    codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchReasonSummarySnapshot {
    code: String,
    count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchPlanResult {
    plan: BatchPlanSnapshot,
    preview_token: Option<String>,
    expires_at_unix_millis: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchPlanSnapshot {
    plan_schema_version: u32,
    operation: hmm_core::BatchOperation,
    game_id: GameId,
    profile_id: ProfileId,
    execution_policy: BatchExecutionPolicy,
    status: BatchPlanStatus,
    item_count: usize,
    ready_item_count: usize,
    blocked_item_count: usize,
    action_count: usize,
    global_blocking_reasons: Vec<BatchReasonSummarySnapshot>,
    warning_codes: Vec<BatchReasonSummarySnapshot>,
    items: Vec<BatchPlanItemSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchPlanItemSnapshot {
    ordinal: usize,
    mod_id: ModId,
    revision_id: Option<ModRevisionId>,
    status: BatchPlanStatus,
    action_summary: BatchActionSummarySnapshot,
    target_count: usize,
    prerequisite: BatchPreflightDecisionSnapshot,
    blocking_reasons: Vec<String>,
    warning_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliLifecycleOperation {
    Install,
    Uninstall,
    Reinstall,
    Recovery,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum BackupCommandResult {
    List(BackupListSnapshot),
    BackgroundStatus(BackupBackgroundStatusSnapshot),
}

pub fn run_from_env() -> i32 {
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    run(std::env::args_os(), &mut stdout, &mut stderr)
}

pub fn run<I, T, W, E>(args: I, stdout: &mut W, stderr: &mut E) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    W: Write + Send,
    E: Write,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let output_format_hint = machine_output_format_hint(&args);
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = error.exit_code();
            if error.use_stderr() {
                if let Some(format) = output_format_hint {
                    return write_parse_error(format, stdout, stderr);
                }
            }
            let rendered = error.to_string();
            let write_result = if error.use_stderr() {
                stderr.write_all(rendered.as_bytes())
            } else {
                stdout.write_all(rendered.as_bytes())
            };
            return if write_result.is_ok() {
                exit_code
            } else {
                CliExitCode::RuntimeUnavailable.get()
            };
        }
    };

    let Cli {
        format,
        environment,
        data_dir,
        command,
        ..
    } = cli;

    match command {
        Commands::Runtime {
            command: RuntimeCommand::Status,
        } => run_runtime_status(format, environment, data_dir, stdout, stderr),
        Commands::Game { command } => {
            run_game_command(format, environment, data_dir, command, stdout, stderr)
        }
        Commands::Install { command } => {
            run_install_command(format, environment, data_dir, command, stdout, stderr)
        }
        Commands::Backup { command } => {
            run_backup_command(format, environment, data_dir, command, stdout, stderr)
        }
        Commands::Diagnostics { command } => {
            run_diagnostics_command(format, environment, data_dir, command, stdout, stderr)
        }
    }
}

fn machine_output_format_hint(args: &[OsString]) -> Option<OutputFormat> {
    for (index, arg) in args.iter().enumerate() {
        if arg == OsStr::new("--format") {
            return args
                .get(index + 1)
                .and_then(|value| value.to_str())
                .and_then(parse_machine_output_format);
        }

        if let Some(value) = arg
            .to_str()
            .and_then(|value| value.strip_prefix("--format="))
        {
            return parse_machine_output_format(value);
        }
    }
    None
}

fn parse_machine_output_format(value: &str) -> Option<OutputFormat> {
    match value {
        "json" => Some(OutputFormat::Json),
        "jsonl" => Some(OutputFormat::Jsonl),
        _ => None,
    }
}

fn write_parse_error<W: Write, E: Write>(
    format: OutputFormat,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let envelope = CommandEnvelope::<serde_json::Value>::failure(
        CLI_PARSE_COMMAND,
        CliErrorEnvelope::new(CLI_USAGE_ERROR, CliErrorCategory::UserActionRequired, false),
    );
    let write_result = match format {
        OutputFormat::Json | OutputFormat::Jsonl => write_json_line(stdout, &envelope),
        OutputFormat::Human => unreachable!("human output is not a machine format hint"),
    };

    if write_result.is_ok() {
        CliExitCode::Usage.get()
    } else {
        let _ = writeln!(stderr, "cli_output_failed");
        CliExitCode::RuntimeUnavailable.get()
    }
}

fn run_runtime_status<W: Write, E: Write>(
    format: OutputFormat,
    environment_option: EnvironmentOption,
    data_dir: Option<PathBuf>,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let environment = match RuntimeEnvironment::from_options(environment_option.into(), data_dir) {
        Ok(environment) => environment,
        Err(error) => {
            return write_environment_error(format, RUNTIME_STATUS_COMMAND, error, stdout, stderr);
        }
    };

    let result = RuntimeStatusResult {
        environment: environment.kind().as_str(),
        data_root_mode: environment.data_root_mode().as_str(),
        write_command_policy: environment.cli_write_command_policy().as_str(),
        production_writes_allowed: environment.kind() == RuntimeEnvironmentKind::Production,
        business_commands_available: true,
    };

    let write_result = match format {
        OutputFormat::Human => writeln!(stdout, "environment: {}", result.environment)
            .and_then(|_| writeln!(stdout, "data root: {}", result.data_root_mode))
            .and_then(|_| writeln!(stdout, "write commands: {}", result.write_command_policy))
            .and_then(|_| {
                writeln!(
                    stdout,
                    "business commands: read_only_game_install_backup_diagnostics"
                )
            }),
        OutputFormat::Json | OutputFormat::Jsonl => write_json_line(
            stdout,
            &CommandEnvelope::success(RUNTIME_STATUS_COMMAND, result),
        ),
    };

    if write_result.is_ok() {
        CliExitCode::Success.get()
    } else {
        let _ = writeln!(stderr, "cli_output_failed");
        CliExitCode::RuntimeUnavailable.get()
    }
}

fn run_game_command<W: Write, E: Write>(
    format: OutputFormat,
    environment_option: EnvironmentOption,
    data_dir: Option<PathBuf>,
    command: GameCommand,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let command_id = match &command {
        GameCommand::Status(_) => GAME_STATUS_COMMAND,
        GameCommand::Scan(_) => GAME_SCAN_COMMAND,
        GameCommand::Validate(_) => GAME_VALIDATE_COMMAND,
        GameCommand::Prerequisites(_) => GAME_PREREQUISITES_COMMAND,
    };
    let environment = match RuntimeEnvironment::from_options(environment_option.into(), data_dir) {
        Ok(environment) => environment,
        Err(error) => {
            return write_environment_error(format, command_id, error, stdout, stderr);
        }
    };
    let automation = match ReadOnlyGameAutomation::from_environment(&environment) {
        Ok(automation) => automation,
        Err(error) => {
            return write_game_error(format, command_id, error, stdout, stderr);
        }
    };
    let result = match command {
        GameCommand::Status(options) => automation
            .status(&options.game)
            .map(GameCommandResult::Status),
        GameCommand::Scan(options) => automation.scan(&options.game).map(GameCommandResult::Scan),
        GameCommand::Validate(options) => automation
            .validate(&options.game)
            .map(GameCommandResult::Validation),
        GameCommand::Prerequisites(options) => automation
            .prerequisites(&options.game)
            .map(GameCommandResult::Prerequisites),
    };
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            return write_game_error(format, command_id, error, stdout, stderr);
        }
    };

    let write_result = match format {
        OutputFormat::Human => write_human_game_result(stdout, &result),
        OutputFormat::Json | OutputFormat::Jsonl => {
            write_json_line(stdout, &CommandEnvelope::success(command_id, result))
        }
    };

    if write_result.is_ok() {
        CliExitCode::Success.get()
    } else {
        let _ = writeln!(stderr, "cli_output_failed");
        CliExitCode::RuntimeUnavailable.get()
    }
}

fn run_install_command<W: Write + Send, E: Write>(
    format: OutputFormat,
    environment_option: EnvironmentOption,
    data_dir: Option<PathBuf>,
    command: InstallCommand,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let command_id = match &command {
        InstallCommand::Plan(_) => INSTALL_PLAN_COMMAND,
        InstallCommand::Apply(_) => INSTALL_APPLY_COMMAND,
        InstallCommand::Batch {
            command: InstallBatchCommand::Plan(_),
        } => INSTALL_BATCH_PLAN_COMMAND,
        InstallCommand::Batch {
            command: InstallBatchCommand::Apply(_),
        } => INSTALL_BATCH_APPLY_COMMAND,
        InstallCommand::Batch {
            command: InstallBatchCommand::Result(_),
        } => INSTALL_BATCH_RESULT_COMMAND,
        InstallCommand::Batch {
            command: InstallBatchCommand::Retry(_),
        } => INSTALL_BATCH_RETRY_COMMAND,
        InstallCommand::Uninstall(_) => INSTALL_UNINSTALL_COMMAND,
        InstallCommand::Reinstall(_) => INSTALL_REINSTALL_COMMAND,
        InstallCommand::Status(_) => INSTALL_STATUS_COMMAND,
        InstallCommand::Recovery {
            command: InstallRecoveryCommand::Scan(_),
        } => INSTALL_RECOVERY_SCAN_COMMAND,
        InstallCommand::Recovery {
            command: InstallRecoveryCommand::Preview(_),
        } => INSTALL_RECOVERY_PREVIEW_COMMAND,
        InstallCommand::Recovery {
            command: InstallRecoveryCommand::Apply(_),
        } => INSTALL_RECOVERY_APPLY_COMMAND,
    };
    let environment = match RuntimeEnvironment::from_options(environment_option.into(), data_dir) {
        Ok(environment) => environment,
        Err(error) => {
            return write_environment_error(format, command_id, error, stdout, stderr);
        }
    };
    let command = match command {
        InstallCommand::Apply(options) => {
            return run_install_apply(format, &environment, options, stdout, stderr);
        }
        InstallCommand::Batch { command } => {
            return run_install_batch_command(format, &environment, command, stdout, stderr);
        }
        InstallCommand::Uninstall(options) => {
            return run_install_uninstall(format, &environment, options, stdout, stderr);
        }
        InstallCommand::Reinstall(options) => {
            return run_install_reinstall(format, &environment, options, stdout, stderr);
        }
        InstallCommand::Recovery {
            command: InstallRecoveryCommand::Apply(options),
        } => {
            return run_install_recovery_apply(format, &environment, options, stdout, stderr);
        }
        command => command,
    };
    let automation = match ReadOnlyInstallAutomation::from_environment(&environment) {
        Ok(automation) => automation,
        Err(error) => {
            return write_install_error(format, command_id, error, stdout, stderr);
        }
    };
    let result = match command {
        InstallCommand::Plan(options) => automation
            .plan_for_profile(&options.game, &options.profile, &options.mod_id)
            .map(InstallCommandResult::Plan),
        InstallCommand::Batch { .. } => {
            unreachable!("install batch is handled before read-only paths")
        }
        InstallCommand::Apply(_) => unreachable!("install apply is handled before read-only paths"),
        InstallCommand::Uninstall(_) => {
            unreachable!("install uninstall is handled before read-only paths")
        }
        InstallCommand::Reinstall(_) => {
            unreachable!("install reinstall is handled before read-only paths")
        }
        InstallCommand::Status(options) => automation
            .status(options.game.as_deref(), &options.profile, &options.mod_ids)
            .map(InstallCommandResult::Status),
        InstallCommand::Recovery {
            command: InstallRecoveryCommand::Scan(options),
        } => automation
            .recovery_scan(&options.game, &options.profile, &options.mod_ids)
            .map(InstallCommandResult::RecoveryScan),
        InstallCommand::Recovery {
            command: InstallRecoveryCommand::Preview(options),
        } => automation
            .recovery_preview(
                &options.game,
                &options.profile,
                &options.mod_id,
                options.action.into(),
            )
            .map(InstallCommandResult::RecoveryPreview),
        InstallCommand::Recovery {
            command: InstallRecoveryCommand::Apply(_),
        } => unreachable!("install recovery apply is handled before read-only paths"),
    };
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            return write_install_error(format, command_id, error, stdout, stderr);
        }
    };

    write_install_result(format, command_id, result, stdout, stderr)
}

fn run_install_batch_command<W: Write + Send, E: Write>(
    format: OutputFormat,
    environment: &RuntimeEnvironment,
    command: InstallBatchCommand,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    match command {
        InstallBatchCommand::Plan(options) => {
            let request = match batch_plan_request(&options.request) {
                Ok(request) => request,
                Err(code) => {
                    return write_batch_error(
                        format,
                        INSTALL_BATCH_PLAN_COMMAND,
                        code,
                        stdout,
                        stderr,
                    );
                }
            };
            match BatchLifecycleAutomation::preview_request(environment, request) {
                Ok(preview) => write_install_result(
                    format,
                    INSTALL_BATCH_PLAN_COMMAND,
                    InstallCommandResult::BatchPlan(BatchPlanResult {
                        plan: project_batch_plan(&preview.plan),
                        preview_token: preview.preview_token,
                        expires_at_unix_millis: preview.expires_at_unix_millis,
                    }),
                    stdout,
                    stderr,
                ),
                Err(error) => write_batch_automation_error(
                    format,
                    INSTALL_BATCH_PLAN_COMMAND,
                    error,
                    stdout,
                    stderr,
                ),
            }
        }
        InstallBatchCommand::Apply(options) => {
            if !options.commit.commit || !options.commit.yes {
                return write_batch_error(
                    format,
                    INSTALL_BATCH_APPLY_COMMAND,
                    "batch_commit_required",
                    stdout,
                    stderr,
                );
            }
            let Some(preview_token) = options.preview_token.as_deref() else {
                return write_batch_error(
                    format,
                    INSTALL_BATCH_APPLY_COMMAND,
                    "batch_preview_token_required",
                    stdout,
                    stderr,
                );
            };
            let request = match batch_plan_request(&options.request) {
                Ok(request) => request,
                Err(code) => {
                    return write_batch_error(
                        format,
                        INSTALL_BATCH_APPLY_COMMAND,
                        code,
                        stdout,
                        stderr,
                    );
                }
            };
            match BatchLifecycleAutomation::apply_request(environment, request, preview_token)
            {
                Ok((operation, _sealed, run)) => {
                    let result = project_batch_run(
                        &run.batch_id,
                        operation,
                        run.attempt_number,
                        run.task_id,
                        run.status,
                        &run.summary,
                    );
                    write_batch_run_result(
                        format,
                        INSTALL_BATCH_APPLY_COMMAND,
                        result,
                        stdout,
                        stderr,
                    )
                }
                Err(error) => write_batch_automation_error(
                    format,
                    INSTALL_BATCH_APPLY_COMMAND,
                    error,
                    stdout,
                    stderr,
                ),
            }
        }
        InstallBatchCommand::Result(options) => {
            match BatchLifecycleAutomation::result(
                environment,
                &options.batch_id,
                options.attempt,
            ) {
                Ok(snapshot) => write_batch_attempt_result(
                    format,
                    INSTALL_BATCH_RESULT_COMMAND,
                    project_batch_attempt(snapshot),
                    stdout,
                    stderr,
                ),
                Err(error) => write_batch_automation_error(
                    format,
                    INSTALL_BATCH_RESULT_COMMAND,
                    error,
                    stdout,
                    stderr,
                ),
            }
        }
        InstallBatchCommand::Retry(options) => {
            if !options.commit.commit || !options.commit.yes {
                return write_batch_error(
                    format,
                    INSTALL_BATCH_RETRY_COMMAND,
                    "batch_commit_required",
                    stdout,
                    stderr,
                );
            }
            match BatchLifecycleAutomation::retry_with_operation(
                environment,
                &options.batch_id,
                options.attempt,
            ) {
                Ok((operation, _retry, run)) => {
                    let result = project_batch_run(
                        &run.batch_id,
                        operation,
                        run.attempt_number,
                        run.task_id,
                        run.status,
                        &run.summary,
                    );
                    write_batch_run_result(
                        format,
                        INSTALL_BATCH_RETRY_COMMAND,
                        result,
                        stdout,
                        stderr,
                    )
                }
                Err(error) => write_batch_automation_error(
                    format,
                    INSTALL_BATCH_RETRY_COMMAND,
                    error,
                    stdout,
                    stderr,
                ),
            }
        }
    }
}

fn project_batch_plan(plan: &hmm_core::BatchPlan) -> BatchPlanSnapshot {
    let ready_item_count = plan.items.iter().filter(|item| item.is_ready()).count();
    let blocked_item_count = plan.items.len().saturating_sub(ready_item_count);
    let action_count = plan
        .items
        .iter()
        .map(|item| item.action_summary.actions)
        .sum();
    let items = plan
        .items
        .iter()
        .map(|item| {
            let revision_id = match &item.input_snapshot {
                BatchItemInput::Install(input) => Some(input.revision_id.clone()),
                BatchItemInput::Uninstall(input) => {
                    Some(input.expected_installed_revision_id.clone())
                }
                BatchItemInput::Reinstall(input) => Some(input.candidate_revision_id.clone()),
            };
            BatchPlanItemSnapshot {
                ordinal: item.ordinal,
                mod_id: item.input_snapshot.mod_id().clone(),
                revision_id,
                status: if item.is_ready() {
                    BatchPlanStatus::Ready
                } else {
                    BatchPlanStatus::Blocked
                },
                action_summary: project_batch_action_summary(&item.action_summary),
                target_count: item.target_claims.len(),
                prerequisite: project_batch_prerequisite(&item.prerequisite),
                blocking_reasons: item.blocking_reasons.clone(),
                warning_codes: item.warning_codes.clone(),
            }
        })
        .collect();

    BatchPlanSnapshot {
        plan_schema_version: plan.plan_schema_version,
        operation: plan.operation,
        game_id: plan.game_id.clone(),
        profile_id: plan.profile_id.clone(),
        execution_policy: plan.execution_policy,
        status: plan.status(),
        item_count: plan.items.len(),
        ready_item_count,
        blocked_item_count,
        action_count,
        global_blocking_reasons: plan
            .global_blocking_reasons
            .iter()
            .map(project_batch_reason)
            .collect(),
        warning_codes: plan
            .warning_codes
            .iter()
            .map(project_batch_reason)
            .collect(),
        items,
    }
}

fn project_batch_attempt(snapshot: RuntimeBatchAttemptSnapshot) -> BatchAttemptResult {
    BatchAttemptResult {
        batch_id: snapshot.batch_id,
        operation: snapshot.operation,
        attempt_number: snapshot.attempt_number,
        status: snapshot.status,
        task_id: snapshot.task_id,
        evidence_health_degraded: snapshot.evidence_health_degraded,
        summary: project_batch_summary(&snapshot.summary),
        items: snapshot
            .items
            .iter()
            .map(project_batch_item_result)
            .collect(),
    }
}

fn project_batch_run(
    batch_id: &hmm_core::BatchId,
    operation: BatchOperation,
    attempt: u32,
    task_id: String,
    status: hmm_core::BatchAttemptStatus,
    summary: &hmm_core::BatchResultSummary,
) -> BatchApplyResult {
    BatchApplyResult {
        batch_id: batch_id.as_str().to_owned(),
        operation,
        attempt,
        task_id,
        status,
        summary: project_batch_summary(summary),
    }
}

fn project_batch_summary(summary: &hmm_core::BatchResultSummary) -> BatchResultSummarySnapshot {
    BatchResultSummarySnapshot {
        item_count: summary.item_count,
        succeeded_count: summary.succeeded_count,
        blocked_count: summary.blocked_count,
        failed_count: summary.failed_count,
        cancelled_count: summary.cancelled_count,
        skipped_count: summary.skipped_count,
        recovery_required_count: summary.recovery_required_count,
    }
}

fn project_batch_item_result(result: &hmm_core::BatchItemResult) -> BatchItemResultSnapshot {
    BatchItemResultSnapshot {
        batch_id: result.batch_id.as_str().to_owned(),
        attempt_number: result.attempt_number,
        item_id: result.item_id.as_str().to_owned(),
        ordinal: result.ordinal,
        mod_id: result.mod_id.as_str().to_owned(),
        status: result.status,
        reason_code: result.reason_code.clone(),
        retryable: result.retryable,
    }
}

fn project_batch_action_summary(
    summary: &hmm_core::BatchActionSummary,
) -> BatchActionSummarySnapshot {
    BatchActionSummarySnapshot {
        actions: summary.actions,
        retained: summary.retained,
        replaced: summary.replaced,
        added: summary.added,
        stale: summary.stale,
    }
}

fn project_batch_prerequisite(
    decision: &hmm_core::BatchPreflightDecision,
) -> BatchPreflightDecisionSnapshot {
    BatchPreflightDecisionSnapshot {
        status: decision.status,
        rules_version: decision.rules_version,
        codes: decision.codes.clone(),
    }
}

fn project_batch_reason(reason: &hmm_core::BatchReasonSummary) -> BatchReasonSummarySnapshot {
    BatchReasonSummarySnapshot {
        code: reason.code.clone(),
        count: reason.count,
    }
}

fn batch_plan_request(
    options: &InstallBatchRequestOptions,
) -> Result<BatchLifecyclePlanRequest, &'static str> {
    let game_id = GameId::parse(&options.game).map_err(|_| "batch_game_invalid")?;
    let profile_id =
        parse_batch_id_component(&options.profile, "batch_profile_invalid").map(ProfileId::new)?;
    if options.items.is_empty() {
        return Err("batch_items_required");
    }
    let operation = BatchOperation::from(options.operation);
    let mut replacement_targets = BTreeMap::new();
    for selection in &options.replacement_targets {
        let (mod_id, target_id) = selection.split_once('=').ok_or("batch_item_invalid")?;
        let mod_id = parse_batch_id_component(mod_id, "batch_item_invalid").map(ModId::new)?;
        let target_id = parse_batch_target_id(target_id)?;
        if replacement_targets.insert(mod_id, target_id).is_some() {
            return Err("batch_item_invalid");
        }
    }
    if operation != BatchOperation::Reinstall && !replacement_targets.is_empty() {
        return Err("batch_item_invalid");
    }
    let items = options
        .items
        .iter()
        .map(|item| {
            let fields = item.split(':').collect::<Vec<_>>();
            match (operation, fields.as_slice()) {
                (BatchOperation::Install, [mod_id, revision_id]) => {
                    let mod_id =
                        parse_batch_id_component(mod_id, "batch_item_invalid").map(ModId::new)?;
                    let revision_id = parse_batch_id_component(revision_id, "batch_item_invalid")
                        .map(ModRevisionId::new)?;
                    Ok(BatchItemInput::Install(hmm_core::InstallBatchItemInput {
                        mod_id,
                        revision_id,
                        // Batch CLI fixtures intentionally expose only the fixed Sandbox base layer.
                        layer: FileLayer::new("base", 0),
                        replacement_binding_snapshot: None,
                    }))
                }
                (BatchOperation::Uninstall, [mod_id, revision_id]) => {
                    let mod_id =
                        parse_batch_id_component(mod_id, "batch_item_invalid").map(ModId::new)?;
                    let expected_installed_revision_id =
                        parse_batch_id_component(revision_id, "batch_item_invalid")
                            .map(ModRevisionId::new)?;
                    Ok(BatchItemInput::Uninstall(UninstallBatchItemInput {
                        mod_id,
                        expected_installed_revision_id,
                    }))
                }
                (
                    BatchOperation::Reinstall,
                    [mod_id, installed_revision_id, candidate_revision_id],
                ) => {
                    let mod_id =
                        parse_batch_id_component(mod_id, "batch_item_invalid").map(ModId::new)?;
                    let installed_revision_id =
                        parse_batch_id_component(installed_revision_id, "batch_item_invalid")
                            .map(ModRevisionId::new)?;
                    let candidate_revision_id =
                        parse_batch_id_component(candidate_revision_id, "batch_item_invalid")
                            .map(ModRevisionId::new)?;
                    Ok(BatchItemInput::Reinstall(ReinstallBatchItemInput {
                        mod_id,
                        installed_revision_id,
                        candidate_revision_id,
                        layer: FileLayer::new("base", 0),
                        replacement_binding_snapshot: None,
                    }))
                }
                _ => Err("batch_item_invalid"),
            }
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    if operation == BatchOperation::Reinstall {
        let mut item_mod_ids = BTreeMap::new();
        for item in &items {
            let BatchItemInput::Reinstall(input) = item else {
                return Err("batch_item_invalid");
            };
            let has_target = replacement_targets.contains_key(&input.mod_id);
            let same_revision = input.installed_revision_id == input.candidate_revision_id;
            if has_target != same_revision {
                return Err("batch_item_invalid");
            }
            item_mod_ids.insert(input.mod_id.clone(), ());
        }
        if replacement_targets
            .keys()
            .any(|mod_id| !item_mod_ids.contains_key(mod_id))
        {
            return Err("batch_item_invalid");
        }
    }
    Ok(BatchLifecyclePlanRequest {
        plan: BatchPlanRequest {
            schema_version: BATCH_PLAN_SCHEMA_VERSION,
            operation,
            game_id,
            profile_id,
            execution_policy: options.policy.into(),
            items,
        },
        replacement_targets,
    })
}

fn parse_batch_target_id(value: &str) -> Result<ReplacementTargetId, &'static str> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 256
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, ':' | '-' | '_' | '.')
        })
    {
        return Err("batch_item_invalid");
    }
    ReplacementTargetId::parse(value).map_err(|_| "batch_item_invalid")
}

fn parse_batch_id_component(
    value: &str,
    invalid_code: &'static str,
) -> Result<String, &'static str> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(invalid_code);
    }
    Ok(value.to_owned())
}

fn batch_exit_code(status: hmm_core::BatchAttemptStatus) -> i32 {
    match status {
        hmm_core::BatchAttemptStatus::Completed => CliExitCode::Success.get(),
        hmm_core::BatchAttemptStatus::CompletedWithErrors => CliExitCode::PartialSuccess.get(),
        hmm_core::BatchAttemptStatus::Cancelled => CliExitCode::Cancelled.get(),
        hmm_core::BatchAttemptStatus::Blocked
        | hmm_core::BatchAttemptStatus::RecoveryRequired
        | hmm_core::BatchAttemptStatus::Interrupted
        | hmm_core::BatchAttemptStatus::Failed
        | hmm_core::BatchAttemptStatus::Sealed
        | hmm_core::BatchAttemptStatus::Queued
        | hmm_core::BatchAttemptStatus::Running
        | hmm_core::BatchAttemptStatus::Stopping => CliExitCode::ControlledFailure.get(),
    }
}

fn batch_result_exit_code(status: hmm_core::BatchAttemptStatus) -> i32 {
    match status {
        hmm_core::BatchAttemptStatus::Sealed
        | hmm_core::BatchAttemptStatus::Queued
        | hmm_core::BatchAttemptStatus::Running
        | hmm_core::BatchAttemptStatus::Stopping => CliExitCode::Success.get(),
        terminal => batch_exit_code(terminal),
    }
}

fn write_batch_attempt_result<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    result: BatchAttemptResult,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let status = result.status;
    let write_exit_code = write_install_result(
        format,
        command,
        InstallCommandResult::BatchAttempt(result),
        stdout,
        stderr,
    );
    if write_exit_code == CliExitCode::Success.get() {
        batch_result_exit_code(status)
    } else {
        write_exit_code
    }
}

fn run_install_apply<W: Write + Send, E: Write>(
    format: OutputFormat,
    environment: &RuntimeEnvironment,
    options: InstallApplyOptions,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    if !options.lifecycle.commit || !options.lifecycle.yes {
        let automation = match ReadOnlyInstallAutomation::from_environment(environment) {
            Ok(automation) => automation,
            Err(error) => {
                return write_install_error(format, INSTALL_APPLY_COMMAND, error, stdout, stderr);
            }
        };
        let result =
            match automation.plan_for_profile(&options.game, &options.profile, &options.mod_id) {
                Ok(result) => InstallCommandResult::Plan(result),
                Err(error) => {
                    return write_install_error(
                        format,
                        INSTALL_APPLY_COMMAND,
                        error,
                        stdout,
                        stderr,
                    );
                }
            };
        return write_install_result(format, INSTALL_APPLY_COMMAND, result, stdout, stderr);
    }

    let Some(plan_token) = options.lifecycle.plan_token.as_deref() else {
        return write_command_error(
            format,
            INSTALL_APPLY_COMMAND,
            CliErrorEnvelope::new(
                "plan_token_required",
                CliErrorCategory::UserActionRequired,
                false,
            ),
            CliExitCode::Usage,
            stdout,
            stderr,
        );
    };
    let cancellation = match install_cli_cancellation(format, INSTALL_APPLY_COMMAND, stdout, stderr)
    {
        Ok(cancellation) => cancellation,
        Err(exit_code) => return exit_code,
    };
    let automation = match CliLifecycleAutomation::prepare_install(
        environment,
        &options.game,
        &options.profile,
        &options.mod_id,
        plan_token,
    ) {
        Ok(automation) => automation,
        Err(error) => {
            return write_lifecycle_error(format, INSTALL_APPLY_COMMAND, error, stdout, stderr);
        }
    };

    run_lifecycle_operation(
        format,
        INSTALL_APPLY_COMMAND,
        &automation,
        CliLifecycleOperation::Install,
        cancellation,
        stdout,
        stderr,
    )
}

fn run_install_uninstall<W: Write + Send, E: Write>(
    format: OutputFormat,
    environment: &RuntimeEnvironment,
    options: InstallUninstallOptions,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    if !options.lifecycle.commit || !options.lifecycle.yes {
        let automation = match ReadOnlyInstallAutomation::from_environment(environment) {
            Ok(automation) => automation,
            Err(error) => {
                return write_install_error(
                    format,
                    INSTALL_UNINSTALL_COMMAND,
                    error,
                    stdout,
                    stderr,
                );
            }
        };
        let result =
            match automation.uninstall_preview(&options.game, &options.profile, &options.mod_id) {
                Ok(result) => InstallCommandResult::Uninstall(result),
                Err(error) => {
                    return write_install_error(
                        format,
                        INSTALL_UNINSTALL_COMMAND,
                        error,
                        stdout,
                        stderr,
                    );
                }
            };
        return write_install_result(format, INSTALL_UNINSTALL_COMMAND, result, stdout, stderr);
    }

    let Some(plan_token) = options.lifecycle.plan_token.as_deref() else {
        return write_command_error(
            format,
            INSTALL_UNINSTALL_COMMAND,
            CliErrorEnvelope::new(
                "plan_token_required",
                CliErrorCategory::UserActionRequired,
                false,
            ),
            CliExitCode::Usage,
            stdout,
            stderr,
        );
    };
    let cancellation =
        match install_cli_cancellation(format, INSTALL_UNINSTALL_COMMAND, stdout, stderr) {
            Ok(cancellation) => cancellation,
            Err(exit_code) => return exit_code,
        };
    let automation = match CliLifecycleAutomation::prepare_uninstall(
        environment,
        &options.game,
        &options.profile,
        &options.mod_id,
        plan_token,
    ) {
        Ok(automation) => automation,
        Err(error) => {
            return write_lifecycle_error(format, INSTALL_UNINSTALL_COMMAND, error, stdout, stderr);
        }
    };

    run_lifecycle_operation(
        format,
        INSTALL_UNINSTALL_COMMAND,
        &automation,
        CliLifecycleOperation::Uninstall,
        cancellation,
        stdout,
        stderr,
    )
}

fn run_install_recovery_apply<W: Write + Send, E: Write>(
    format: OutputFormat,
    environment: &RuntimeEnvironment,
    options: InstallRecoveryApplyOptions,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    if !options.lifecycle.commit || !options.lifecycle.yes {
        let automation = match ReadOnlyInstallAutomation::from_environment(environment) {
            Ok(automation) => automation,
            Err(error) => {
                return write_install_error(
                    format,
                    INSTALL_RECOVERY_APPLY_COMMAND,
                    error,
                    stdout,
                    stderr,
                );
            }
        };
        let result = match automation.recovery_preview(
            &options.game,
            &options.profile,
            &options.mod_id,
            options.action.into(),
        ) {
            Ok(result) => InstallCommandResult::RecoveryPreview(result),
            Err(error) => {
                return write_install_error(
                    format,
                    INSTALL_RECOVERY_APPLY_COMMAND,
                    error,
                    stdout,
                    stderr,
                );
            }
        };
        return write_install_result(
            format,
            INSTALL_RECOVERY_APPLY_COMMAND,
            result,
            stdout,
            stderr,
        );
    }

    let Some(plan_token) = options.lifecycle.plan_token.as_deref() else {
        return write_command_error(
            format,
            INSTALL_RECOVERY_APPLY_COMMAND,
            CliErrorEnvelope::new(
                "plan_token_required",
                CliErrorCategory::UserActionRequired,
                false,
            ),
            CliExitCode::Usage,
            stdout,
            stderr,
        );
    };
    let cancellation =
        match install_cli_cancellation(format, INSTALL_RECOVERY_APPLY_COMMAND, stdout, stderr) {
            Ok(cancellation) => cancellation,
            Err(exit_code) => return exit_code,
        };
    let automation = match CliLifecycleAutomation::prepare_recovery(
        environment,
        &options.game,
        &options.profile,
        &options.mod_id,
        options.action.into(),
        plan_token,
    ) {
        Ok(automation) => automation,
        Err(error) => {
            return write_lifecycle_error(
                format,
                INSTALL_RECOVERY_APPLY_COMMAND,
                error,
                stdout,
                stderr,
            );
        }
    };

    run_lifecycle_operation(
        format,
        INSTALL_RECOVERY_APPLY_COMMAND,
        &automation,
        CliLifecycleOperation::Recovery,
        cancellation,
        stdout,
        stderr,
    )
}

fn run_install_reinstall<W: Write + Send, E: Write>(
    format: OutputFormat,
    environment: &RuntimeEnvironment,
    options: InstallReinstallOptions,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    if !options.lifecycle.commit || !options.lifecycle.yes {
        let automation = match ReadOnlyInstallAutomation::from_environment(environment) {
            Ok(automation) => automation,
            Err(error) => {
                return write_install_error(
                    format,
                    INSTALL_REINSTALL_COMMAND,
                    error,
                    stdout,
                    stderr,
                );
            }
        };
        let result = match automation.reinstall_preview(
            &options.game,
            &options.profile,
            &options.mod_id,
            &options.candidate_revision_id,
        ) {
            Ok(result) => InstallCommandResult::Reinstall(result),
            Err(error) => {
                return write_install_error(
                    format,
                    INSTALL_REINSTALL_COMMAND,
                    error,
                    stdout,
                    stderr,
                );
            }
        };
        return write_install_result(format, INSTALL_REINSTALL_COMMAND, result, stdout, stderr);
    }

    let Some(plan_token) = options.lifecycle.plan_token.as_deref() else {
        return write_command_error(
            format,
            INSTALL_REINSTALL_COMMAND,
            CliErrorEnvelope::new(
                "plan_token_required",
                CliErrorCategory::UserActionRequired,
                false,
            ),
            CliExitCode::Usage,
            stdout,
            stderr,
        );
    };
    let cancellation =
        match install_cli_cancellation(format, INSTALL_REINSTALL_COMMAND, stdout, stderr) {
            Ok(cancellation) => cancellation,
            Err(exit_code) => return exit_code,
        };
    let automation = match CliLifecycleAutomation::prepare_reinstall(
        environment,
        &options.game,
        &options.profile,
        &options.mod_id,
        &options.candidate_revision_id,
        plan_token,
    ) {
        Ok(automation) => automation,
        Err(error) => {
            return write_lifecycle_error(format, INSTALL_REINSTALL_COMMAND, error, stdout, stderr);
        }
    };

    run_lifecycle_operation(
        format,
        INSTALL_REINSTALL_COMMAND,
        &automation,
        CliLifecycleOperation::Reinstall,
        cancellation,
        stdout,
        stderr,
    )
}

fn install_cli_cancellation<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<Arc<CliCancellationCoordinator>, i32> {
    CliCancellationCoordinator::install().map_err(|_| {
        write_command_error(
            format,
            command,
            CliErrorEnvelope::new(
                "cli_cancellation_unavailable",
                CliErrorCategory::Recoverable,
                true,
            ),
            CliExitCode::RuntimeUnavailable,
            stdout,
            stderr,
        )
    })
}

fn run_lifecycle_operation<W: Write + Send, E: Write>(
    format: OutputFormat,
    command: &'static str,
    automation: &CliLifecycleAutomation,
    operation: CliLifecycleOperation,
    cancellation: Arc<CliCancellationCoordinator>,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    cancellation.bind(automation.cancellation_handle());

    if format == OutputFormat::Jsonl {
        let output = Arc::new(Mutex::new(&mut *stdout));
        let observer = CliTaskProgressObserver::new(
            command,
            Arc::clone(&output),
            automation.task_log_writer(),
        );
        let cancellation_observer = cancellation.observing(&observer);
        let result = run_lifecycle_with_observer(automation, operation, &cancellation_observer);
        let cancelled = cancellation.cancelled_event();
        if let Some(event) = &cancelled {
            let _ = cancellation_observer.observe(event);
        }
        drop(cancellation_observer);
        let observer_error = observer.first_error();
        drop(observer);
        drop(output);
        if observer_error.is_some() {
            let _ = writeln!(stderr, "cli_task_observer_failed");
            return CliExitCode::RuntimeUnavailable.get();
        }
        if cancelled.is_some() {
            return CliExitCode::Cancelled.get();
        }
        return match result {
            Ok(_) => CliExitCode::Success.get(),
            Err(CliLifecycleAutomationError::TaskFailed { .. }) => {
                CliExitCode::ControlledFailure.get()
            }
            Err(error) => write_lifecycle_error(format, command, error, stdout, stderr),
        };
    }

    let observer = NoopCliTaskProgressObserver;
    let cancellation_observer = cancellation.observing(&observer);
    let outcome = run_lifecycle_with_observer(automation, operation, &cancellation_observer);
    if let Some(cancelled) = cancellation.cancelled_event() {
        return write_cancelled_lifecycle_result(format, command, &cancelled, stdout, stderr);
    }
    match outcome {
        Ok(outcome) => {
            let result = LifecycleApplyResult {
                status: "completed",
                event_count: outcome.events.len(),
            };
            let write_result = match format {
                OutputFormat::Human => writeln!(stdout, "task: {}", outcome.task_id)
                    .and_then(|_| writeln!(stdout, "status: {}", result.status)),
                OutputFormat::Json => {
                    let mut envelope = CommandEnvelope::success(command, result);
                    envelope.task_id = Some(outcome.task_id);
                    write_json_line(stdout, &envelope)
                }
                OutputFormat::Jsonl => unreachable!("JSONL is handled above"),
            };
            if write_result.is_ok() {
                CliExitCode::Success.get()
            } else {
                let _ = writeln!(stderr, "cli_output_failed");
                CliExitCode::RuntimeUnavailable.get()
            }
        }
        Err(error) => write_lifecycle_error(format, command, error, stdout, stderr),
    }
}

fn run_lifecycle_with_observer<O: TaskProgressObserver + ?Sized>(
    automation: &CliLifecycleAutomation,
    operation: CliLifecycleOperation,
    observer: &O,
) -> Result<LifecycleTaskOutcome, CliLifecycleAutomationError> {
    match operation {
        CliLifecycleOperation::Install => automation.run_install_with_observer(observer),
        CliLifecycleOperation::Uninstall => automation.run_uninstall_with_observer(observer),
        CliLifecycleOperation::Reinstall => automation.run_reinstall_with_observer(observer),
        CliLifecycleOperation::Recovery => automation.run_recovery_with_observer(observer),
    }
}

fn write_cancelled_lifecycle_result<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    event: &TaskProgressEvent,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let write_result = match format {
        OutputFormat::Human => writeln!(stderr, "task_cancelled"),
        OutputFormat::Json => {
            let mut envelope = CommandEnvelope::<serde_json::Value>::failure(
                command,
                CliErrorEnvelope::new(
                    "task_cancelled",
                    CliErrorCategory::UserActionRequired,
                    false,
                ),
            );
            envelope.task_id = Some(event.task_id.clone());
            write_json_line(stdout, &envelope)
        }
        OutputFormat::Jsonl => unreachable!("JSONL cancellation is emitted as a terminal event"),
    };
    if write_result.is_ok() {
        CliExitCode::Cancelled.get()
    } else {
        let _ = writeln!(stderr, "cli_output_failed");
        CliExitCode::RuntimeUnavailable.get()
    }
}

fn write_install_result<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    result: InstallCommandResult,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let write_result = match format {
        OutputFormat::Human => write_human_install_result(stdout, &result),
        OutputFormat::Json | OutputFormat::Jsonl => {
            write_json_line(stdout, &CommandEnvelope::success(command, &result))
        }
    };
    if write_result.is_ok() {
        CliExitCode::Success.get()
    } else {
        let _ = writeln!(stderr, "cli_output_failed");
        CliExitCode::RuntimeUnavailable.get()
    }
}

fn run_backup_command<W: Write, E: Write>(
    format: OutputFormat,
    environment_option: EnvironmentOption,
    data_dir: Option<PathBuf>,
    command: BackupCommand,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let command_id = match &command {
        BackupCommand::List(_) => BACKUP_LIST_COMMAND,
        BackupCommand::Background {
            command: BackupBackgroundCommand::Status(_),
        } => BACKUP_BACKGROUND_STATUS_COMMAND,
    };
    let environment = match RuntimeEnvironment::from_options(environment_option.into(), data_dir) {
        Ok(environment) => environment,
        Err(error) => {
            return write_environment_error(format, command_id, error, stdout, stderr);
        }
    };
    let automation = match ReadOnlyBackupAutomation::from_environment(&environment) {
        Ok(automation) => automation,
        Err(error) => {
            return write_backup_error(format, command_id, error, stdout, stderr);
        }
    };
    let result = match command {
        BackupCommand::List(options) => automation
            .list(&options.game, &options.profile, options.limit)
            .map(BackupCommandResult::List),
        BackupCommand::Background {
            command: BackupBackgroundCommand::Status(options),
        } => automation
            .background_status(&options.game, &options.profile)
            .map(BackupCommandResult::BackgroundStatus),
    };
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            return write_backup_error(format, command_id, error, stdout, stderr);
        }
    };

    let write_result = match format {
        OutputFormat::Human => write_human_backup_result(stdout, &result),
        OutputFormat::Json | OutputFormat::Jsonl => {
            write_json_line(stdout, &CommandEnvelope::success(command_id, result))
        }
    };
    if write_result.is_ok() {
        CliExitCode::Success.get()
    } else {
        let _ = writeln!(stderr, "cli_output_failed");
        CliExitCode::RuntimeUnavailable.get()
    }
}

fn run_diagnostics_command<W: Write, E: Write>(
    format: OutputFormat,
    environment_option: EnvironmentOption,
    data_dir: Option<PathBuf>,
    command: DiagnosticsCommand,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let command_id = match command {
        DiagnosticsCommand::Snapshot => DIAGNOSTICS_SNAPSHOT_COMMAND,
    };
    let environment = match RuntimeEnvironment::from_options(environment_option.into(), data_dir) {
        Ok(environment) => environment,
        Err(error) => {
            return write_environment_error(format, command_id, error, stdout, stderr);
        }
    };
    let automation = match ReadOnlyDiagnosticsAutomation::from_environment(&environment) {
        Ok(automation) => automation,
        Err(error) => {
            return write_diagnostics_error(format, command_id, error, stdout, stderr);
        }
    };
    let result = automation.snapshot();

    let write_result = match format {
        OutputFormat::Human => write_human_diagnostics_result(stdout, &result),
        OutputFormat::Json | OutputFormat::Jsonl => {
            write_json_line(stdout, &CommandEnvelope::success(command_id, result))
        }
    };
    if write_result.is_ok() {
        CliExitCode::Success.get()
    } else {
        let _ = writeln!(stderr, "cli_output_failed");
        CliExitCode::RuntimeUnavailable.get()
    }
}

fn write_environment_error<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    error: RuntimeEnvironmentError,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    write_command_error(
        format,
        command,
        CliErrorEnvelope::new(error.code(), CliErrorCategory::UserActionRequired, false),
        CliExitCode::Usage,
        stdout,
        stderr,
    )
}

fn write_game_error<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    error: ReadOnlyGameAutomationError,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let (category, exit_code) = match error {
        ReadOnlyGameAutomationError::UnsupportedGame => {
            (CliErrorCategory::UserActionRequired, CliExitCode::Usage)
        }
        ReadOnlyGameAutomationError::ConfiguredGamePathRejected
        | ReadOnlyGameAutomationError::SandboxGamePathRejected => {
            (CliErrorCategory::DataSafetyRisk, CliExitCode::Rejected)
        }
        ReadOnlyGameAutomationError::AppDataUnavailable
        | ReadOnlyGameAutomationError::StorageCorrupted
        | ReadOnlyGameAutomationError::StorageUnavailable
        | ReadOnlyGameAutomationError::ScanUnavailable => (
            CliErrorCategory::Recoverable,
            CliExitCode::RuntimeUnavailable,
        ),
        ReadOnlyGameAutomationError::InternalUnavailable => (
            CliErrorCategory::InternalBug,
            CliExitCode::RuntimeUnavailable,
        ),
    };

    write_command_error(
        format,
        command,
        CliErrorEnvelope::new(error.code(), category, false),
        exit_code,
        stdout,
        stderr,
    )
}

fn write_install_error<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    error: ReadOnlyInstallAutomationError,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let (category, exit_code, retryable) = match error {
        ReadOnlyInstallAutomationError::UnsupportedGame
        | ReadOnlyInstallAutomationError::ProfileIdInvalid
        | ReadOnlyInstallAutomationError::ModIdInvalid
        | ReadOnlyInstallAutomationError::SourceRevisionIdInvalid
        | ReadOnlyInstallAutomationError::CandidateRevisionIdInvalid => (
            CliErrorCategory::UserActionRequired,
            CliExitCode::Usage,
            false,
        ),
        ReadOnlyInstallAutomationError::SandboxStoragePathRejected
        | ReadOnlyInstallAutomationError::ConfiguredGamePathRejected
        | ReadOnlyInstallAutomationError::SandboxGamePathRejected
        | ReadOnlyInstallAutomationError::InstallPlanInvalid
        | ReadOnlyInstallAutomationError::InstallStateInvalid => (
            CliErrorCategory::DataSafetyRisk,
            CliExitCode::Rejected,
            false,
        ),
        ReadOnlyInstallAutomationError::GameInstanceUnavailable
        | ReadOnlyInstallAutomationError::ImportedModNotFound => (
            CliErrorCategory::UserActionRequired,
            CliExitCode::Rejected,
            false,
        ),
        ReadOnlyInstallAutomationError::AppDataUnavailable
        | ReadOnlyInstallAutomationError::GameConfigCorrupted
        | ReadOnlyInstallAutomationError::GameConfigUnavailable
        | ReadOnlyInstallAutomationError::ImportedModCatalogUnavailable
        | ReadOnlyInstallAutomationError::ImportedModSandboxUnavailable
        | ReadOnlyInstallAutomationError::ImportedModFilesUnavailable
        | ReadOnlyInstallAutomationError::InstallManifestUnavailable
        | ReadOnlyInstallAutomationError::InstallRecoveryUnavailable
        | ReadOnlyInstallAutomationError::InstallRecoveryPreviewUnavailable => (
            CliErrorCategory::Recoverable,
            CliExitCode::RuntimeUnavailable,
            true,
        ),
    };

    write_command_error(
        format,
        command,
        CliErrorEnvelope::new(error.code(), category, retryable),
        exit_code,
        stdout,
        stderr,
    )
}

fn write_lifecycle_error<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    error: CliLifecycleAutomationError,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let (category, exit_code, retryable) = match &error {
        CliLifecycleAutomationError::WriteRejected => (
            CliErrorCategory::DataSafetyRisk,
            CliExitCode::Rejected,
            false,
        ),
        CliLifecycleAutomationError::PlanBlocked
        | CliLifecycleAutomationError::PlanTokenExpired
        | CliLifecycleAutomationError::PlanTokenInvalid
        | CliLifecycleAutomationError::RecoveryBlocked
        | CliLifecycleAutomationError::ReinstallBlocked
        | CliLifecycleAutomationError::UninstallBlocked => (
            CliErrorCategory::UserActionRequired,
            CliExitCode::Rejected,
            false,
        ),
        CliLifecycleAutomationError::PlanUnavailable
        | CliLifecycleAutomationError::RuntimeUnavailable
        | CliLifecycleAutomationError::TaskUnavailable => (
            CliErrorCategory::Recoverable,
            CliExitCode::RuntimeUnavailable,
            true,
        ),
        CliLifecycleAutomationError::TaskFailed { .. } => (
            CliErrorCategory::DataSafetyRisk,
            CliExitCode::ControlledFailure,
            false,
        ),
    };
    let mut envelope = CommandEnvelope::<serde_json::Value>::failure(
        command,
        CliErrorEnvelope::new(error.code(), category, retryable),
    );
    envelope.task_id = error.task_id().map(str::to_owned);
    let write_result = match format {
        OutputFormat::Human => writeln!(stderr, "error: {}", error.code()),
        OutputFormat::Json | OutputFormat::Jsonl => write_json_line(stdout, &envelope),
    };
    if write_result.is_ok() {
        exit_code.get()
    } else {
        let _ = writeln!(stderr, "cli_output_failed");
        CliExitCode::RuntimeUnavailable.get()
    }
}

fn write_backup_error<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    error: ReadOnlyBackupAutomationError,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let (category, exit_code, retryable) = match error {
        ReadOnlyBackupAutomationError::UnsupportedGame
        | ReadOnlyBackupAutomationError::ProfileIdInvalid
        | ReadOnlyBackupAutomationError::LimitInvalid => (
            CliErrorCategory::UserActionRequired,
            CliExitCode::Usage,
            false,
        ),
        ReadOnlyBackupAutomationError::SandboxStoragePathRejected
        | ReadOnlyBackupAutomationError::BackupStateInvalid => (
            CliErrorCategory::DataSafetyRisk,
            CliExitCode::Rejected,
            false,
        ),
        ReadOnlyBackupAutomationError::AppDataUnavailable
        | ReadOnlyBackupAutomationError::DatabaseUnavailable
        | ReadOnlyBackupAutomationError::BackgroundFixtureUnavailable
        | ReadOnlyBackupAutomationError::BackgroundStatusUnavailable => (
            CliErrorCategory::Recoverable,
            CliExitCode::RuntimeUnavailable,
            true,
        ),
    };
    write_command_error(
        format,
        command,
        CliErrorEnvelope::new(error.code(), category, retryable),
        exit_code,
        stdout,
        stderr,
    )
}

fn write_diagnostics_error<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    error: ReadOnlyDiagnosticsAutomationError,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let (category, exit_code) = match error {
        ReadOnlyDiagnosticsAutomationError::SandboxStoragePathRejected => {
            (CliErrorCategory::DataSafetyRisk, CliExitCode::Rejected)
        }
        ReadOnlyDiagnosticsAutomationError::AppDataUnavailable => (
            CliErrorCategory::Recoverable,
            CliExitCode::RuntimeUnavailable,
        ),
    };
    write_command_error(
        format,
        command,
        CliErrorEnvelope::new(error.code(), category, false),
        exit_code,
        stdout,
        stderr,
    )
}

fn write_batch_error<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    code: &'static str,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let (category, exit_code) = match code {
        "batch_item_invalid"
        | "batch_items_required"
        | "batch_game_invalid"
        | "batch_profile_invalid"
        | "batch_commit_required"
        | "batch_preview_token_required" => {
            (CliErrorCategory::UserActionRequired, CliExitCode::Usage)
        }
        _ => (CliErrorCategory::UserActionRequired, CliExitCode::Rejected),
    };
    write_command_error(
        format,
        command,
        CliErrorEnvelope::new(code, category, false),
        exit_code,
        stdout,
        stderr,
    )
}

fn write_batch_automation_error<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    error: BatchAutomationError,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let code = error.code();
    let (category, exit_code) = match error.class() {
        BatchAutomationErrorClass::DataSafetyRisk => {
            (CliErrorCategory::DataSafetyRisk, CliExitCode::Rejected)
        }
        BatchAutomationErrorClass::UserActionRequired => {
            (CliErrorCategory::UserActionRequired, CliExitCode::Rejected)
        }
        BatchAutomationErrorClass::Recoverable => (
            CliErrorCategory::Recoverable,
            CliExitCode::RuntimeUnavailable,
        ),
    };
    write_command_error(
        format,
        command,
        CliErrorEnvelope::new(code, category, error.retryable()),
        exit_code,
        stdout,
        stderr,
    )
}

fn write_command_error<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    error: CliErrorEnvelope,
    exit_code: CliExitCode,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let write_result = match format {
        OutputFormat::Human => writeln!(stderr, "error: {}", error.code),
        OutputFormat::Json | OutputFormat::Jsonl => write_json_line(
            stdout,
            &CommandEnvelope::<serde_json::Value>::failure(command, error),
        ),
    };

    if write_result.is_ok() {
        exit_code.get()
    } else {
        let _ = writeln!(stderr, "cli_output_failed");
        CliExitCode::RuntimeUnavailable.get()
    }
}

fn write_human_game_result<W: Write>(writer: &mut W, result: &GameCommandResult) -> io::Result<()> {
    match result {
        GameCommandResult::Status(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "status", &snapshot.status)?;
            write_human_value(writer, "error", &snapshot.error_code)
        }
        GameCommandResult::Scan(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            writeln!(writer, "candidates: {}", snapshot.candidate_count)?;
            writeln!(
                writer,
                "valid candidates: {}",
                snapshot.valid_candidate_count
            )?;
            writeln!(
                writer,
                "invalid candidates: {}",
                snapshot.invalid_candidate_count
            )?;
            write_human_value(writer, "max confidence", &snapshot.max_confidence)?;
            write_human_value(writer, "issues", &snapshot.issue_codes)
        }
        GameCommandResult::Validation(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "state", &snapshot.state)?;
            write_human_value(writer, "valid", &snapshot.valid)?;
            write_human_value(writer, "confidence", &snapshot.confidence)?;
            write_human_value(writer, "evidence", &snapshot.evidence)?;
            write_human_value(writer, "issues", &snapshot.issue_codes)
        }
        GameCommandResult::Prerequisites(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "state", &snapshot.state)?;
            write_human_value(writer, "status", &snapshot.status)?;
            writeln!(writer, "items: {}", snapshot.item_count)?;
            write_human_value(writer, "issues", &snapshot.issue_codes)?;
            write_human_value(writer, "error", &snapshot.error_code)?;
            for item in &snapshot.items {
                write_human_value(writer, &format!("item {}", item.code), &item.status)?;
                write_human_value(
                    writer,
                    &format!("item {} issues", item.code),
                    &item.issue_codes,
                )?;
            }
            Ok(())
        }
    }
}

fn write_human_install_result<W: Write>(
    writer: &mut W,
    result: &InstallCommandResult,
) -> io::Result<()> {
    match result {
        InstallCommandResult::Plan(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "profile", &snapshot.profile_id)?;
            write_human_value(writer, "mod", &snapshot.mod_id)?;
            writeln!(writer, "actions: {}", snapshot.action_count)?;
            writeln!(writer, "conflicts: {}", snapshot.conflict_count)?;
            writeln!(
                writer,
                "blocking conflicts: {}",
                snapshot.has_blocking_conflicts
            )?;
            for (index, action) in snapshot.actions.iter().enumerate() {
                write_human_value(
                    writer,
                    &format!("action {} target", index + 1),
                    &action.target_path,
                )?;
                writeln!(
                    writer,
                    "action {} priority: {}",
                    index + 1,
                    action.layer_priority
                )?;
            }
            for (index, conflict) in snapshot.conflicts.iter().enumerate() {
                write_human_value(
                    writer,
                    &format!("conflict {} target", index + 1),
                    &conflict.target_path,
                )?;
                writeln!(
                    writer,
                    "conflict {} providers: {}",
                    index + 1,
                    conflict.provider_count
                )?;
            }
            write_human_value(writer, "plan token", &snapshot.plan_token)?;
            write_human_value(writer, "plan expires at", &snapshot.expires_at_unix_millis)?;
            Ok(())
        }
        InstallCommandResult::BatchPlan(plan) => {
            write_human_value(writer, "game", &plan.plan.game_id)?;
            write_human_value(writer, "profile", &plan.plan.profile_id)?;
            write_human_value(writer, "operation", &plan.plan.operation)?;
            write_human_value(writer, "status", &plan.plan.status)?;
            writeln!(writer, "items: {}", plan.plan.item_count)?;
            writeln!(writer, "ready items: {}", plan.plan.ready_item_count)?;
            writeln!(writer, "blocked items: {}", plan.plan.blocked_item_count)?;
            writeln!(writer, "actions: {}", plan.plan.action_count)?;
            for reason in &plan.plan.global_blocking_reasons {
                writeln!(writer, "blocking reason {}: {}", reason.code, reason.count)?;
            }
            write_human_value(writer, "preview token", &plan.preview_token)?;
            write_human_value(writer, "preview expires at", &plan.expires_at_unix_millis)
        }
        InstallCommandResult::BatchAttempt(snapshot) => {
            write_human_value(writer, "batch", &snapshot.batch_id)?;
            write_human_value(writer, "operation", &snapshot.operation)?;
            writeln!(writer, "attempt: {}", snapshot.attempt_number)?;
            write_human_value(writer, "status", &snapshot.status)?;
            write_human_value(writer, "task", &snapshot.task_id)?;
            write_human_value(
                writer,
                "evidence degraded",
                &snapshot.evidence_health_degraded,
            )?;
            write_human_batch_summary(writer, &snapshot.summary)
        }
        InstallCommandResult::Uninstall(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "profile", &snapshot.profile_id)?;
            write_human_value(writer, "mod", &snapshot.mod_id)?;
            write_human_value(writer, "status", &snapshot.status)?;
            writeln!(writer, "available: {}", snapshot.available)?;
            writeln!(writer, "managed files: {}", snapshot.managed_file_count)?;
            writeln!(writer, "backups: {}", snapshot.backup_count)?;
            write_human_value(writer, "plan token", &snapshot.plan_token)?;
            write_human_value(writer, "plan expires at", &snapshot.expires_at_unix_millis)?;
            Ok(())
        }
        InstallCommandResult::Reinstall(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "profile", &snapshot.profile_id)?;
            write_human_value(writer, "mod", &snapshot.mod_id)?;
            write_human_value(
                writer,
                "candidate revision",
                &snapshot.candidate_revision_id,
            )?;
            write_human_value(writer, "status", &snapshot.status)?;
            write_human_value(
                writer,
                "installed revision",
                &snapshot.installed_revision_id,
            )?;
            writeln!(writer, "retained: {}", snapshot.retained_count)?;
            writeln!(writer, "replaced: {}", snapshot.replaced_count)?;
            writeln!(writer, "added: {}", snapshot.added_count)?;
            writeln!(writer, "stale: {}", snapshot.stale_count)?;
            for reason in &snapshot.blocking_reasons {
                writeln!(writer, "blocking reason {}: {}", reason.code, reason.count)?;
            }
            write_human_value(writer, "plan token", &snapshot.plan_token)?;
            write_human_value(writer, "plan expires at", &snapshot.expires_at_unix_millis)?;
            Ok(())
        }
        InstallCommandResult::Status(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "profile", &snapshot.profile_id)?;
            writeln!(writer, "items: {}", snapshot.item_count)?;
            for item in &snapshot.items {
                write_human_value(writer, &format!("mod {} status", item.mod_id), &item.status)?;
                writeln!(
                    writer,
                    "mod {} managed files: {}",
                    item.mod_id, item.managed_file_count
                )?;
                writeln!(writer, "mod {} backups: {}", item.mod_id, item.backup_count)?;
            }
            Ok(())
        }
        InstallCommandResult::RecoveryScan(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "profile", &snapshot.profile_id)?;
            writeln!(writer, "items: {}", snapshot.item_count)?;
            for item in &snapshot.items {
                write_human_value(writer, &format!("mod {} status", item.mod_id), &item.status)?;
                writeln!(
                    writer,
                    "mod {} managed files: {}",
                    item.mod_id, item.managed_file_count
                )?;
                writeln!(writer, "mod {} backups: {}", item.mod_id, item.backup_count)?;
                writeln!(writer, "mod {} issues: {}", item.mod_id, item.issue_count)?;
                for issue in &item.issues {
                    writeln!(
                        writer,
                        "mod {} issue {}: {}",
                        item.mod_id, issue.code, issue.count
                    )?;
                }
            }
            Ok(())
        }
        InstallCommandResult::RecoveryPreview(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "profile", &snapshot.profile_id)?;
            write_human_value(writer, "mod", &snapshot.mod_id)?;
            write_human_value(writer, "action", &snapshot.action)?;
            write_human_value(writer, "availability", &snapshot.availability)?;
            writeln!(writer, "remove files: {}", snapshot.remove_file_count)?;
            writeln!(writer, "restore files: {}", snapshot.restore_file_count)?;
            writeln!(writer, "backups: {}", snapshot.backup_count)?;
            writeln!(writer, "blocking issues: {}", snapshot.blocking_issue_count)?;
            for reason in &snapshot.blocking_reasons {
                writeln!(writer, "blocking reason {}: {}", reason.code, reason.count)?;
            }
            write_human_value(writer, "plan token", &snapshot.plan_token)?;
            write_human_value(writer, "plan expires at", &snapshot.expires_at_unix_millis)?;
            Ok(())
        }
    }
}

fn write_human_batch_apply_result<W: Write>(
    writer: &mut W,
    result: &BatchApplyResult,
) -> io::Result<()> {
    write_human_value(writer, "batch", &result.batch_id)?;
    write_human_value(writer, "operation", &result.operation)?;
    writeln!(writer, "attempt: {}", result.attempt)?;
    write_human_value(writer, "task", &result.task_id)?;
    write_human_value(writer, "status", &result.status)?;
    write_human_batch_summary(writer, &result.summary)
}

fn write_human_batch_summary<W: Write>(
    writer: &mut W,
    summary: &BatchResultSummarySnapshot,
) -> io::Result<()> {
    writeln!(writer, "items: {}", summary.item_count)?;
    writeln!(writer, "succeeded: {}", summary.succeeded_count)?;
    writeln!(writer, "failed: {}", summary.failed_count)?;
    writeln!(writer, "blocked: {}", summary.blocked_count)?;
    writeln!(writer, "cancelled: {}", summary.cancelled_count)?;
    writeln!(writer, "skipped: {}", summary.skipped_count)?;
    writeln!(
        writer,
        "recovery required: {}",
        summary.recovery_required_count
    )
}

fn write_batch_run_result<W: Write, E: Write>(
    format: OutputFormat,
    command: &'static str,
    result: BatchApplyResult,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let exit_code = batch_exit_code(result.status);
    let write_result = match format {
        OutputFormat::Human => write_human_batch_apply_result(stdout, &result),
        OutputFormat::Json => {
            let mut envelope = CommandEnvelope::success(command, result.clone());
            envelope.task_id = Some(result.task_id.clone());
            write_json_line(stdout, &envelope)
        }
        OutputFormat::Jsonl => {
            let event = batch_terminal_event(command, &result);
            let mut envelope = CommandEnvelope::success(command, result.clone());
            envelope.task_id = Some(result.task_id.clone());
            write_json_line(stdout, &event).and_then(|_| write_json_line(stdout, &envelope))
        }
    };
    if write_result.is_ok() {
        exit_code
    } else {
        let _ = writeln!(stderr, "cli_output_failed");
        CliExitCode::RuntimeUnavailable.get()
    }
}

fn batch_terminal_event(command: &'static str, result: &BatchApplyResult) -> TaskEventEnvelope {
    let operation = result.operation.as_str();
    let (event_type, status, phase_suffix, error_code) = match result.status {
        hmm_core::BatchAttemptStatus::Completed => (
            TaskEventType::Completed,
            CliTaskStatus::Completed,
            "completed",
            None,
        ),
        hmm_core::BatchAttemptStatus::CompletedWithErrors => (
            TaskEventType::Completed,
            CliTaskStatus::Completed,
            "completed_with_errors",
            None,
        ),
        hmm_core::BatchAttemptStatus::Cancelled => (
            TaskEventType::Cancelled,
            CliTaskStatus::Cancelled,
            "cancelled",
            None,
        ),
        hmm_core::BatchAttemptStatus::Blocked => (
            TaskEventType::Failed,
            CliTaskStatus::Failed,
            "failed",
            Some("batch_plan_blocked"),
        ),
        hmm_core::BatchAttemptStatus::RecoveryRequired => (
            TaskEventType::Failed,
            CliTaskStatus::Failed,
            "recovery_required",
            Some("batch_recovery_required"),
        ),
        hmm_core::BatchAttemptStatus::Interrupted => (
            TaskEventType::Failed,
            CliTaskStatus::Failed,
            "failed",
            Some("batch_interrupted"),
        ),
        hmm_core::BatchAttemptStatus::Failed => (
            TaskEventType::Failed,
            CliTaskStatus::Failed,
            "failed",
            Some("batch_failed"),
        ),
        hmm_core::BatchAttemptStatus::Sealed
        | hmm_core::BatchAttemptStatus::Queued
        | hmm_core::BatchAttemptStatus::Running
        | hmm_core::BatchAttemptStatus::Stopping => (
            TaskEventType::Failed,
            CliTaskStatus::Failed,
            "failed",
            Some("batch_attempt_reconciliation_required"),
        ),
    };
    let phase = format!("install.batch.{operation}.{phase_suffix}");
    let mut event = TaskEventEnvelope::new(
        event_type,
        command,
        0,
        result.task_id.clone(),
        status,
        phase,
    );
    event.current = u64::try_from(result.summary.item_count).ok();
    event.total = event.current;
    event.result = Some(serde_json::json!({
        "batchId": result.batch_id,
        "attempt": result.attempt,
        "status": result.status,
        "summary": result.summary,
    }));
    if let Some(error_code) = error_code {
        event.error = Some(TaskEventError::new(error_code));
    }
    event
}

fn write_human_backup_result<W: Write>(
    writer: &mut W,
    result: &BackupCommandResult,
) -> io::Result<()> {
    match result {
        BackupCommandResult::List(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "profile", &snapshot.profile_id)?;
            writeln!(writer, "items: {}", snapshot.item_count)?;
            for item in &snapshot.items {
                write_human_value(
                    writer,
                    &format!("backup {} trigger", item.backup_id),
                    &item.trigger,
                )?;
                write_human_value(
                    writer,
                    &format!("backup {} status", item.backup_id),
                    &item.status,
                )?;
                writeln!(
                    writer,
                    "backup {} created at: {}",
                    item.backup_id, item.created_at
                )?;
                writeln!(
                    writer,
                    "backup {} size bytes: {}",
                    item.backup_id, item.size_bytes
                )?;
                writeln!(
                    writer,
                    "backup {} files: {}",
                    item.backup_id, item.file_count
                )?;
            }
            Ok(())
        }
        BackupCommandResult::BackgroundStatus(snapshot) => {
            write_human_value(writer, "game", &snapshot.game_id)?;
            write_human_value(writer, "profile", &snapshot.profile_id)?;
            write_human_value(writer, "status", &snapshot.status)?;
            write_human_value(
                writer,
                "background protection enabled",
                &snapshot.background_protection_enabled,
            )?;
            write_human_value(writer, "last checked at", &snapshot.last_checked_at)?;
            write_human_value(writer, "last attempt at", &snapshot.last_attempt_at)?;
            write_human_value(writer, "last success at", &snapshot.last_success_at)?;
            write_human_value(writer, "next due at", &snapshot.next_due_at)?;
            write_human_value(writer, "pending reason", &snapshot.pending_reason)?;
            write_human_value(writer, "error", &snapshot.last_error_code)
        }
    }
}

fn write_human_diagnostics_result<W: Write>(
    writer: &mut W,
    snapshot: &DiagnosticsSnapshot,
) -> io::Result<()> {
    write_human_value(writer, "platform status", &snapshot.platform_status)?;
    write_human_value(writer, "app log status", &snapshot.app_log_status)?;
    writeln!(writer, "app log lines: {}", snapshot.app_log_line_count)?;
    write_human_value(writer, "task log status", &snapshot.task_log_status)?;
    writeln!(writer, "task log lines: {}", snapshot.task_log_line_count)?;
    write_human_value(writer, "audit log status", &snapshot.audit_log_status)?;
    writeln!(writer, "audit events: {}", snapshot.audit_event_count)?;
    if let Some(platform) = &snapshot.platform {
        write_human_value(writer, "app version", &platform.app_version)?;
        write_human_value(writer, "os", &platform.os)?;
        write_human_value(writer, "arch", &platform.arch)?;
        write_human_value(writer, "game adapters", &platform.game_adapter_ids)?;
    }
    Ok(())
}

fn write_human_value<W: Write, T: Serialize>(
    writer: &mut W,
    label: &str,
    value: &T,
) -> io::Result<()> {
    writeln!(writer, "{label}: {}", human_value(value)?)
}

fn human_value<T: Serialize>(value: &T) -> io::Result<String> {
    let value = serde_json::to_value(value).map_err(io::Error::other)?;
    match value {
        serde_json::Value::Null => Ok("none".to_owned()),
        serde_json::Value::Bool(value) => Ok(value.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::String(value) => Ok(value),
        serde_json::Value::Array(values) => {
            if values.is_empty() {
                return Ok("none".to_owned());
            }
            values
                .into_iter()
                .map(|value| match value {
                    serde_json::Value::String(value) => Ok(value),
                    _ => Err(io::Error::other("human_output_value_invalid")),
                })
                .collect::<io::Result<Vec<_>>>()
                .map(|values| values.join(", "))
        }
        serde_json::Value::Object(_) => Err(io::Error::other("human_output_value_invalid")),
    }
}

fn write_json_line<W: Write, T: Serialize>(writer: &mut W, value: &T) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, value).map_err(io::Error::other)?;
    writeln!(writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_output_is_uncolored_for_stable_cli_zero_contracts() {
        assert_eq!(Cli::command().get_color(), clap::ColorChoice::Never);
    }

    #[test]
    fn invalid_persisted_install_state_maps_to_data_safety_error() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = write_install_error(
            OutputFormat::Json,
            INSTALL_RECOVERY_SCAN_COMMAND,
            ReadOnlyInstallAutomationError::InstallStateInvalid,
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(exit_code, CliExitCode::Rejected.get());
        assert!(stderr.is_empty());
        let value: serde_json::Value =
            serde_json::from_slice(&stdout).expect("machine error envelope");
        assert_eq!(value["command"], INSTALL_RECOVERY_SCAN_COMMAND);
        assert_eq!(value["error"]["code"], "install_state_invalid");
        assert_eq!(value["error"]["category"], "data_safety_risk");
        assert_eq!(value["error"]["retryable"], false);
    }

    #[test]
    fn lifecycle_task_failure_does_not_claim_rollback_succeeded() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = write_lifecycle_error(
            OutputFormat::Json,
            INSTALL_REINSTALL_COMMAND,
            CliLifecycleAutomationError::TaskFailed {
                task_id: "install-opaque-task".to_owned(),
                code: "install_reinstall_task_failed",
            },
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(exit_code, CliExitCode::ControlledFailure.get());
        assert!(stderr.is_empty());
        let value: serde_json::Value =
            serde_json::from_slice(&stdout).expect("machine error envelope");
        assert_eq!(value["command"], INSTALL_REINSTALL_COMMAND);
        assert_eq!(value["taskId"], "install-opaque-task");
        assert_eq!(value["error"]["code"], "install_reinstall_task_failed");
        assert_eq!(value["error"]["category"], "data_safety_risk");
        assert_ne!(value["error"]["category"], "rollback_succeeded");
        assert_eq!(value["error"]["retryable"], false);
    }

    #[test]
    fn completed_with_errors_uses_partial_success_exit_code() {
        assert_eq!(
            batch_exit_code(hmm_core::BatchAttemptStatus::CompletedWithErrors),
            CliExitCode::PartialSuccess.get()
        );
    }

    #[test]
    fn batch_attempt_result_uses_authoritative_status_exit_code() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = BatchAttemptResult {
            batch_id: "batch-a".to_owned(),
            operation: BatchOperation::Uninstall,
            attempt_number: 0,
            status: hmm_core::BatchAttemptStatus::CompletedWithErrors,
            task_id: Some("task-a".to_owned()),
            evidence_health_degraded: false,
            summary: BatchResultSummarySnapshot {
                item_count: 2,
                succeeded_count: 1,
                blocked_count: 1,
                failed_count: 0,
                cancelled_count: 0,
                skipped_count: 0,
                recovery_required_count: 0,
            },
            items: Vec::new(),
        };

        assert_eq!(
            write_batch_attempt_result(
                OutputFormat::Json,
                INSTALL_BATCH_RESULT_COMMAND,
                result,
                &mut stdout,
                &mut stderr,
            ),
            CliExitCode::PartialSuccess.get()
        );
        assert!(stderr.is_empty());
        let value: serde_json::Value = serde_json::from_slice(&stdout).expect("batch result json");
        assert_eq!(value["result"]["status"], "completed_with_errors");
    }

    #[test]
    fn batch_non_terminal_attempt_results_remain_successful_queries() {
        for status in [
            hmm_core::BatchAttemptStatus::Sealed,
            hmm_core::BatchAttemptStatus::Queued,
            hmm_core::BatchAttemptStatus::Running,
            hmm_core::BatchAttemptStatus::Stopping,
        ] {
            assert_eq!(batch_result_exit_code(status), CliExitCode::Success.get());
        }
    }

    #[test]
    fn batch_terminal_attempt_results_preserve_apply_exit_codes() {
        for status in [
            hmm_core::BatchAttemptStatus::Completed,
            hmm_core::BatchAttemptStatus::CompletedWithErrors,
            hmm_core::BatchAttemptStatus::Blocked,
            hmm_core::BatchAttemptStatus::Cancelled,
            hmm_core::BatchAttemptStatus::RecoveryRequired,
            hmm_core::BatchAttemptStatus::Interrupted,
            hmm_core::BatchAttemptStatus::Failed,
        ] {
            assert_eq!(batch_result_exit_code(status), batch_exit_code(status));
        }
    }

    #[test]
    fn batch_human_summary_uses_scalar_counts() {
        let mut output = Vec::new();
        write_human_batch_summary(
            &mut output,
            &BatchResultSummarySnapshot {
                item_count: 7,
                succeeded_count: 1,
                blocked_count: 2,
                failed_count: 3,
                cancelled_count: 4,
                skipped_count: 5,
                recovery_required_count: 6,
            },
        )
        .expect("human batch summary");
        let output = String::from_utf8(output).expect("utf8");
        for expected in [
            "items: 7",
            "succeeded: 1",
            "blocked: 2",
            "failed: 3",
            "cancelled: 4",
            "skipped: 5",
            "recovery required: 6",
        ] {
            assert!(output.contains(expected), "{output}");
        }
        assert!(!output.contains('{'));
    }

    #[test]
    fn batch_attempt_human_summary_prints_item_count_once() {
        let mut output = Vec::new();
        write_human_install_result(
            &mut output,
            &InstallCommandResult::BatchAttempt(BatchAttemptResult {
                batch_id: "batch-a".to_owned(),
                operation: BatchOperation::Install,
                attempt_number: 0,
                status: hmm_core::BatchAttemptStatus::Completed,
                task_id: Some("task-a".to_owned()),
                evidence_health_degraded: false,
                summary: BatchResultSummarySnapshot {
                    item_count: 1,
                    succeeded_count: 1,
                    blocked_count: 0,
                    failed_count: 0,
                    cancelled_count: 0,
                    skipped_count: 0,
                    recovery_required_count: 0,
                },
                items: Vec::new(),
            }),
        )
        .expect("human batch attempt");
        let output = String::from_utf8(output).expect("utf8");
        assert_eq!(
            output
                .lines()
                .filter(|line| line.starts_with("items:"))
                .count(),
            1,
            "{output}"
        );
    }

    fn batch_request_options(
        operation: BatchOperationOption,
        items: &[&str],
        replacement_targets: &[&str],
    ) -> InstallBatchRequestOptions {
        InstallBatchRequestOptions {
            operation,
            game: "mhw".to_owned(),
            profile: "default".to_owned(),
            items: items.iter().map(|item| (*item).to_owned()).collect(),
            replacement_targets: replacement_targets
                .iter()
                .map(|target| (*target).to_owned())
                .collect(),
            policy: BatchExecutionPolicyOption::StopOnFailure,
        }
    }

    #[test]
    fn batch_request_parser_builds_operation_specific_item_union() {
        let install = batch_plan_request(&batch_request_options(
            BatchOperationOption::Install,
            &["mod-a:revision-a"],
            &[],
        ))
        .expect("install request");
        assert_eq!(install.plan.operation, BatchOperation::Install);
        assert!(matches!(install.plan.items[0], BatchItemInput::Install(_)));

        let uninstall = batch_plan_request(&batch_request_options(
            BatchOperationOption::Uninstall,
            &["mod-a:revision-a"],
            &[],
        ))
        .expect("uninstall request");
        assert_eq!(uninstall.plan.operation, BatchOperation::Uninstall);
        assert!(matches!(
            uninstall.plan.items[0],
            BatchItemInput::Uninstall(_)
        ));

        let reinstall = batch_plan_request(&batch_request_options(
            BatchOperationOption::Reinstall,
            &["mod-a:revision-a:revision-b"],
            &[],
        ))
        .expect("reinstall request");
        assert_eq!(reinstall.plan.operation, BatchOperation::Reinstall);
        assert!(matches!(
            reinstall.plan.items[0],
            BatchItemInput::Reinstall(_)
        ));
    }

    #[test]
    fn clap_accepts_reinstall_operation_and_separate_replacement_target() {
        let cli = Cli::try_parse_from([
            "hmm",
            "install",
            "batch",
            "plan",
            "--operation",
            "reinstall",
            "--item",
            "mod-a:revision-a:revision-a",
            "--replacement-target",
            "mod-a=mhw:armor:fatalis-beta",
        ])
        .expect("batch reinstall parser");
        let Commands::Install {
            command:
                InstallCommand::Batch {
                    command: InstallBatchCommand::Plan(options),
                },
        } = cli.command
        else {
            panic!("expected batch plan command");
        };
        assert_eq!(options.request.operation, BatchOperationOption::Reinstall);
        assert_eq!(
            options.request.replacement_targets,
            ["mod-a=mhw:armor:fatalis-beta"]
        );
    }

    #[test]
    fn same_revision_reinstall_requires_only_a_stable_target_identity() {
        let request = batch_plan_request(&batch_request_options(
            BatchOperationOption::Reinstall,
            &["mod-a:revision-a:revision-a"],
            &["mod-a=mhw:armor:fatalis-beta"],
        ))
        .expect("same revision retarget request");
        assert_eq!(
            request
                .replacement_targets
                .get(&ModId::new("mod-a"))
                .map(ReplacementTargetId::as_str),
            Some("mhw:armor:fatalis-beta")
        );
        let BatchItemInput::Reinstall(input) = &request.plan.items[0] else {
            panic!("expected reinstall item");
        };
        assert!(input.replacement_binding_snapshot.is_none());

        assert_eq!(
            batch_plan_request(&batch_request_options(
                BatchOperationOption::Reinstall,
                &["mod-a:revision-a:revision-a"],
                &[],
            )),
            Err("batch_item_invalid")
        );
        assert_eq!(
            batch_plan_request(&batch_request_options(
                BatchOperationOption::Reinstall,
                &["mod-a:revision-a:revision-a"],
                &[r"mod-a=C:\private\target"],
            )),
            Err("batch_item_invalid")
        );
        assert_eq!(
            batch_plan_request(&batch_request_options(
                BatchOperationOption::Reinstall,
                &["mod-a:revision-a:revision-b"],
                &["mod-a=mhw:armor:fatalis-beta"],
            )),
            Err("batch_item_invalid")
        );
    }

    #[test]
    fn batch_terminal_phase_uses_the_authoritative_operation() {
        let result = BatchApplyResult {
            batch_id: "batch-a".to_owned(),
            operation: BatchOperation::Uninstall,
            attempt: 0,
            task_id: "task-a".to_owned(),
            status: hmm_core::BatchAttemptStatus::CompletedWithErrors,
            summary: BatchResultSummarySnapshot {
                item_count: 1,
                succeeded_count: 0,
                blocked_count: 0,
                failed_count: 1,
                cancelled_count: 0,
                skipped_count: 0,
                recovery_required_count: 0,
            },
        };

        let value =
            serde_json::to_value(batch_terminal_event(INSTALL_BATCH_APPLY_COMMAND, &result))
                .expect("terminal event");
        assert_eq!(
            value["phase"],
            "install.batch.uninstall.completed_with_errors"
        );
    }
}
