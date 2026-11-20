use crate::error::{AppError, Result};
use crate::{archive, paths::Paths};
use serde::Deserialize;
use std::cmp::Ordering;
use std::io::Read;
use std::time::Duration;

/// A ricochet-server release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
}

impl Release {
    /// The target ricochet publishes a server for on this computer, or `None` when it publishes none.
    pub fn target_for(os: &str, arch: &str) -> Option<&'static str> {
        match (os, arch) {
            ("macos", "aarch64" | "arm64") => Some("aarch64-macos"),
            ("linux", "x86_64") => Some("x86_64"),
            ("linux", "aarch64" | "arm64") => Some("aarch64"),
            ("windows", "x86_64") => Some("x86_64-windows"),
            _ => None,
        }
    }

    /// The archive for `target`: a `.zip` for Windows, a `.tar.gz` everywhere else.
    pub fn archive_url(&self, target: &str) -> String {
        let version = &self.version;
        let extension = if target.ends_with("windows") {
            "zip"
        } else {
            "tar.gz"
        };
        format!(
            "https://s3.eu-central-003.backblazeb2.com/ricochet-server/{version}/ricochet-{version}-{target}.{extension}"
        )
    }

    pub fn latest() -> Result<Self> {
        #[derive(Deserialize)]
        struct Index {
            latest: String,
        }
        let mut response = ureq::get("https://ricochet.rs/releases.json")
            .config()
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .call()
            .map_err(|_| AppError::Download("the list of ricochet releases".into()))?;
        let mut body = String::new();
        response
            .body_mut()
            .as_reader()
            .read_to_string(&mut body)
            .map_err(|_| AppError::Download("the list of ricochet releases".into()))?;
        let index: Index = serde_json::from_str(&body)
            .map_err(|_| AppError::Download("the list of ricochet releases".into()))?;
        Ok(Self::from_tag(&index.latest))
    }

    fn from_tag(tag: &str) -> Self {
        Self {
            version: tag.strip_prefix('v').unwrap_or(tag).to_owned(),
        }
    }

    pub fn from_version_output(output: &str) -> Option<Self> {
        output.split_whitespace().nth(1).map(|version| Self {
            version: version.to_owned(),
        })
    }

    pub fn is_newer_than(&self, other: &Self) -> bool {
        compare_versions(&self.version, &other.version) == Ordering::Greater
    }

    /// Downloads this release and puts its binary at `paths.binary`, replacing any older one.
    pub fn install(&self, paths: &Paths) -> Result<()> {
        let target = Self::target_for(std::env::consts::OS, std::env::consts::ARCH)
            .ok_or(AppError::UnsupportedPlatform)?;
        let member = if target.ends_with("windows") {
            "ricochet-server.exe"
        } else {
            "ricochet-server"
        };
        archive::install_executable(
            member,
            &self.archive_url(target),
            &paths.binary,
            &format!("ricochet-server {}", self.version),
        )
    }
}

/// Compares dotted versions number by number, so 0.10.0 is newer than 0.9.0.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts = |version: &str| -> Vec<u64> {
        version
            .split('.')
            .map(|part| {
                part.trim_matches(|c: char| !c.is_ascii_digit())
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    };
    let (a, b) = (parts(a), parts(b));
    (0..a.len().max(b.len()))
        .map(|i| a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0)))
        .find(|ordering| ordering.is_ne())
        .unwrap_or(Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(Release::from_tag("v0.10.0").is_newer_than(&Release::from_tag("0.9.0")));
        assert!(!Release::from_tag("0.25.0").is_newer_than(&Release::from_tag("0.25.0")));
        assert!(Release::from_tag("0.25.1").is_newer_than(&Release::from_tag("0.25")));
    }

    #[test]
    fn every_published_target_has_an_archive() {
        assert_eq!(Release::target_for("linux", "x86_64"), Some("x86_64"));
        assert_eq!(Release::target_for("linux", "arm64"), Some("aarch64"));
        assert_eq!(
            Release::target_for("macos", "aarch64"),
            Some("aarch64-macos")
        );
        assert_eq!(Release::target_for("macos", "x86_64"), None);
        assert_eq!(
            Release::target_for("windows", "x86_64"),
            Some("x86_64-windows")
        );
    }

    #[test]
    fn windows_downloads_a_zip() {
        let release = Release::from_tag("v0.25.0");
        assert!(
            release
                .archive_url("x86_64-windows")
                .ends_with("ricochet-0.25.0-x86_64-windows.zip")
        );
        assert!(
            release
                .archive_url("x86_64")
                .ends_with("ricochet-0.25.0-x86_64.tar.gz")
        );
    }

    #[test]
    fn the_installed_version_is_the_second_word() {
        let release = Release::from_version_output("ricochet 0.25.0 (2026-11-02)\n").unwrap();
        assert_eq!(release.version, "0.25.0");
    }
}
