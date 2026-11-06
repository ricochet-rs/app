use super::Backend;
use super::local::LocalBackend;
use crate::command::CommandResult;
use crate::error::{AppError, Result};
use crate::paths::Paths;
use crate::release::Release;
use crate::service::{self, Service};
use crate::shell::Shell;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Quotes `value` as one word for a POSIX shell.
pub fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Whether `host` is safe to hand to `ssh`: an alias or `user@host`, never an option.
pub fn is_valid_host(host: &str) -> bool {
    !host.is_empty()
        && !host.starts_with('-')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-_@:[]%".contains(c))
}

/// What went wrong reaching `host`, from the text `ssh` printed, in words for the person using the app.
pub fn explain_ssh_error(host: &str, stderr: &str) -> String {
    let text = stderr.to_lowercase();
    if text.contains("could not resolve hostname") {
        format!(
            "Ricochet can't find a computer called {host}. Check the name, or add it to your SSH config."
        )
    } else if text.contains("permission denied") {
        format!(
            "{host} refused the login. Make sure `ssh {host}` signs in with a key and no password."
        )
    } else if text.contains("host key verification failed") {
        format!(
            "Your computer doesn't trust {host} yet. Run `ssh {host}` once in a terminal to accept its host key."
        )
    } else if text.contains("connection refused")
        || text.contains("timed out")
        || text.contains("no route to host")
    {
        format!("Ricochet can't reach {host}. Check that it is on and that SSH is running.")
    } else {
        format!("Ricochet couldn't connect to {host}. {}", stderr.trim())
    }
}

/// A computer reached through the system `ssh`, so your SSH config, agent and keys apply.
pub struct SshShell {
    host: String,
    control_dir: PathBuf,
}

impl SshShell {
    pub fn new(host: &str) -> Result<Self> {
        if !is_valid_host(host) {
            return Err(AppError::Connection(format!(
                "`{host}` isn't an SSH host name. Use an alias from your SSH config or user@host."
            )));
        }
        let control_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("ricochet-app");
        let _ = std::fs::create_dir_all(&control_dir);
        Ok(Self {
            host: host.to_owned(),
            control_dir,
        })
    }

    fn options(&self) -> Vec<String> {
        vec![
            "-o".into(),
            "BatchMode=yes".into(),
            "-o".into(),
            "ConnectTimeout=10".into(),
            "-o".into(),
            "ControlMaster=auto".into(),
            "-o".into(),
            "ControlPersist=120".into(),
            "-o".into(),
            format!("ControlPath={}/ssh-%C", self.control_dir.display()),
        ]
    }

    /// Runs `script` in the remote shell, feeding it `input`.
    fn script(&self, script: &str, input: Option<&[u8]>) -> io::Result<CommandResult> {
        let mut child = Command::new("ssh")
            .args(self.options())
            .arg(&self.host)
            .arg(script)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            stdin.write_all(input)?;
        }
        let output = child.wait_with_output()?;
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        // ssh itself exits 255 when it could not connect or authenticate.
        if output.status.code() == Some(255) {
            return Err(io::Error::other(stderr.trim().to_owned()));
        }
        Ok(CommandResult {
            success: output.status.success(),
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr,
        })
    }

    fn sh(&self, script: &str) -> io::Result<CommandResult> {
        self.script(script, None)
    }
}

impl Shell for SshShell {
    fn run(&self, program: &str, arguments: &[&str]) -> io::Result<CommandResult> {
        let words: Vec<String> = std::iter::once(program)
            .chain(arguments.iter().copied())
            .map(sh_quote)
            .collect();
        self.sh(&format!(
            "export XDG_RUNTIME_DIR=\"${{XDG_RUNTIME_DIR:-/run/user/$(id -u)}}\"; exec {}",
            words.join(" ")
        ))
    }

    fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>> {
        let path = sh_quote(&path.to_string_lossy());
        let result = self.sh(&format!(
            "if [ -f {path} ]; then cat {path}; else exit 3; fi"
        ))?;
        match result.code {
            Some(0) => Ok(Some(result.stdout.into_bytes())),
            Some(3) => Ok(None),
            _ => Err(io::Error::other(result.output())),
        }
    }

