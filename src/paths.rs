use std::path::PathBuf;

/// Where the app installs ricochet-server and where the server keeps its data.
#[derive(Clone, Debug)]
pub struct Paths {
    pub user_home: PathBuf,
    pub binary: PathBuf,
    pub home: PathBuf,
    pub config: PathBuf,
    pub logs: PathBuf,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
    pub renv: PathBuf,
    /// `HOME` for ricochet-server and its content, inside the one folder the sandbox lets them write.
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
        Self::under(user_home)
    }

    pub fn under(user_home: PathBuf) -> Self {
        let app_support = if cfg!(target_os = "macos") {
            user_home.join("Library/Application Support/Ricochet")
        } else {
            user_home.join(".local/share/ricochet-app")
        };
        Self::with_app_support(user_home, app_support)
    }

    /// The paths on a Linux computer reached over SSH, where this app keeps nothing of its own.
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
