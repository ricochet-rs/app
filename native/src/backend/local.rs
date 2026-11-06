use super::Backend;
use crate::command;
use crate::error::{AppError, Result};
use crate::paths::Paths;
use crate::ports::NetworkPorts;
use crate::release::Release;
use crate::service::{self, Service};
use crate::shell::{LocalShell, Shell};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// This computer.
pub struct LocalBackend {
    paths: Arc<Paths>,
    service: Box<dyn Service>,
    shell: LocalShell,
}

impl LocalBackend {
    pub fn new() -> Self {
        let paths = Arc::new(Paths::new());
        Self {
            service: service::server(paths.clone()),
            paths,
            shell: LocalShell,
        }
    }

    /// Finds a file shipped with this app, next to it or in a macOS bundle's `Resources` or `Helpers`.
    pub(super) fn bundled(name: &str) -> Option<PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let folder = exe.parent()?;
        [
            folder.to_path_buf(),
            folder.join("../Resources"),
            folder.join("../Helpers"),
        ]
        .into_iter()
        .map(|folder| folder.join(name))
        .find(|candidate| candidate.is_file())
    }

    /// Copies renv and the service launcher out of this app, so moving the app never breaks ricochet.
    fn install_bundled_files(&self) -> Result<()> {
        let mut files = vec![("renv.tar.gz", self.paths.renv.clone())];
        if cfg!(target_os = "macos") {
            files.push(("ricochet-launcher", self.paths.launcher.clone()));
        }
        for (name, installed) in files {
            let Some(bundled) = Self::bundled(name) else {
                continue;
            };
            if std::fs::read(&bundled)? == std::fs::read(&installed).unwrap_or_default() {
                continue;
            }
            if let Some(folder) = installed.parent() {
                std::fs::create_dir_all(folder)?;
            }
            let _ = std::fs::remove_file(&installed);
            std::fs::copy(&bundled, &installed)?;
        }
        // launchd can't run a quarantined executable that Apple hasn't notarized.
        if cfg!(target_os = "macos") && self.paths.launcher.exists() {
            let launcher = self.paths.launcher.to_string_lossy();
            let _ = command::run(
                "/usr/bin/xattr",
                &["-d", "com.apple.quarantine", launcher.as_ref()],
                &[],
            );
        }
        Ok(())
    }
}

impl Backend for LocalBackend {
    fn is_remote(&self) -> bool {
        false
    }

    fn paths(&self) -> &Paths {
        &self.paths
    }

    fn shell(&self) -> &dyn Shell {
        &self.shell
    }

    fn service(&self) -> &dyn Service {
        self.service.as_ref()
    }

    fn health(&self, port: u16) -> bool {
        ureq::get(&format!("http://localhost:{port}/health"))
            .config()
            .timeout_global(Some(Duration::from_secs(2)))
            .build()
            .call()
            .is_ok_and(|response| response.status() == 200)
    }

    fn is_port_free(&self, port: u16) -> bool {
        NetworkPorts::is_free(port)
    }

    fn install_release(&self, release: &Release) -> Result<()> {
        release.install(&self.paths)
    }

    fn prepare(&self) -> Result<()> {
        std::fs::create_dir_all(&self.paths.home)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.paths.home, std::fs::Permissions::from_mode(0o700))?;
        }
        self.install_bundled_files()
    }

    fn uninstall(&self) -> Result<()> {
        let paths = &self.paths;
        for item in [&paths.home, &paths.app_support, &paths.binary] {
            if item.exists() {
                trash::delete(item).map_err(|error| AppError::UserCommand(error.to_string()))?;
            }
        }
        Ok(())
    }

    fn open_dashboard(&self, port: u16) -> Result<()> {
        let _ = open::that(format!("http://localhost:{port}"));
        Ok(())
    }

    fn open_logs(&self) -> bool {
        let paths = &self.paths;
        let _ = open::that(if paths.logs.exists() {
            &paths.logs
        } else {
            &paths.home
        });
        true
    }
}
