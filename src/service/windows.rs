//! The background job on Windows: a `Run` entry under the current user's registry key starts the
//! launcher at every sign-in, and the launcher keeps ricochet-server running without admin rights.

use super::{LABEL, Service};
use crate::command;
use crate::error::{AppError, Result};
use crate::paths::Paths;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct RunEntry {
    paths: Arc<Paths>,
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Quotes one argument the way Windows splits a command line.
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

impl RunEntry {
    pub fn new(paths: Arc<Paths>) -> Self {
        Self { paths }
    }

    /// The launcher's arguments: where to run, where to write output, and the server's own command line.
    fn arguments(&self) -> Vec<String> {
        let paths = &self.paths;
        vec![
            text(&paths.home),
            text(&paths.stdout),
            text(&paths.stderr),
            text(&paths.binary),
            "start".into(),
            "--config".into(),
            text(&paths.config),
        ]
    }

    /// The command line Windows runs at sign-in.
    fn command_line(&self) -> String {
        std::iter::once(text(&self.paths.launcher))
            .chain(self.arguments())
            .map(|argument| quote(&argument))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn job_file(&self) -> std::path::PathBuf {
        self.paths.app_support.join("ricochet-job.txt")
    }

    fn running(image: &str) -> bool {
        let filter = format!("IMAGENAME eq {image}");
        command::run("tasklist", &["/FI", &filter, "/FO", "CSV", "/NH"], &[])
            .is_ok_and(|result| result.stdout.to_lowercase().contains(&image.to_lowercase()))
    }

    fn kill(image: &str) {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/T", "/IM", image])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }

    fn launch(&self) -> Result<()> {
        std::fs::create_dir_all(&self.paths.home)?;
        std::process::Command::new(&self.paths.launcher)
            .args(self.arguments())
            .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
            .spawn()
            .map_err(|error| AppError::Service(error.to_string()))?;
        Ok(())
    }

    fn reg(arguments: &[&str]) -> Result<command::CommandResult> {
        Ok(command::run("reg", arguments, &[])?)
    }
}

impl Service for RunEntry {
    fn write_job(&self) -> Result<bool> {
        let contents = self.command_line();
        let file = self.job_file();
        if std::fs::read_to_string(&file).is_ok_and(|existing| existing == contents) {
            return Ok(false);
        }
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::write(&file, &contents)?;
        Ok(true)
    }

    fn is_loaded(&self) -> bool {
        Self::running("ricochet-launcher.exe") || Self::running("ricochet-server.exe")
    }

    fn start(&self) -> Result<()> {
        let result = Self::reg(&[
            "add",
            RUN_KEY,
            "/v",
            LABEL,
            "/t",
            "REG_SZ",
            "/d",
            &self.command_line(),
            "/f",
        ])?;
        if !result.success {
            return Err(AppError::Service(result.output()));
        }
        if !self.is_loaded() {
            self.launch()?;
        }
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let _ = Self::reg(&["delete", RUN_KEY, "/v", LABEL, "/f"]);
        Self::kill("ricochet-launcher.exe");
        Self::kill("ricochet-server.exe");
        for _ in 0..40 {
            if !self.is_loaded() {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(())
    }

    fn restart(&self) -> Result<()> {
        Self::kill("ricochet-launcher.exe");
        Self::kill("ricochet-server.exe");
        for _ in 0..40 {
            if !self.is_loaded() {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        self.launch()
    }

    fn remove(&self) -> Result<()> {
        self.stop()?;
        let _ = std::fs::remove_file(self.job_file());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sign_in_command_quotes_every_path() {
        let paths = Paths::windows(
            "C:/Users/Jo Doe".into(),
            Path::new("C:/Users/Jo Doe/AppData/Local"),
        );
        let line = RunEntry::new(Arc::new(paths)).command_line();
        assert!(
            line.starts_with(
                "\"C:/Users/Jo Doe/AppData/Local/ricochet-app/ricochet-launcher.exe\""
            )
        );
        assert!(line.contains("\"start\" \"--config\""));
    }
}
