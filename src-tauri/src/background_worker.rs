use std::ffi::OsString;

use hmm_runtime::{production_app_data_dir, HmmRuntime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundWorkerCommand {
    Once,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BackgroundWorkerEntryError {
    InvalidArgs,
    AppDataUnavailable,
    StateUnavailable,
    WorkerFailed(&'static str),
}

impl BackgroundWorkerEntryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgs => "save_backup_background_worker_invalid_args",
            Self::AppDataUnavailable => "save_backup_background_app_data_unavailable",
            Self::StateUnavailable => "save_backup_background_state_unavailable",
            Self::WorkerFailed(code) => code,
        }
    }
}

pub fn parse_background_worker_args<I, T>(
    args: I,
) -> Result<BackgroundWorkerCommand, BackgroundWorkerEntryError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();

    match args.as_slice() {
        [_, flag] if flag.to_str() == Some("--once") => Ok(BackgroundWorkerCommand::Once),
        _ => Err(BackgroundWorkerEntryError::InvalidArgs),
    }
}

pub fn run_save_backup_worker_once_from_env() -> Result<(), BackgroundWorkerEntryError> {
    parse_background_worker_args(std::env::args_os())?;

    let app_data_dir =
        production_app_data_dir().ok_or(BackgroundWorkerEntryError::AppDataUnavailable)?;
    let runtime = HmmRuntime::from_app_data_dir(app_data_dir)
        .map_err(|_| BackgroundWorkerEntryError::StateUnavailable)?;
    let worker_instance_id = format!("worker-{}", uuid::Uuid::new_v4());

    runtime
        .save_backup_background_worker
        .run_once(&worker_instance_id)
        .map_err(|error| BackgroundWorkerEntryError::WorkerFailed(error.code()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_background_worker_args, BackgroundWorkerCommand};

    #[test]
    fn parses_only_the_once_command() {
        assert_eq!(
            parse_background_worker_args(["hmm-save-backup-worker", "--once"]),
            Ok(BackgroundWorkerCommand::Once)
        );
    }

    #[test]
    fn rejects_paths_and_internal_scheduler_arguments() {
        for args in [
            ["hmm-save-backup-worker", "--save-directory", "C:/save"],
            ["hmm-save-backup-worker", "--profile", "default"],
            ["hmm-save-backup-worker", "--lease-owner", "worker-a"],
        ] {
            let error = parse_background_worker_args(args).expect_err("unsafe argument rejected");

            assert_eq!(error.code(), "save_backup_background_worker_invalid_args");
        }
    }

    #[test]
    fn rejects_missing_or_extra_arguments() {
        let missing = parse_background_worker_args(["hmm-save-backup-worker"])
            .expect_err("missing command rejected");
        assert_eq!(missing.code(), "save_backup_background_worker_invalid_args");

        let extra =
            parse_background_worker_args(["hmm-save-backup-worker", "--once", "--save-directory"])
                .expect_err("extra argument rejected");
        assert_eq!(extra.code(), "save_backup_background_worker_invalid_args");
    }
}
