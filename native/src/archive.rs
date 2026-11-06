use crate::error::{AppError, Result};
use std::io::Read;
use std::path::Path;
use std::time::Duration;

/// Downloads the `.tar.gz` at `url` and installs the executable `member` from it at `destination`.
pub fn install_executable(member: &str, url: &str, destination: &Path, name: &str) -> Result<()> {
    let fail = || AppError::Download(name.to_owned());
    let mut response = ureq::get(url)
        .config()
        .timeout_global(Some(Duration::from_secs(600)))
        .build()
        .call()
        .map_err(|_| fail())?;
    let mut archive = Vec::new();
    response
        .body_mut()
        .as_reader()
        .read_to_end(&mut archive)
        .map_err(|_| fail())?;

    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive.as_slice()));
    let mut binary = None;
    for entry in tar.entries().map_err(|_| fail())? {
        let mut entry = entry.map_err(|_| fail())?;
        if entry.path().map_err(|_| fail())?.as_ref() == Path::new(member) {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(|_| fail())?;
            binary = Some(bytes);
            break;
        }
    }
    let binary = binary.ok_or_else(fail)?;

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
    std::fs::rename(&staging, destination)?;
    Ok(())
}