    fn write(&self, path: &Path, contents: &[u8], private: bool) -> io::Result<()> {
        let path = sh_quote(&path.to_string_lossy());
        let mode = if private { "600" } else { "644" };
        let result = self.script(
            &format!(
                "set -e; mkdir -p \"$(dirname {path})\"; umask 077; cat > {path}.tmp; chmod {mode} {path}.tmp; mv -f {path}.tmp {path}"
            ),
            Some(contents),
        )?;
        if result.success {
            Ok(())
        } else {
            Err(io::Error::other(result.output()))
        }
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        self.sh(&format!("rm -f {}", sh_quote(&path.to_string_lossy())))
            .map(|_| ())
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.sh(&format!("mkdir -p {}", sh_quote(&path.to_string_lossy())))
            .map(|_| ())
    }
}

/// A Linux computer reached over SSH.
pub struct SshBackend {
    host: String,
    paths: Arc<Paths>,
    shell: Arc<SshShell>,
    service: Box<dyn Service>,
    tunnel: Mutex<Option<(u16, Child)>>,
}

impl SshBackend {
    /// Connects to `host` and checks it can run ricochet, which needs Linux with systemd and curl.
    pub fn connect(host: &str) -> Result<Self> {
        let shell = Arc::new(SshShell::new(host)?);
        let connection_error =
            |error: io::Error| AppError::Connection(explain_ssh_error(host, &error.to_string()));
        let system = shell.run("uname", &["-sm"]).map_err(connection_error)?;
        let mut words = system.stdout.split_whitespace();
        if words.next() != Some("Linux") {
            return Err(AppError::Connection(format!(
                "{host} isn't running Linux. Ricochet can only manage Linux computers over SSH."
            )));
        }
        let needs = shell.sh("command -v curl >/dev/null && command -v systemctl >/dev/null")?;
        if !needs.success {
            return Err(AppError::Service(format!(
                "{host} needs curl and systemd to run ricochet."
            )));
        }
        let home = shell.sh("printf %s \"$HOME\"")?.stdout;
        let paths = Arc::new(Paths::linux(PathBuf::from(home.trim())));
        let service = service::systemd_unit(paths.clone(), shell.clone());
        Ok(Self {
            host: host.to_owned(),
            paths,
            shell,
            service,
            tunnel: Mutex::new(None),
        })
    }

    /// Keeps `ssh -L port:localhost:port` open so the dashboard answers on this computer.
    fn ensure_tunnel(&self, port: u16) -> Result<()> {
        let mut tunnel = self.tunnel.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((open_port, child)) = tunnel.as_mut() {
            if *open_port == port && matches!(child.try_wait(), Ok(None)) {
                return Ok(());
            }
            let _ = child.kill();
        }
        *tunnel = None;
        let mut child = Command::new("ssh")
            .args(self.shell.options())
            .args(["-N", "-o", "ExitOnForwardFailure=yes", "-L"])
            .arg(format!("{port}:localhost:{port}"))
            .arg(&self.host)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        std::thread::sleep(Duration::from_millis(900));
        if matches!(child.try_wait(), Ok(None)) {
            *tunnel = Some((port, child));
            return Ok(());
        }
        Err(AppError::Connection(format!(
            "Couldn't forward port {port} from {}. Another app on this computer may be using it.",
            self.host
        )))
    }
}

impl Drop for SshBackend {
    fn drop(&mut self) {
        if let Some((_, child)) = self
            .tunnel
            .get_mut()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            let _ = child.kill();
        }
    }
}

impl Backend for SshBackend {
    fn is_remote(&self) -> bool {
        true
    }

    fn paths(&self) -> &Paths {
        &self.paths
    }

    fn shell(&self) -> &dyn Shell {
        self.shell.as_ref()
    }

    fn service(&self) -> &dyn Service {
        self.service.as_ref()
    }

    fn health(&self, port: u16) -> bool {
        let url = format!("http://localhost:{port}/health");
        self.shell
            .run("curl", &["-fsS", "-o", "/dev/null", "-m", "2", &url])
            .is_ok_and(|result| result.success)
    }

    fn is_port_free(&self, port: u16) -> bool {
        let script =
            format!("ss -ltnH 'sport = :{port}' 2>/dev/null | grep -q . && exit 1; exit 0");
        self.shell.sh(&script).is_ok_and(|result| result.success)
    }

