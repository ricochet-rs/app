//! HTTP requests from this computer, through the proxy the app's settings choose.

use crate::config::Proxy;
use std::time::Duration;

/// A request to the internet, through the app's proxy when there is one.
pub fn download(
    url: &str,
    timeout: Duration,
) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    let proxy =
        crate::settings::download_proxy().and_then(|proxy| ureq::Proxy::new(&proxy.url).ok());
    ureq::Agent::config_builder()
        .proxy(proxy)
        .timeout_global(Some(timeout))
        .build()
        .new_agent()
        .get(url)
}

/// A request to ricochet on this computer, which never goes through a proxy.
pub fn local(url: &str, timeout: Duration) -> ureq::RequestBuilder<ureq::typestate::WithoutBody> {
    ureq::Agent::config_builder()
        .proxy(None)
        .timeout_global(Some(timeout))
        .build()
        .new_agent()
        .get(url)
}

/// The proxy this computer's own settings name: System Settings on macOS, Internet Settings on Windows,
/// and `HTTPS_PROXY` on Linux. Proxy auto-configuration (PAC) isn't read.
pub fn system_proxy() -> Option<Proxy> {
    #[cfg(target_os = "macos")]
    {
        let output = crate::command::run("/usr/sbin/scutil", &["--proxy"], &[]).ok()?;
        scutil_proxy(&output.stdout)
    }
    #[cfg(windows)]
    {
        const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
        let value = |name: &str| {
            let output = crate::command::run("reg", &["query", KEY, "/v", name], &[]).ok()?;
            registry_value(&output.stdout, name)
        };
        windows_proxy(
            value("ProxyEnable").as_deref(),
            value("ProxyServer").as_deref(),
            value("ProxyOverride").as_deref(),
        )
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let variables: Vec<(String, String)> = std::env::vars().collect();
        Proxy::from_environment(
            variables
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        )
    }
}

/// The HTTPS proxy, or else the HTTP one, that `scutil --proxy` prints, with its exceptions.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn scutil_proxy(text: &str) -> Option<Proxy> {
    let value = |key: &str| {
        text.lines().find_map(|line| {
            let (name, value) = line.split_once(" : ")?;
            (name.trim() == key).then(|| value.trim().to_owned())
        })
    };
    let (host, port) = ["HTTPS", "HTTP"].into_iter().find_map(|kind| {
        (value(&format!("{kind}Enable")).as_deref() == Some("1"))
            .then(|| {
                Some((
                    value(&format!("{kind}Proxy"))?,
                    value(&format!("{kind}Port"))?,
                ))
            })
            .flatten()
    })?;
    let mut bypass = Vec::new();
    let mut in_exceptions = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("ExceptionsList") {
            in_exceptions = true;
        } else if in_exceptions && line == "}" {
            in_exceptions = false;
        } else if in_exceptions && let Some((_, host)) = line.split_once(" : ") {
            bypass.push(host.trim().trim_start_matches('*').to_owned());
        }
    }
    Some(Proxy {
        url: format!("http://{host}:{port}"),
        bypass,
    })
}

/// A value `reg query` printed, such as `ProxyServer    REG_SZ    proxy:8080`.
#[cfg_attr(not(windows), allow(dead_code))]
fn registry_value(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        (parts.next()? == name).then(|| {
            parts.next();
            parts.collect::<Vec<_>>().join(" ")
        })
    })
}

/// The proxy Windows' Internet Settings name, where `ProxyServer` is `host:port` or `http=host:port;https=host:port`.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_proxy(
    enable: Option<&str>,
    server: Option<&str>,
    bypass: Option<&str>,
) -> Option<Proxy> {
    if enable? != "0x1" {
        return None;
    }
    let server = server?;
    let address = if server.contains('=') {
        let named = |scheme: &str| {
            server.split(';').find_map(|part| {
                let (name, address) = part.split_once('=')?;
                (name.trim() == scheme).then(|| address.trim().to_owned())
            })
        };
        named("https").or_else(|| named("http"))?
    } else {
        server.trim().to_owned()
    };
    let url = if address.contains("://") {
        address
    } else {
        format!("http://{address}")
    };
    Some(Proxy {
        url,
        bypass: crate::config::split_hosts(bypass.unwrap_or_default()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_proxies_come_from_scutil() {
        let text = "<dictionary> {\n  ExceptionsList : <array> {\n    0 : *.local\n    1 : 169.254/16\n  }\n  HTTPEnable : 1\n  HTTPPort : 3128\n  HTTPProxy : proxy.corp\n  HTTPSEnable : 0\n}\n";
        assert_eq!(
            scutil_proxy(text),
            Some(Proxy {
                url: "http://proxy.corp:3128".into(),
                bypass: vec![".local".into(), "169.254/16".into()],
            })
        );
        assert_eq!(scutil_proxy("<dictionary> {\n  HTTPEnable : 0\n}\n"), None);
    }

    #[test]
    fn windows_proxies_come_from_internet_settings() {
        let printed = "\r\nHKEY_CURRENT_USER\\...\\Internet Settings\r\n    ProxyServer    REG_SZ    http=web:80;https=secure:8443\r\n";
        assert_eq!(
            registry_value(printed, "ProxyServer").as_deref(),
            Some("http=web:80;https=secure:8443")
        );
        let proxy = windows_proxy(
            Some("0x1"),
            Some("http=web:80;https=secure:8443"),
            Some("*.corp;<local>"),
        );
        assert_eq!(
            proxy,
            Some(Proxy {
                url: "http://secure:8443".into(),
                bypass: vec!["*.corp".into()],
            })
        );
        assert_eq!(windows_proxy(Some("0x0"), Some("web:80"), None), None);
    }
}
