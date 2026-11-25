//! Finds newer versions of this app on GitHub, installs them over the running copy, and starts the new one.

use crate::error::{AppError, Result};
use crate::{archive, command, release};
use serde::Deserialize;
use std::cmp::Ordering;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

/// This build's version, which `just` sets from the release tag; `0.0.0` marks a development build, which never updates.
pub const VERSION: &str = match option_env!("RICOCHET_APP_VERSION") {
    Some(version) => version,
    None => "0.0.0",
};

const LATEST_RELEASE: &str = "https://api.github.com/repos/ricochet-rs/app/releases/latest";

/// A newer version of the app and the file to install it from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub version: String,
    url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// The release file that holds `version` of the app for this platform.
fn asset_name(version: &str) -> Option<String> {
    if cfg!(target_os = "macos") {
        Some(format!("ricochet-{version}.dmg"))
    } else if cfg!(windows) {
        Some(format!("ricochet-{version}-windows-x86_64.zip"))
    } else if cfg!(target_os = "linux") {
        Some(format!("ricochet-{version}-x86_64.AppImage"))
    } else {
        None
    }
}

/// Whether this copy can replace itself: a release build running from its `.app`, its AppImage, or its folder on Windows.
pub fn can_update() -> bool {
    if VERSION == "0.0.0" {
        return false;
    }
    if cfg!(target_os = "macos") {
        app_bundle().is_some()
    } else if cfg!(target_os = "linux") {
        std::env::var_os("APPIMAGE").is_some()
    } else {
        cfg!(windows)
    }
}

/// The newest published release when it is newer than this build and has a file for this platform.
pub fn check() -> Result<Option<Update>> {
    let fail = || AppError::Download("the list of app releases".into());
    let mut response = crate::net::download(LATEST_RELEASE, Duration::from_secs(15))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "ricochet-app")
        .call()
        .map_err(|_| fail())?;
    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .read_to_string(&mut body)
        .map_err(|_| fail())?;
    let latest: GithubRelease = serde_json::from_str(&body).map_err(|_| fail())?;
    Ok(newer(&latest, VERSION))
}

fn newer(latest: &GithubRelease, current: &str) -> Option<Update> {
    let version = latest
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&latest.tag_name);
    if release::compare_versions(version, current) != Ordering::Greater {
        return None;
    }
    let name = asset_name(version)?;
    let asset = latest.assets.iter().find(|asset| asset.name == name)?;
    Some(Update {
        version: version.to_owned(),
        url: asset.browser_download_url.clone(),
    })
}

impl Update {
    /// Downloads this version and puts it where the running copy lives. The running copy keeps going until `relaunch`.
    pub fn install(&self) -> Result<()> {
        let bytes = archive::download(&self.url, &format!("version {} of the app", self.version))?;
        install_from(&bytes)
    }
}

/// Starts the copy `install` put in place; the caller quits right after.
pub fn relaunch() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let bundle = app_bundle().ok_or_else(not_installed)?;
        command::new("open").arg("-n").arg(bundle).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        let appimage = std::env::var_os("APPIMAGE").ok_or_else(not_installed)?;
        command::new(appimage).spawn()?;
    }
    #[cfg(windows)]
    {
        command::new(std::env::current_exe()?).spawn()?;
    }
    Ok(())
}

/// Removes what an update on Windows moved aside, which it couldn't delete while it was running.
pub fn clean_up() {
    #[cfg(windows)]
    if let Ok(exe) = std::env::current_exe()
        && let Some(folder) = exe.parent()
    {
        for name in WINDOWS_FILES {
            let _ = std::fs::remove_file(folder.join(name).with_extension("old"));
        }
    }
}

fn not_installed() -> AppError {
    AppError::Update("Only an installed copy of the app can update itself.".into())
}

/// The `.app` the running executable is in, such as `/Applications/ricochet.app`.
fn app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bundle = exe.ancestors().nth(3)?;
    (bundle.extension()? == "app").then(|| bundle.to_path_buf())
}