    fn install_release(&self, release: &Release) -> Result<()> {
        let machine = self.shell.run("uname", &["-m"])?.stdout;
        let target =
            Release::target_for("linux", machine.trim()).ok_or(AppError::UnsupportedPlatform)?;
        let binary = sh_quote(&self.paths.binary.to_string_lossy());
        let folder = sh_quote(
            &self
                .paths
                .binary
                .parent()
                .unwrap_or(Path::new("/"))
                .to_string_lossy(),
        );
        let url = sh_quote(&release.archive_url(target));
        let script = format!(
            "set -e; tmp=$(mktemp -d); trap 'rm -rf \"$tmp\"' EXIT; curl -fsSL {url} -o \"$tmp/r.tar.gz\"; tar -xzf \"$tmp/r.tar.gz\" -C \"$tmp\" ricochet-server; mkdir -p {folder}; mv -f \"$tmp/ricochet-server\" {binary}.new; chmod 755 {binary}.new; mv -f {binary}.new {binary}"
        );
        let result = self.shell.run("sh", &["-c", &script])?;
        if result.success {
            Ok(())
        } else {
            Err(AppError::UserCommand(format!(
                "{} couldn't download ricochet-server {}. {}",
                self.host,
                release.version,
                result.output()
            )))
        }
    }

    fn prepare(&self) -> Result<()> {
        let home = &self.paths.home;
        self.shell.create_dir_all(home)?;
        self.shell.run("chmod", &["700", &home.to_string_lossy()])?;
        if let Some(bundled) = LocalBackend::bundled("renv.tar.gz") {
            let contents = std::fs::read(bundled)?;
            let remote = self.paths.renv.to_string_lossy().into_owned();
            let size = self
                .shell
                .sh(&format!(
                    "wc -c < {} 2>/dev/null || true",
                    sh_quote(&remote)
                ))?
                .stdout;
            if size.trim() != contents.len().to_string() {
                self.shell.write(&self.paths.renv, &contents, false)?;
            }
        }
        Ok(())
    }

    fn uninstall(&self) -> Result<()> {
        let items: Vec<String> = [&self.paths.home, &self.paths.binary]
            .iter()
            .map(|path| sh_quote(&path.to_string_lossy()))
            .collect();
        let script = format!(
            "set -e; ts=$(date +%s); for p in {}; do if [ -e \"$p\" ]; then mv \"$p\" \"$p.removed-$ts\"; fi; done",
            items.join(" ")
        );
        let result = self.shell.sh(&script)?;
        if result.success {
            Ok(())
        } else {
            Err(AppError::UserCommand(result.output()))
        }
    }

    fn open_dashboard(&self, port: u16) -> Result<()> {
        self.ensure_tunnel(port)?;
        let _ = open::that(format!("http://localhost:{port}"));
        Ok(())
    }

    fn open_logs(&self) -> bool {
        false
    }

    fn keep_running_after_logout(&self) -> Result<()> {
        let result = self.shell.sh("loginctl enable-linger \"$(id -un)\"")?;
        if result.success {
            Ok(())
        } else {
            Err(AppError::Service(result.output()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_survives_single_quotes() {
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
        assert_eq!(sh_quote("a b"), "'a b'");
    }

    #[test]
    fn an_unreachable_host_fails_with_a_clear_message() {
        let error = SshBackend::connect("ricochet-test.invalid")
            .err()
            .expect("should not connect");
        assert!(
            error
                .to_string()
                .starts_with("Ricochet can't find a computer called ricochet-test.invalid."),
            "{error}"
        );
    }

    #[test]
    fn ssh_failures_are_explained_in_plain_words() {
        let say = |text| explain_ssh_error("devbox", text);
        assert!(say("Permission denied (publickey).").contains("refused the login"));
        assert!(say("Host key verification failed.").contains("host key"));
        assert!(
            say("ssh: connect to host devbox port 22: Connection refused")
                .contains("can't reach")
        );
        assert!(say("something odd").ends_with("something odd"));
    }

    #[test]
    fn hosts_cannot_be_options_or_commands() {
        assert!(is_valid_host("devbox"));
        assert!(is_valid_host("me@10.0.0.5"));
        assert!(is_valid_host("[fe80::1]"));
        assert!(!is_valid_host("-oProxyCommand=evil"));
        assert!(!is_valid_host("host; rm -rf /"));
        assert!(!is_valid_host(""));
    }
}
