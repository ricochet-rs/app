use std::path::PathBuf;

/// Where the app installs ricochet-server and where the server keeps its data.
#[derive(Clone, Debug)]
pub struct Paths {
    /// Read by the launchd job and the systemd unit, which Windows has neither of.
    #[cfg_attr(windows, allow(dead_code))]
    pub user_home: PathBuf,
    pub binary: PathBuf,
    pub home: PathBuf,
    pub config: PathBuf,
    pub logs: PathBuf,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
    pub renv: PathBuf,
    /// `HOME` for ricochet-server and its content, inside the one folder the sandbox lets them write.
    #[cfg_attr(windows, allow(dead_code))]
    pub sandbox_home: PathBuf,
    /// The app's own files, outside every folder the sandbox lets content read.
    pub app_support: PathBuf,
    /// Every failure the app showed, with the step it was on, so a cut-off message can still be read.
    pub app_log: PathBuf,
    /// The service's executable, which macOS shows by name in **Login Items & Extensions**.
    pub launcher: PathBuf,
}

impl Paths {
    pub fn new() -> Self {
        let user_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        if cfg!(windows) {
            let local = dirs::data_local_dir().unwrap_or_else(|| user_home.join("AppData/Local"));
            return Self::windows(user_home, &local);
        }
        Self::under(user_home)
    }

    /// The paths on Windows, under `%LOCALAPPDATA%`, where per-user programs and their data live.
    pub fn windows(user_home: PathBuf, local: &std::path::Path) -> Self {
        let home = local.join("ricochet");
        let app_support = local.join("ricochet-app");
        Self {
            binary: local.join("Programs/ricochet/ricochet-server.exe"),
            config: home.join("ricochet-config.toml"),
            logs: home.join("logs"),
            stdout: home.join("ricochet.stdout"),
            stderr: home.join("ricochet.err"),
            renv: home.join("vendor/r/renv.tar.gz"),
            sandbox_home: home.join("home"),
            app_log: app_support.join("ricochet-app.log"),
            launcher: app_support.join("ricochet-launcher.exe"),
            app_support,
            home,
            user_home,
        }
    }

    pub fn under(user_home: PathBuf) -> Self {
        let app_support = if cfg!(target_os = "macos") {
            user_home.join("Library/Application Support/Ricochet")
        } else {
            user_home.join(".local/share/ricochet-app")
        };
        Self::with_app_support(user_home, app_support)
    }

    /// The paths on a Linux computer whose home folder is `user_home`.
    #[cfg(all(test, target_os = "linux"))]
    pub fn linux(user_home: PathBuf) -> Self {
        let app_support = user_home.join(".local/share/ricochet-app");
        Self::with_app_support(user_home, app_support)
    }

    fn with_app_support(user_home: PathBuf, app_support: PathBuf) -> Self {
        let home = user_home.join(".local/share/ricochet");
        Self {
            binary: user_home.join(".local/bin/ricochet-server"),
            config: home.join("ricochet-config.toml"),
            logs: home.join("logs"),
            stdout: home.join("ricochet.stdout"),
            stderr: home.join("ricochet.err"),
            renv: home.join("vendor/r/renv.tar.gz"),
            sandbox_home: home.join("home"),
            app_log: app_support.join("ricochet-app.log"),
            launcher: app_support.join("ricochet"),
            app_support,
            home,
            user_home,
        }
    }
}

impl Default for Paths {
    fn default() -> Self {
        Self::new()
    }
}
