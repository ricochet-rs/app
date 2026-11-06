use super::{LABEL, Service};
use crate::command;
use crate::error::{AppError, Result};
use crate::paths::Paths;
use plist::{Dictionary, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// Seatbelt rules for ricochet-server and every process it starts, which inherit them.
/// Nothing under `/Users` or `/Volumes` is readable except the listed folders, and only ricochet's folder is writable.
const SANDBOX_PROFILE: &str = r#"(version 1)
(allow default)
(deny appleevent-send)
(deny file-read-data (subpath "/Users") (subpath "/Volumes"))
(allow file-read-data
    (subpath (param "RICOCHET_HOME"))
    (subpath (param "LOCAL_BIN"))
    (subpath (param "UV_PYTHONS"))
    (subpath (param "JULIAUP_BIN"))
    (subpath (param "JULIAUP_DATA")))
(deny file-write*)
(allow file-write*
    (subpath (param "RICOCHET_HOME"))
    (subpath (param "JULIAUP_DATA"))
    (subpath "/private/tmp")
    (subpath "/private/var/folders")
    (subpath "/dev"))"#;

/// A launchd job in your login session, which launchd starts at login and restarts when it exits.
pub struct LaunchAgent {
    paths: Arc<Paths>,
}

impl LaunchAgent {
    pub fn new(paths: Arc<Paths>) -> Self {
        Self { paths }
    }

    fn plist(&self) -> PathBuf {
        self.paths
            .user_home
            .join(format!("Library/LaunchAgents/{LABEL}.plist"))
    }

    fn domain(&self) -> String {
        format!("gui/{}", unsafe { libc::getuid() })
    }

    fn service(&self) -> String {
        format!("{}/{LABEL}", self.domain())
    }

    fn launchctl(&self, arguments: &[&str]) -> Result<command::CommandResult> {
        Ok(command::run("/bin/launchctl", arguments, &[])?)
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl Service for LaunchAgent {
    fn write_job(&self) -> Result<bool> {
        if !Path::new(SANDBOX_EXEC).is_file() {
            return Err(AppError::SandboxUnavailable(SANDBOX_EXEC.into()));
        }
        let paths = &self.paths;
        let local_bin = paths.user_home.join(".local/bin");
        let uv_pythons = paths.user_home.join(".local/share/uv/python");
        let juliaup_bin = paths.user_home.join(".juliaup");
        let julia_depot = paths.user_home.join(".julia");

        let mut arguments = vec![text(&paths.launcher)];
        for (name, folder) in [
            ("RICOCHET_HOME", paths.home.clone()),
            ("LOCAL_BIN", local_bin.clone()),
            ("UV_PYTHONS", uv_pythons.clone()),
            ("JULIAUP_BIN", juliaup_bin.clone()),
            ("JULIAUP_DATA", julia_depot.join("juliaup")),
        ] {
            arguments.push("-D".into());
            arguments.push(format!("{name}={}", text(&folder)));
        }
        arguments.extend([
            "-p".into(),
            SANDBOX_PROFILE.into(),
            text(&paths.binary),
            "start".into(),
        ]);
        arguments.extend(["--config".into(), text(&paths.config)]);

        let path = [
            text(&local_bin),
            text(&juliaup_bin.join("bin")),
            "/opt/homebrew/bin".into(),
            "/usr/local/bin".into(),
            "/usr/bin".into(),
            "/bin".into(),
            "/usr/sbin".into(),
            "/sbin".into(),
        ]
        .join(":");

        std::fs::create_dir_all(&paths.sandbox_home)?;

        let mut environment = Dictionary::new();
        environment.insert("HOME".into(), text(&paths.sandbox_home).into());
        environment.insert("PATH".into(), path.into());
        environment.insert("UV_PYTHON_INSTALL_DIR".into(), text(&uv_pythons).into());
        environment.insert("JULIAUP_DEPOT_PATH".into(), text(&julia_depot).into());
        let mut keep_alive = Dictionary::new();
        keep_alive.insert("SuccessfulExit".into(), false.into());

        let mut job = Dictionary::new();
        job.insert("Label".into(), LABEL.into());
        job.insert(
            "ProgramArguments".into(),
            Value::Array(arguments.into_iter().map(Value::String).collect()),
        );
        job.insert("WorkingDirectory".into(), text(&paths.home).into());
        job.insert(
            "EnvironmentVariables".into(),
            Value::Dictionary(environment),
        );
        job.insert(
            "AssociatedBundleIdentifiers".into(),
            "rs.ricochet.mac".into(),
        );
        job.insert("RunAtLoad".into(), true.into());
        job.insert("KeepAlive".into(), Value::Dictionary(keep_alive));
        job.insert("StandardOutPath".into(), text(&paths.stdout).into());
        job.insert("StandardErrorPath".into(), text(&paths.stderr).into());

        let mut data = Vec::new();
        plist::to_writer_xml(&mut data, &Value::Dictionary(job))
            .map_err(|error| AppError::Service(error.to_string()))?;
        let file = self.plist();
        if std::fs::read(&file).is_ok_and(|existing| existing == data) {
            return Ok(false);
        }
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let staging = file.with_extension("plist.tmp");
        std::fs::write(&staging, data)?;
        std::fs::rename(&staging, &file)?;
        Ok(true)
    }

    fn is_loaded(&self) -> bool {
        self.launchctl(&["print", &self.service()])
            .is_ok_and(|result| result.success)
    }

    fn start(&self) -> Result<()> {
        self.launchctl(&["enable", &self.service()])?;
        if self.is_loaded() {
            return Ok(());
        }
        let _ = std::fs::remove_file(&self.paths.stderr);
        let result = self.launchctl(&["bootstrap", &self.domain(), &text(&self.plist())])?;
        if result.success {
            Ok(())
        } else {
            Err(AppError::Service(result.output()))
        }
    }

    fn stop(&self) -> Result<()> {
        self.launchctl(&["disable", &self.service()])?;
        if !self.is_loaded() {
            return Ok(());
        }
        let result = self.launchctl(&["bootout", &self.service()])?;
        if !result.success {
            return Err(AppError::Service(result.output()));
        }
        for _ in 0..40 {
            if !self.is_loaded() {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        let _ = std::fs::remove_file(&self.paths.stderr);
        let result = self.launchctl(&["kickstart", "-k", &self.service()])?;
        if result.success {
            Ok(())
        } else {
            Err(AppError::Service(result.output()))
        }
    }

    fn remove(&self) -> Result<()> {
        self.stop()?;
        let _ = std::fs::remove_file(self.plist());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_job_runs_the_server_through_the_launcher_and_the_sandbox() {
        let home = tempfile::tempdir().unwrap();
        let agent = LaunchAgent::new(Arc::new(Paths::under(home.path().to_path_buf())));

        assert!(agent.write_job().unwrap());
        assert!(
            !agent.write_job().unwrap(),
            "an unchanged job is not rewritten"
        );

        let job = plist::Value::from_file(agent.plist()).unwrap();
        let job = job.as_dictionary().unwrap();
        assert_eq!(job["Label"].as_string(), Some(LABEL));
        let arguments: Vec<&str> = job["ProgramArguments"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|a| a.as_string())
            .collect();
        assert!(arguments[0].ends_with("Application Support/Ricochet/ricochet"));
        assert!(arguments.contains(&"start"));
        assert!(arguments.iter().any(|a| a.starts_with("RICOCHET_HOME=")));
        assert!(
            job["EnvironmentVariables"].as_dictionary().unwrap()["HOME"]
                .as_string()
                .unwrap()
                .ends_with("ricochet/home")
        );
    }
}
