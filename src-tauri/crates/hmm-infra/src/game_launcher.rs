use hmm_ports::{GameLaunchError, GameLaunchRunner};
use std::process::Command;

#[derive(Debug, Default)]
pub struct SystemGameLaunchRunner;

impl GameLaunchRunner for SystemGameLaunchRunner {
    fn open_uri(&self, uri: &str) -> Result<(), GameLaunchError> {
        let mut command = platform_uri_command(uri)?;
        let status = command
            .status()
            .map_err(|error| GameLaunchError::LauncherUnavailable(error.to_string()))?;

        if status.success() {
            return Ok(());
        }

        Err(GameLaunchError::LaunchFailed(
            "platform URI opener exited unsuccessfully".to_owned(),
        ))
    }
}

#[cfg(target_os = "windows")]
fn platform_uri_command(uri: &str) -> Result<Command, GameLaunchError> {
    let mut command = Command::new("rundll32");
    command.arg("url.dll,FileProtocolHandler").arg(uri);
    Ok(command)
}

#[cfg(target_os = "macos")]
fn platform_uri_command(uri: &str) -> Result<Command, GameLaunchError> {
    let mut command = Command::new("open");
    command.arg(uri);
    Ok(command)
}

#[cfg(target_os = "linux")]
fn platform_uri_command(uri: &str) -> Result<Command, GameLaunchError> {
    let mut command = Command::new("xdg-open");
    command.arg(uri);
    Ok(command)
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn platform_uri_command(_uri: &str) -> Result<Command, GameLaunchError> {
    Err(GameLaunchError::LauncherUnavailable(
        "platform URI opener is unsupported".to_owned(),
    ))
}
