use crate::error::{AppError, Result};
use std::io::Read;
use std::path::Path;
use std::time::Duration;

/// The bytes of `member` in a `.tar.gz` archive, or `None` when it isn't there.
fn from_tar_gz(archive: &[u8], member: &str) -> Option<Vec<u8>> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries().ok()? {
        let mut entry = entry.ok()?;
        if entry.path().ok()?.as_ref() == Path::new(member) {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).ok()?;
            return Some(bytes);
        }
    }
    None
}

/// The bytes of `member` in a `.zip` archive, or `None` when it isn't there.
pub fn from_zip(archive: &[u8], member: &str) -> Option<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive)).ok()?;
    let mut entry = zip.by_name(member).ok()?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// The bytes at `url`, where `name` says what they are in the error shown when the download fails.
pub fn download(url: &str, name: &str) -> Result<Vec<u8>> {
    let fail = || AppError::Download(name.to_owned());
    let mut response = crate::net::download(url, Duration::from_secs(600))
        .call()
        .map_err(|_| fail())?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut bytes)
        .map_err(|_| fail())?;
    Ok(bytes)
}

/// Downloads the `.tar.gz` or `.zip` at `url` and installs the executable `member` from it at `destination`.
pub fn install_executable(member: &str, url: &str, destination: &Path, name: &str) -> Result<()> {
    let fail = || AppError::Download(name.to_owned());
    let archive = download(url, name)?;

    let binary = if url.ends_with(".zip") {
        from_zip(&archive, member)
    } else {
        from_tar_gz(&archive, member)
    }
    .ok_or_else(fail)?;

    if let Some(folder) = destination.parent() {
        std::fs::create_dir_all(folder)?;
    }
    // A running server holds its binary open, so replace the name rather than write through it.
    let staging = destination.with_extension("download");
    std::fs::write(&staging, binary)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755))?;
    }
    // Windows can't replace a running .exe, but it can move one aside.
    #[cfg(windows)]
    {
        let old = destination.with_extension("old");
        let _ = std::fs::remove_file(&old);
        if destination.exists() {
            let _ = std::fs::rename(destination, &old);
        }
    }
    std::fs::rename(&staging, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn finds_the_server_in_a_zip() {
        let mut bytes = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut bytes));
            zip.start_file(
                "ricochet-server.exe",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"MZ server").unwrap();
            zip.finish().unwrap();
        }
        assert_eq!(
            from_zip(&bytes, "ricochet-server.exe").as_deref(),
            Some(&b"MZ server"[..])
        );
        assert!(from_zip(&bytes, "missing.exe").is_none());
    }
}
