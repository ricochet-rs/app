//! A computer ricochet can run on: this one, or another reached over SSH.

mod local;
mod ssh;

pub use local::LocalBackend;
pub use ssh::{SshBackend, is_valid_host};

use crate::config::ServerConfig;
use crate::controller::ServerStatus;
use crate::error::{AppError, Result};
use crate::paths::Paths;
use crate::release::Release;
use crate::service::Service;
use crate::shell::Shell;
use std::time::Duration;

pub trait Backend: Send + Sync {
    fn is_remote(&self) -> bool;
    /// Paths on that computer.
    fn paths(&self) -> &Paths;
    fn shell(&self) -> &dyn Shell;
    fn service(&self) -> &dyn Service;
    /// Whether ricochet's `/health` answers on that computer within two seconds.
    fn health(&self, port: u16) -> bool;
    /// Whether ricochet could bind `port` on that computer.
    fn is_port_free(&self, port: u16) -> bool;
    fn install_release(&self, release: &Release) -> Result<()>;
    /// Creates ricochet's folder and puts the files this app ships in it.
    fn prepare(&self) -> Result<()>;
    /// Moves ricochet and its data aside rather than deleting them, because that folder holds the only keys that decrypt its environment variables.
    fn uninstall(&self) -> Result<()>;
    /// Opens ricochet in a browser on this computer.
    fn open_dashboard(&self, port: u16) -> Result<()>;
    /// Opens the logs, returning whether this computer can show them.
    fn open_logs(&self) -> bool;
    /// Keeps ricochet running after you log out of that computer.
    fn keep_running_after_logout(&self) -> Result<()> {
        Ok(())
    }

    /// The answers saved on that computer, or `None` before the first setup.
    fn load_config(&self) -> Option<ServerConfig> {
        let text = self.shell().read_text(&self.paths().config).ok()??;
        ServerConfig::parse(&text)
    }

    fn installed(&self) -> Option<Release> {
        let binary = self.paths().binary.clone();
        self.shell().read(&binary).ok()??;
        let result = self
            .shell()
            .run(&binary.to_string_lossy(), &["--version"])
            .ok()?;
        result
            .success
            .then(|| Release::from_version_output(&result.stdout))
            .flatten()
    }

    fn save_config(&self, config: &ServerConfig, proxy_port: u16) -> Result<()> {
        let paths = self.paths();
        let existing = self.shell().read_text(&paths.config)?.unwrap_or_default();
        let text = config.render(&existing, proxy_port, &paths.home, &|port| {
            self.is_port_free(port)
        })?;
        self.shell().write(&paths.config, text.as_bytes(), true)?;
        Ok(())
    }

    /// Runs `ricochet-server user --config <file> ...` and returns what it prints.
    fn users(&self, arguments: &[&str]) -> Result<String> {
        let paths = self.paths();
        let binary = paths.binary.to_string_lossy().into_owned();
        let config = paths.config.to_string_lossy().into_owned();
        let mut full = vec!["user", "--config", config.as_str()];
        full.extend_from_slice(arguments);
        let result = self.shell().run(&binary, &full)?;
        if !result.success {
            return Err(AppError::UserCommand(result.output()));
        }
        Ok(result.stdout.trim().to_owned())
    }

    fn status(&self, installed: bool, port: u16) -> ServerStatus {
        if !installed {
            ServerStatus::NotInstalled
        } else if self.health(port) {
            ServerStatus::Running
        } else if self.service().is_loaded() {
            ServerStatus::Starting
        } else {
            ServerStatus::Stopped
        }
    }

    fn wait_until_running(&self, port: u16) -> Result<()> {
        for _ in 0..60 {
            if self.health(port) {
                return Ok(());
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        let log = self
            .shell()
            .read_text(&self.paths().stderr)
            .ok()
            .flatten()
            .unwrap_or_default();
        let lines: Vec<&str> = log.lines().collect();
        Err(AppError::DidNotStart(
            lines[lines.len().saturating_sub(8)..].join("\n"),
        ))
    }

    fn install_latest(&self, progress: &dyn Fn(String)) -> Result<()> {
        let release = Release::latest()?;
        progress(format!("Downloading ricochet {}", release.version));
        self.install_release(&release)
    }
}
