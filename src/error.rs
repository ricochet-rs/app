use std::path::PathBuf;

/// Everything that can stop ricochet from being set up or started, worded for the person using the app.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    UserCommand(String),
    #[error(
        "Line {line} of ricochet-config.toml is invalid: {reason}. Fix the file and save again."
    )]
    InvalidConfig { line: usize, reason: String },
    #[error("ricochet can't start in its sandbox because {0} is missing.")]
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    SandboxUnavailable(PathBuf),
    #[error("Port {0} is in use. Choose another port.")]
    PortInUse(u16),
    #[error("Couldn't download {0}. Check your internet connection and try again.")]
    Download(String),
    #[error("Couldn't start ricochet in the background. {0}")]
    Service(String),
    #[error("{0}")]
    Connection(String),
    #[error("{}", did_not_start(.0))]
    DidNotStart(String),
    #[error("ricochet has no server build for this machine yet.")]
    UnsupportedPlatform,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

fn did_not_start(log: &str) -> String {
    if log.is_empty() {
        "ricochet didn't respond within a minute. Check Server > Logging > Live for details.".into()
    } else {
        format!("Ricochet stopped before it was ready:\n{log}")
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
