//! The app's own settings, in `settings.json` next to `servers.json`: the proxy it reaches servers and downloads through.

use crate::config::{Proxy, ProxyMode, ProxySettings};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;

/// The proxy the app uses, read once and kept current as you change it.
static PROXY: RwLock<Option<ProxySettings>> = RwLock::new(None);

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    #[serde(default)]
    proxy: StoredProxy,
}

#[derive(Default, Serialize, Deserialize)]
struct StoredProxy {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    bypass: String,
}

fn file() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("ricochet-app/settings.json"))
}

fn load() -> ProxySettings {
    let stored: Stored = file()
        .and_then(|file| std::fs::read_to_string(file).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    ProxySettings {
        mode: ProxyMode::parse(&stored.proxy.mode).unwrap_or(ProxyMode::System),
        url: stored.proxy.url,
        bypass: stored.proxy.bypass,
    }
}

/// The app's proxy setting.
pub fn proxy_settings() -> ProxySettings {
    if let Some(settings) = PROXY.read().ok().and_then(|proxy| proxy.clone()) {
        return settings;
    }
    let settings = load();
    if let Ok(mut current) = PROXY.write() {
        *current = Some(settings.clone());
    }
    settings
}

/// Saves the app's proxy setting, readable only by you because a proxy URL can hold a password.
pub fn set_proxy_settings(settings: ProxySettings) {
    if let Ok(mut current) = PROXY.write() {
        *current = Some(settings.clone());
    }
    let Some(file) = file() else { return };
    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }
    let stored = Stored {
        proxy: StoredProxy {
            mode: settings.mode.raw().to_owned(),
            url: settings.url,
            bypass: settings.bypass,
        },
    };
    let Ok(text) = serde_json::to_string_pretty(&stored) else {
        return;
    };
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    if let Ok(mut out) = options.open(&file) {
        let _ = std::io::Write::write_all(&mut out, text.as_bytes());
    }
}

/// The proxy the app's downloads go through, or `None` to connect directly.
pub fn download_proxy() -> Option<Proxy> {
    proxy_settings().resolve(crate::net::system_proxy)
}

/// The proxy the web UI at `url` loads through, when you entered one yourself.
/// The system choice needs none, because the webview follows the system proxy by itself.
pub fn webview_proxy(url: &str) -> Option<Proxy> {
    let settings = proxy_settings();
    if settings.mode != ProxyMode::Manual {
        return None;
    }
    let proxy = settings.resolve(|| None)?;
    let host = host_of(url)?;
    let bypassed = is_local(&host) || proxy.bypass.iter().any(|entry| matches_host(&host, entry));
    (!bypassed).then_some(proxy)
}

/// The host in `url`, without port or brackets.
fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = if let Some(inner) = authority.strip_prefix('[') {
        inner.split(']').next()?
    } else {
        authority.split(':').next()?
    };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

fn is_local(host: &str) -> bool {
    host == "localhost" || host == "::1" || host.starts_with("127.")
}

/// Whether `host` matches a bypass entry such as `intranet`, `.corp.example.com` or `*.corp.example.com`.
fn matches_host(host: &str, entry: &str) -> bool {
    let entry = entry.trim().trim_start_matches('*').to_ascii_lowercase();
    if entry.is_empty() {
        return false;
    }
    if let Some(suffix) = entry.strip_prefix('.') {
        host == suffix || host.ends_with(&format!(".{suffix}"))
    } else {
        host == entry || host.ends_with(&format!(".{entry}"))
    }
}

/// The host and port of a proxy URL such as `http://user:secret@proxy.corp:8080`, for the webview.
pub fn endpoint(proxy: &Proxy) -> Option<(String, String)> {
    let rest = proxy
        .url
        .split_once("://")
        .map_or(proxy.url.as_str(), |(_, rest)| rest);
    let authority = rest.split('/').next()?;
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let (host, port) = authority.rsplit_once(':').unwrap_or((authority, "80"));
    (!host.is_empty()).then(|| (host.to_owned(), port.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_come_out_of_urls() {
        assert_eq!(
            host_of("https://Ricochet.Example.com/app").as_deref(),
            Some("ricochet.example.com")
        );
        assert_eq!(
            host_of("http://localhost:6188").as_deref(),
            Some("localhost")
        );
        assert_eq!(host_of("http://[::1]:6188/").as_deref(), Some("::1"));
    }

    #[test]
    fn bypass_entries_match_a_domain_and_its_subdomains() {
        assert!(matches_host("a.corp.example.com", ".corp.example.com"));
        assert!(matches_host("corp.example.com", "*.corp.example.com"));
        assert!(matches_host("intranet", "intranet"));
        assert!(!matches_host("example.com", "corp.example.com"));
    }

    #[test]
    fn endpoints_drop_the_scheme_and_credentials() {
        let proxy = Proxy {
            url: "http://jo:secret@proxy.corp:3128".into(),
            bypass: Vec::new(),
        };
        assert_eq!(endpoint(&proxy), Some(("proxy.corp".into(), "3128".into())));
    }
}
