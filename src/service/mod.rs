//! The background job that keeps ricochet-server running: launchd on macOS, systemd on Linux, a sign-in `Run` entry on Windows.

use crate::error::Result;

#[cfg(target_os = "macos")]
mod launchd;

mod systemd;
#[cfg(windows)]
mod windows;

/// A job in your login session, which the system starts at login and restarts when it exits.
pub trait Service: Send + Sync {
    /// Writes the job that runs ricochet-server with the extra `environment`, such as its proxy,
    /// returning whether the job changed.
    fn write_job(&self, environment: &[(String, String)]) -> Result<bool>;
    fn is_loaded(&self) -> bool;
    /// Loads the job, which starts it now and at every login.
    fn start(&self) -> Result<()>;
    /// Unloads the job and disables it, so it stays stopped across logins until `start`.
    fn stop(&self) -> Result<()>;
    fn restart(&self) -> Result<()>;
    /// Stops the job and deletes it, so nothing starts it again.
    fn remove(&self) -> Result<()>;
}

pub const LABEL: &str = "rs.ricochet.server";

#[cfg(target_os = "macos")]
pub fn server(paths: std::sync::Arc<crate::paths::Paths>) -> Box<dyn Service> {
    Box::new(launchd::LaunchAgent::new(paths))
}

#[cfg(target_os = "linux")]
pub fn server(paths: std::sync::Arc<crate::paths::Paths>) -> Box<dyn Service> {
    systemd_unit(paths, std::sync::Arc::new(crate::shell::LocalShell))
}

/// The systemd user unit on a Linux computer reached through `shell`.
pub fn systemd_unit(
    paths: std::sync::Arc<crate::paths::Paths>,
    shell: std::sync::Arc<dyn crate::shell::Shell>,
) -> Box<dyn Service> {
    Box::new(systemd::UserUnit::new(paths, shell))
}

#[cfg(windows)]
pub fn server(paths: std::sync::Arc<crate::paths::Paths>) -> Box<dyn Service> {
    Box::new(windows::RunEntry::new(paths))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
pub fn server(_paths: std::sync::Arc<crate::paths::Paths>) -> Box<dyn Service> {
    Box::new(Unsupported)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
struct Unsupported;

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
impl Service for Unsupported {
    fn write_job(&self, _environment: &[(String, String)]) -> Result<bool> {
        Err(crate::error::AppError::UnsupportedPlatform)
    }
    fn is_loaded(&self) -> bool {
        false
    }
    fn start(&self) -> Result<()> {
        Err(crate::error::AppError::UnsupportedPlatform)
    }
    fn stop(&self) -> Result<()> {
        Ok(())
    }
    fn restart(&self) -> Result<()> {
        Err(crate::error::AppError::UnsupportedPlatform)
    }
    fn remove(&self) -> Result<()> {
        Ok(())
    }
}
