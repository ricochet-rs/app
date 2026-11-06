use std::path::PathBuf;

/// Everything that can stop ricochet from being set up or started, worded for the person using the app.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    UserCommand(String),
    #[error(
        "Ricochet can't read line {line} of ricochet-config.toml: {reason}. Fix the file, then save again."
    )]
    InvalidConfig { line: usize, reason: String },
    #[error("Ricochet can't find {0}, so it can't start inside its sandbox.")]
    SandboxUnavailable(PathBuf),
    #[error("Another app is using port {0}. Choose a different port.")]
    PortInUse(u16),
    #[error("Ricochet couldn't download {0}. Check your internet connection and try again.")]
    Download(String),
    #[error("Ricochet couldn't start in the background. {0}")]
    Service(String),
    #[error("{0}")]
    Connection(String),
    #[error("{}", did_not_start(.0))]
    DidNotStart(String),
    #[error("Ricochet doesn't publish a server for this computer yet.")]
    UnsupportedPlatform,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

fn did_not_start(log: &str) -> String {
    if log.is_empty() {
        "Ricochet didn't respond within a minute. Select Show logs for details.".into()
    } else {
        format!("Ricochet stopped before it was ready:\n{log}")
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
