//! A computer ricochet can run on: this one, or another reached over SSH.

mod local;
mod ssh;

pub use local::LocalBackend;
pub use ssh::{SshBackend, is_valid_host};

use crate::config::{Proxy, ProxySettings, ServerConfig};
use crate::controller::ServerStatus;
use crate::error::{AppError, Result};
use crate::paths::Paths;
use crate::release::Release;
use crate::service::Service;
use crate::shell::Shell;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Data in ricochet's home folder that can be deleted while ricochet stays installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Data {
    Cache,
    Database,
    Content,
}

impl Data {
    pub const ALL: [Self; 3] = [Self::Cache, Self::Database, Self::Content];

    pub fn title(self) -> &'static str {
        match self {
            Self::Cache => "Package cache",
            Self::Database => "Database",
            Self::Content => "Content",
        }
    }

    /// The files and folders under ricochet's `home` that hold this data.
    pub fn paths(self, home: &Path) -> Vec<PathBuf> {
        match self {
            Self::Cache => vec![home.join(".cache")],
            Self::Database => [
                "ricochet.sqlite3",
                "ricochet.sqlite3-shm",
                "ricochet.sqlite3-wal",
            ]
            .iter()
            .map(|name| home.join(name))
            .collect(),
            Self::Content => vec![home.join("content")],
        }
    }
}

/// One file or folder in a listing.
#[derive(Clone, Debug, PartialEq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    /// Seconds since the Unix epoch.
    pub modified: Option<i64>,
}

/// Folders first, then by name.
pub fn sort_entries(entries: &mut [DirEntry]) {
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

pub trait Backend: Send + Sync {
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
    /// Removes ricochet, its data and its encryption keys: to the Trash where there is one, for good on a Linux server.
    fn uninstall(&self) -> Result<()>;
    /// The files and folders directly inside `path` on that computer.
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>>;
    /// Removes `paths` on that computer, skipping any that don't exist.
    fn remove_data(&self, paths: &[PathBuf]) -> Result<()>;
    /// The proxy that computer's own settings name, for the Network proxy pane's system choice.
    fn system_proxy(&self) -> Option<Proxy>;
    /// Sends that computer's downloads of ricochet through the proxy in `environment`.
    fn use_proxy(&self, environment: &[(String, String)]);

    /// The proxy variables ricochet's service gets on that computer, as `settings` choose them.
    fn proxy_environment(&self, settings: &ProxySettings) -> Vec<(String, String)> {
        settings
            .resolve(|| self.system_proxy())
            .map(|proxy| proxy.environment())
            .unwrap_or_default()
    }

    /// The last `lines` lines of the text file at `path` on that computer.
    fn tail(&self, path: &Path, lines: usize) -> Result<String> {
        let count = lines.to_string();
        let result = self
            .shell()
            .run("tail", &["-n", &count, &path.to_string_lossy()])?;
        if result.success {
            Ok(result.stdout)
        } else {
            Err(AppError::UserCommand(result.output()))
        }
    }
    /// Opens ricochet in a browser on this computer.
    fn open_dashboard(&self, port: u16) -> Result<()>;
    /// Makes ricochet answer on `localhost:port` of this computer, so the app can call its API.
    fn reach_dashboard(&self, _port: u16) -> Result<()> {
        Ok(())
    }
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
