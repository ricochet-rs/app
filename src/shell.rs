//! Running commands and touching files on this computer.

use crate::command::{self, CommandResult};
use std::io;
use std::path::Path;

pub trait Shell: Send + Sync {
    fn run(&self, program: &str, arguments: &[&str]) -> io::Result<CommandResult>;
    /// The file's text, or `None` when it does not exist.
    fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>>;
    /// Writes `contents` to `path`, creating its folder, readable only by this user when `private`.
    fn write(&self, path: &Path, contents: &[u8], private: bool) -> io::Result<()>;
    /// Only the systemd unit removes files and creates folders through a shell, so its tests can watch.
    #[cfg(target_os = "linux")]
    fn remove(&self, path: &Path) -> io::Result<()>;
    #[cfg(target_os = "linux")]
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;

    fn read_text(&self, path: &Path) -> io::Result<Option<String>> {
        Ok(self
            .read(path)?
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
    }
}

/// This computer.
pub struct LocalShell;

impl Shell for LocalShell {
    fn run(&self, program: &str, arguments: &[&str]) -> io::Result<CommandResult> {
        command::run(program, arguments, &[])
    }

    fn read(&self, path: &Path) -> io::Result<Option<Vec<u8>>> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn write(&self, path: &Path, contents: &[u8], private: bool) -> io::Result<()> {
        use std::io::Write;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        let staging = path.with_extension("tmp");
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, if private { 0o600 } else { 0o644 });
        let _ = private;
        options.open(&staging)?.write_all(contents)?;
        std::fs::rename(&staging, path)
    }

    #[cfg(target_os = "linux")]
    fn remove(&self, path: &Path) -> io::Result<()> {
        match std::fs::remove_file(path) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }

    #[cfg(target_os = "linux")]
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn private_files_are_readable_by_this_user_only() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("nested/ricochet-config.toml");
        LocalShell.write(&file, b"a = 1", true).unwrap();
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            LocalShell.read_text(&file).unwrap().as_deref(),
            Some("a = 1")
        );
        assert_eq!(
            LocalShell.read(&folder.path().join("missing")).unwrap(),
            None
        );
    }
}
