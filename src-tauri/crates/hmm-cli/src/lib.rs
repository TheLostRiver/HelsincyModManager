mod cancellation;
mod contract;
mod task_events;

use cancellation::{CliCancellationCoordinator, NoopCliTaskProgressObserver};
use clap::{Args, Parser, Subcommand, ValueEnum};
use hmm_runtime::{
    BackupBackgroundStatusSnapshot, BackupListSnapshot, DiagnosticsSnapshot,
    GamePrerequisiteSnapshot, GameScanSnapshot, GameStatusSnapshot, GameValidationSnapshot,
    InstallPlanSnapshot, InstallRecoveryPreviewSnapshot, InstallRecoveryScanSnapshot,
    InstallStatusSnapshot, LifecycleTaskOutcome, ReadOnlyBackupAutomation,
    ReadOnlyBackupAutomationError, ReadOnlyDiagnosticsAutomation,
    ReadOnlyDiagnosticsAutomationError, ReadOnlyGameAutomation, ReadOnlyGameAutomationError,
    ReadOnlyInstallAutomation, ReadOnlyInstallAutomationError, ReadOnlyInstallRecoveryAction,
    ReinstallPlanSnapshot, RuntimeEnvironment, RuntimeEnvironmentError, RuntimeEnvironmentKind,
    SandboxLifecycleAutomation, SandboxLifecycleAutomationError, TaskProgressEvent,
    TaskProgressObserver, UninstallPlanSnapshot,
};
use serde::Serialize;
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
    Uninstall(InstallUninstallOptions),
    Reinstall(InstallReinstallOptions),
    Status(InstallStatusOptions),
    Recovery {
        #[command(subcommand)]
        command: InstallRecoveryCommand,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SandboxLifecycleOperation {
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
        production_writes_allowed: false,
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

fn run_install_apply<W: Write + Send, E: Write>(
    format: OutputFormat,
    environment: &RuntimeEnvironment,
    options: InstallApplyOptions,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    if environment.kind() != RuntimeEnvironmentKind::Sandbox {
        return write_command_error(
            format,
            INSTALL_APPLY_COMMAND,
            CliErrorEnvelope::new(
                "production_write_command_forbidden",
                CliErrorCategory::DataSafetyRisk,
                false,
            ),
            CliExitCode::Rejected,
            stdout,
            stderr,
        );
    }

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
    let automation = match SandboxLifecycleAutomation::prepare_install(
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

    run_sandbox_lifecycle_operation(
        format,
        INSTALL_APPLY_COMMAND,
        &automation,
        SandboxLifecycleOperation::Install,
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
    if environment.kind() != RuntimeEnvironmentKind::Sandbox {
        return write_command_error(
            format,
            INSTALL_UNINSTALL_COMMAND,
            CliErrorEnvelope::new(
                "production_write_command_forbidden",
                CliErrorCategory::DataSafetyRisk,
                false,
            ),
            CliExitCode::Rejected,
            stdout,
            stderr,
        );
    }

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
    let automation = match SandboxLifecycleAutomation::prepare_uninstall(
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

    run_sandbox_lifecycle_operation(
        format,
        INSTALL_UNINSTALL_COMMAND,
        &automation,
        SandboxLifecycleOperation::Uninstall,
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
    if environment.kind() != RuntimeEnvironmentKind::Sandbox {
        return write_command_error(
            format,
            INSTALL_RECOVERY_APPLY_COMMAND,
            CliErrorEnvelope::new(
                "production_write_command_forbidden",
                CliErrorCategory::DataSafetyRisk,
                false,
            ),
            CliExitCode::Rejected,
            stdout,
            stderr,
        );
    }

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
    let automation = match SandboxLifecycleAutomation::prepare_recovery(
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

    run_sandbox_lifecycle_operation(
        format,
        INSTALL_RECOVERY_APPLY_COMMAND,
        &automation,
        SandboxLifecycleOperation::Recovery,
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
    if environment.kind() != RuntimeEnvironmentKind::Sandbox {
        return write_command_error(
            format,
            INSTALL_REINSTALL_COMMAND,
            CliErrorEnvelope::new(
                "production_write_command_forbidden",
                CliErrorCategory::DataSafetyRisk,
                false,
            ),
            CliExitCode::Rejected,
            stdout,
            stderr,
        );
    }

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
    let automation = match SandboxLifecycleAutomation::prepare_reinstall(
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

    run_sandbox_lifecycle_operation(
        format,
        INSTALL_REINSTALL_COMMAND,
        &automation,
        SandboxLifecycleOperation::Reinstall,
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

fn run_sandbox_lifecycle_operation<W: Write + Send, E: Write>(
    format: OutputFormat,
    command: &'static str,
    automation: &SandboxLifecycleAutomation,
    operation: SandboxLifecycleOperation,
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
            Err(SandboxLifecycleAutomationError::TaskFailed { .. }) => {
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
    automation: &SandboxLifecycleAutomation,
    operation: SandboxLifecycleOperation,
    observer: &O,
) -> Result<LifecycleTaskOutcome, SandboxLifecycleAutomationError> {
    match operation {
        SandboxLifecycleOperation::Install => automation.run_install_with_observer(observer),
        SandboxLifecycleOperation::Uninstall => automation.run_uninstall_with_observer(observer),
        SandboxLifecycleOperation::Reinstall => automation.run_reinstall_with_observer(observer),
        SandboxLifecycleOperation::Recovery => automation.run_recovery_with_observer(observer),
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
    error: SandboxLifecycleAutomationError,
    stdout: &mut W,
    stderr: &mut E,
) -> i32 {
    let (category, exit_code, retryable) = match &error {
        SandboxLifecycleAutomationError::ProductionForbidden
        | SandboxLifecycleAutomationError::WriteRejected => (
            CliErrorCategory::DataSafetyRisk,
            CliExitCode::Rejected,
            false,
        ),
        SandboxLifecycleAutomationError::PlanBlocked
        | SandboxLifecycleAutomationError::PlanTokenExpired
        | SandboxLifecycleAutomationError::PlanTokenInvalid
        | SandboxLifecycleAutomationError::RecoveryBlocked
        | SandboxLifecycleAutomationError::ReinstallBlocked
        | SandboxLifecycleAutomationError::UninstallBlocked => (
            CliErrorCategory::UserActionRequired,
            CliExitCode::Rejected,
            false,
        ),
        SandboxLifecycleAutomationError::PlanUnavailable
        | SandboxLifecycleAutomationError::RuntimeUnavailable
        | SandboxLifecycleAutomationError::TaskUnavailable => (
            CliErrorCategory::Recoverable,
            CliExitCode::RuntimeUnavailable,
            true,
        ),
        SandboxLifecycleAutomationError::TaskFailed { .. } => (
            CliErrorCategory::RollbackSucceeded,
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
}