#[cfg(target_os = "macos")]
fn install_from(dmg: &[u8]) -> Result<()> {
    let bundle = app_bundle().ok_or_else(not_installed)?;
    let folder = bundle.parent().ok_or_else(not_installed)?;
    let work = std::env::temp_dir().join(format!("ricochet-update-{}", std::process::id()));
    std::fs::create_dir_all(&work)?;
    let image = work.join("ricochet.dmg");
    let mount = work.join("mount");
    std::fs::write(&image, dmg)?;

    let attached = command::run(
        "hdiutil",
        &[
            "attach".as_ref(),
            "-nobrowse".as_ref(),
            "-readonly".as_ref(),
            "-noautoopen".as_ref(),
            "-mountpoint".as_ref(),
            mount.as_os_str(),
            image.as_os_str(),
        ],
        &[],
    )?;
    if !attached.success {
        let _ = std::fs::remove_dir_all(&work);
        return Err(AppError::Update(format!(
            "Couldn't open the downloaded disk image. {}",
            attached.output()
        )));
    }
    let staged = folder.join(".ricochet-update.app");
    let _ = std::fs::remove_dir_all(&staged);
    let copied = command::run(
        "ditto",
        &[mount.join("ricochet.app").as_os_str(), staged.as_os_str()],
        &[],
    );
    let _ = command::run(
        "hdiutil",
        &["detach".as_ref(), "-quiet".as_ref(), mount.as_os_str()],
        &[],
    );
    let _ = std::fs::remove_dir_all(&work);
    let copied = copied?;
    if !copied.success {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(AppError::Update(format!(
            "Couldn't copy the new version into {}. {}",
            folder.display(),
            copied.output()
        )));
    }
    swap(&bundle, &staged, &folder.join(".ricochet-old.app"))
}

/// Puts `staged` at `current`, and puts `current` back when that fails.
#[cfg(target_os = "macos")]
fn swap(current: &std::path::Path, staged: &std::path::Path, old: &std::path::Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(old);
    std::fs::rename(current, old)?;
    if let Err(error) = std::fs::rename(staged, current) {
        let _ = std::fs::rename(old, current);
        return Err(error.into());
    }
    let _ = std::fs::remove_dir_all(old);
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_from(appimage: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let current = PathBuf::from(std::env::var_os("APPIMAGE").ok_or_else(not_installed)?);
    let staged = current.with_extension("download");
    std::fs::write(&staged, appimage)?;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&staged, &current)?;
    Ok(())
}

/// The files in the Windows zip, which sit together in the folder the app runs from.
#[cfg(windows)]
const WINDOWS_FILES: [&str; 3] = ["ricochet.exe", "ricochet-launcher.exe", "renv.tar.gz"];

#[cfg(windows)]
fn install_from(zip: &[u8]) -> Result<()> {
    let exe = std::env::current_exe()?;
    let folder = exe.parent().ok_or_else(not_installed)?;
    let mut files = Vec::new();
    for name in WINDOWS_FILES {
        let bytes = archive::from_zip(zip, name)
            .ok_or_else(|| AppError::Update(format!("The download is missing {name}.")))?;
        files.push((name, bytes));
    }
    for (name, bytes) in files {
        let destination = if name == "ricochet.exe" {
            exe.clone()
        } else {
            folder.join(name)
        };
        let staged = destination.with_extension("download");
        std::fs::write(&staged, bytes)?;
        // Windows can't replace a running .exe, but it can move one aside.
        let old = destination.with_extension("old");
        let _ = std::fs::remove_file(&old);
        if destination.exists() {
            std::fs::rename(&destination, &old)?;
        }
        std::fs::rename(&staged, &destination)?;
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn install_from(_: &[u8]) -> Result<()> {
    Err(not_installed())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, names: &[String]) -> GithubRelease {
        GithubRelease {
            tag_name: tag.into(),
            assets: names
                .iter()
                .map(|name| GithubAsset {
                    name: name.clone(),
                    browser_download_url: format!("https://example.com/{name}"),
                })
                .collect(),
        }
    }

    #[test]
    fn offers_a_newer_release_with_a_file_for_this_platform() {
        let name = asset_name("0.3.0").unwrap();
        let update = newer(&release("v0.3.0", std::slice::from_ref(&name)), "0.2.0").unwrap();
        assert_eq!(update.version, "0.3.0");
        assert!(update.url.ends_with(&name));
    }

    #[test]
    fn ignores_the_same_or_an_older_release() {
        let name = asset_name("0.2.0").unwrap();
        assert!(newer(&release("v0.2.0", std::slice::from_ref(&name)), "0.2.0").is_none());
        assert!(newer(&release("v0.2.0", &[name]), "0.10.0").is_none());
    }

    #[test]
    fn ignores_a_release_still_missing_this_platform() {
        assert!(newer(&release("v0.3.0", &[]), "0.2.0").is_none());
    }
}
