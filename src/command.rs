use std::ffi::OsStr;
use std::io;
use std::process::Command;

pub struct CommandResult {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandResult {
    /// Stdout then stderr, trimmed, for showing a failure.
    pub fn output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr).trim().to_owned()
    }
}

/// A command that, on Windows, opens no console window, because every console program the app runs would otherwise flash one.
pub fn new(executable: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

/// Runs an executable to completion. `environment` adds to, and overrides, the app's own environment.
pub fn run<S: AsRef<OsStr>>(
    executable: impl AsRef<OsStr>,
    arguments: &[S],
    environment: &[(&str, &str)],
) -> io::Result<CommandResult> {
    let output = new(executable)
        .args(arguments)
        .envs(environment.iter().copied())
        .output()?;
    Ok(CommandResult {
        success: output.status.success(),
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}
