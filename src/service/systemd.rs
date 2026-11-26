use super::{LABEL, Service};
use crate::error::{AppError, Result};
use crate::paths::Paths;
use crate::shell::Shell;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// A systemd user unit, which systemd starts at login and restarts when it fails.
pub struct UserUnit {
    paths: Arc<Paths>,
    shell: Arc<dyn Shell>,
}

impl UserUnit {
    pub fn new(paths: Arc<Paths>, shell: Arc<dyn Shell>) -> Self {
        Self { paths, shell }
    }

    fn unit(&self) -> PathBuf {
        self.paths
            .user_home
            .join(format!(".config/systemd/user/{LABEL}.service"))
    }

    fn systemctl(&self, arguments: &[&str]) -> Result<crate::command::CommandResult> {
        let mut full = vec!["--user"];
        full.extend_from_slice(arguments);
        Ok(self.shell.run("systemctl", &full)?)
    }

    fn checked(&self, arguments: &[&str]) -> Result<()> {
        let result = self.systemctl(arguments)?;
        if result.success {
            Ok(())
        } else {
            Err(AppError::Service(result.output()))
        }
    }
}

/// Quotes `value` for a unit file, where `%` starts a specifier.
fn quote(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("\"{escaped}\"")
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn render(paths: &Paths, extra: &[(String, String)]) -> String {
    let local_bin = paths.user_home.join(".local/bin");
    let uv_pythons = paths.user_home.join(".local/share/uv/python");
    let juliaup_bin = paths.user_home.join(".juliaup");
    let julia_depot = paths.user_home.join(".julia");
    let path = [
        text(&local_bin),
        text(&juliaup_bin.join("bin")),
        "/usr/local/bin".into(),
        "/usr/bin".into(),
        "/bin".into(),
    ]
    .join(":");
    let environment = [
        ("HOME", text(&paths.sandbox_home)),
        ("PATH", path),
        ("UV_PYTHON_INSTALL_DIR", text(&uv_pythons)),
        ("JULIAUP_DEPOT_PATH", text(&julia_depot)),
    ]
    .map(|(name, value)| (name.to_owned(), value))
    .into_iter()
    .chain(extra.iter().cloned())
    .map(|(name, value)| format!("Environment={}", quote(&format!("{name}={value}"))))
    .collect::<Vec<_>>()
    .join("\n");
    format!(
        "[Unit]\nDescription=ricochet server\n\n[Service]\nExecStart={binary} start --config {config}\nWorkingDirectory={home}\n{environment}\nRestart=on-failure\nStandardOutput=append:{stdout}\nStandardError=truncate:{stderr}\n\n[Install]\nWantedBy=default.target\n",
        binary = quote(&text(&paths.binary)),
        config = quote(&text(&paths.config)),
        home = text(&paths.home).replace(' ', "\\x20"),
        stdout = text(&paths.stdout).replace(' ', "\\x20"),
        stderr = text(&paths.stderr).replace(' ', "\\x20"),
    )
}

impl Service for UserUnit {
    fn write_job(&self, environment: &[(String, String)]) -> Result<bool> {
        self.shell.create_dir_all(&self.paths.sandbox_home)?;
        let contents = render(&self.paths, environment);
        let file = self.unit();
        if self
            .shell
            .read_text(&file)?
            .is_some_and(|existing| existing == contents)
        {
            return Ok(false);
        }
        // Readable by its owner only, because a proxy in its environment can hold a password.
        self.shell.write(&file, contents.as_bytes(), true)?;
        self.checked(&["daemon-reload"])?;
        Ok(true)
    }

    fn is_loaded(&self) -> bool {
        self.systemctl(&["show", "--property=ActiveState", "--value", LABEL])
            .is_ok_and(|result| {
                matches!(result.stdout.trim(), "active" | "activating" | "reloading")
            })
    }

    fn start(&self) -> Result<()> {
        self.checked(&["enable", "--now", LABEL])
    }

    fn stop(&self) -> Result<()> {
        if self.shell.read(&self.unit())?.is_none() {
            return Ok(());
        }
        self.checked(&["disable", "--now", LABEL])?;
        for _ in 0..40 {
            if !self.is_loaded() {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        self.checked(&["restart", LABEL])
    }

    fn remove(&self) -> Result<()> {
        self.stop()?;
        if self.shell.read(&self.unit())?.is_some() {
            self.shell.remove(&self.unit())?;
            self.checked(&["daemon-reload"])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandResult;
    use std::collections::HashMap;
    use std::io;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeShell {
        files: Mutex<HashMap<PathBuf, Vec<u8>>>,
        commands: Mutex<Vec<String>>,
    }

    impl Shell for FakeShell {
        fn run(&self, program: &str, arguments: &[&str]) -> io::Result<CommandResult> {
            self.commands
                .lock()
                .unwrap()
                .push(format!("{program} {}", arguments.join(" ")));
            Ok(CommandResult {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            })
        }
        fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>> {
            Ok(self.files.lock().unwrap().get(path).cloned())
        }
        fn write(&self, path: &Path, contents: &[u8], _: bool) -> io::Result<()> {
            self.files
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), contents.to_vec());
            Ok(())
        }
        fn remove(&self, path: &Path) -> io::Result<()> {
            self.files.lock().unwrap().remove(path);
            Ok(())
        }
        fn create_dir_all(&self, _: &Path) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn the_unit_names_the_binary_and_config() {
        let paths = Paths::under("/home/someone".into());
        let unit = render(&paths, &[]);
        assert!(unit.contains(r#"ExecStart="/home/someone/.local/bin/ricochet-server" start --config "/home/someone/.local/share/ricochet/ricochet-config.toml""#));
        assert!(unit.contains("WorkingDirectory=/home/someone/.local/share/ricochet\n"));
    }

    #[test]
    fn specifiers_in_values_are_escaped() {
        assert_eq!(quote("a%b\"c"), r#""a%%b\"c""#);
    }

    #[test]
    fn writing_the_job_reloads_systemd_once() {
        let shell = Arc::new(FakeShell::default());
        let unit = UserUnit::new(
            Arc::new(Paths::linux("/home/someone".into())),
            shell.clone(),
        );

        assert!(unit.write_job(&[]).unwrap());
        assert!(
            !unit.write_job(&[]).unwrap(),
            "an unchanged unit is not rewritten"
        );
        let reloads = shell
            .commands
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.as_str() == "systemctl --user daemon-reload")
            .count();
        assert_eq!(reloads, 1);
    }

    #[test]
    fn removing_the_job_stops_it_and_deletes_the_unit() {
        let shell = Arc::new(FakeShell::default());
        let unit = UserUnit::new(
            Arc::new(Paths::linux("/home/someone".into())),
            shell.clone(),
        );
        unit.write_job(&[]).unwrap();
        unit.remove().unwrap();
        assert!(shell.files.lock().unwrap().is_empty());
    }
}
