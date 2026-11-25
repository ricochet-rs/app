use super::choice::choice;
use super::section::{Field, SettingsSection, TableRead, TableWrite};
use toml_edit::{Table, TableLike};

choice! {
    ProxyMode {
        System => ("system", "Use the system settings"),
        Manual => ("manual", "Use this proxy"),
        Off => ("off", "Connect directly"),
    }
}

/// Hosts that always bypass the proxy, because ricochet and the app reach each other on them.
const LOCAL_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "::1"];

/// `[desktop.proxy]`, the HTTP proxy ricochet and its downloads use on that machine.
///
/// ricochet has no proxy setting: it, R, Python and Git read `HTTPS_PROXY` and `NO_PROXY`,
/// so the app writes these into the service's environment, and ricochet ignores the table.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxySettings {
    pub mode: ProxyMode,
    pub url: String,
    /// Hosts that bypass the proxy, separated by commas, such as `.corp.example.com`.
    pub bypass: String,
}

impl Default for ProxySettings {
    fn default() -> Self {
        Self {
            mode: ProxyMode::System,
            url: String::new(),
            bypass: String::new(),
        }
    }
}

/// A proxy and the hosts that bypass it, as found on a machine or typed into the form.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Proxy {
    pub url: String,
    pub bypass: Vec<String>,
}

impl Proxy {
    /// The variables that point ricochet, R, Python, Git and curl at the proxy, in both cases.
    pub fn environment(&self) -> Vec<(String, String)> {
        let mut bypass: Vec<String> = LOCAL_HOSTS.iter().map(|host| (*host).to_owned()).collect();
        for host in &self.bypass {
            if !bypass.contains(host) {
                bypass.push(host.clone());
            }
        }
        let bypass = bypass.join(",");
        [
            ("HTTP_PROXY", &self.url),
            ("HTTPS_PROXY", &self.url),
            ("http_proxy", &self.url),
            ("https_proxy", &self.url),
            ("NO_PROXY", &bypass),
            ("no_proxy", &bypass),
        ]
        .map(|(name, value)| (name.to_owned(), value.clone()))
        .to_vec()
    }

    /// The proxy named by `HTTPS_PROXY`, `HTTP_PROXY` or their lowercase forms in `variables`.
    pub fn from_environment<'a>(
        variables: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Option<Self> {
        let variables: Vec<(&str, &str)> = variables.into_iter().collect();
        let find = |names: &[&str]| {
            names.iter().find_map(|name| {
                variables
                    .iter()
                    .find(|(key, value)| key == name && !value.trim().is_empty())
                    .map(|(_, value)| value.trim().to_owned())
            })
        };
        let url = find(&["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"])?;
        let bypass = find(&["NO_PROXY", "no_proxy"]).unwrap_or_default();
        Some(Self {
            url,
            bypass: split_hosts(&bypass),
        })
    }
}

/// Splits a bypass list typed with commas, spaces or semicolons, as Windows separates it.
pub fn split_hosts(text: &str) -> Vec<String> {
    text.split([',', ';', ' ', '\n'])
        .map(str::trim)
        .filter(|host| !host.is_empty() && *host != "<local>")
        .map(str::to_owned)
        .collect()
}

impl ProxySettings {
    /// The proxy to use, given the one the machine's own settings name for `System`.
    pub fn resolve(&self, system: impl FnOnce() -> Option<Proxy>) -> Option<Proxy> {
        match self.mode {
            ProxyMode::System => system(),
            ProxyMode::Manual => Some(Proxy {
                url: self.url.trimmed().to_owned(),
                bypass: split_hosts(&self.bypass),
            }),
            ProxyMode::Off => None,
        }
    }
}

impl SettingsSection for ProxySettings {
    fn from_table(table: &dyn TableLike) -> Self {
        let Some(proxy) = table
            .child("desktop")
            .and_then(|desktop| desktop.child("proxy"))
        else {
            return Self::default();
        };
        Self {
            mode: proxy
                .string("mode")
                .and_then(|raw| ProxyMode::parse(&raw))
                .unwrap_or(ProxyMode::System),
            url: proxy.string("url").unwrap_or_default(),
            bypass: proxy.strings("bypass").unwrap_or_default().join(", "),
        }
    }

    fn apply(&self, table: &mut Table) {
        if self.mode == ProxyMode::System {
            if let Some(desktop) = table
                .get_mut("desktop")
                .and_then(|item| item.as_table_mut())
            {
                desktop.remove("proxy");
            }
            table.remove_if_empty("desktop");
            return;
        }
        let proxy = table.section("desktop").section("proxy");
        proxy.set("mode", Some(self.mode.raw()));
        let manual = self.mode == ProxyMode::Manual;
        proxy.set("url", manual.then(|| self.url.trimmed().to_owned()));
        let hosts = split_hosts(&self.bypass);
        proxy.set(
            "bypass",
            (manual && !hosts.is_empty()).then(|| {
                hosts
                    .iter()
                    .map(String::as_str)
                    .collect::<toml_edit::Array>()
            }),
        );
    }

    fn problem(&self) -> Option<String> {
        (self.mode == ProxyMode::Manual && !self.url.is_http_url())
            .then(|| "Enter the proxy's address, such as http://proxy.example.com:8080.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(settings: &ProxySettings) -> (String, ProxySettings) {
        let mut document = toml_edit::DocumentMut::new();
        settings.apply(document.as_table_mut());
        let text = document.to_string();
        let parsed: toml_edit::DocumentMut = text.parse().unwrap();
        (text, ProxySettings::from_table(parsed.as_table()))
    }

    #[test]
    fn system_settings_write_nothing() {
        let (text, parsed) = round_trip(&ProxySettings::default());
        assert_eq!(text.trim(), "");
        assert_eq!(parsed, ProxySettings::default());
    }

    #[test]
    fn a_manual_proxy_round_trips() {
        let settings = ProxySettings {
            mode: ProxyMode::Manual,
            url: " http://proxy.corp:8080 ".into(),
            bypass: ".corp.example.com; intranet".into(),
        };
        let (text, parsed) = round_trip(&settings);
        assert!(text.contains("[desktop.proxy]"), "{text}");
        assert_eq!(parsed.url, "http://proxy.corp:8080");
        assert_eq!(parsed.bypass, ".corp.example.com, intranet");
        assert_eq!(settings.problem(), None);
    }

    #[test]
    fn the_environment_always_bypasses_this_machine() {
        let proxy = Proxy {
            url: "http://proxy:3128".into(),
            bypass: vec!["intranet".into(), "localhost".into()],
        };
        let environment = proxy.environment();
        let no_proxy = &environment
            .iter()
            .find(|(name, _)| name == "NO_PROXY")
            .unwrap()
            .1;
        assert_eq!(no_proxy, "localhost,127.0.0.1,::1,intranet");
        assert!(environment.contains(&("https_proxy".into(), "http://proxy:3128".into())));
    }

    #[test]
    fn proxies_are_read_from_variables() {
        let found = Proxy::from_environment([
            ("PATH", "/bin"),
            ("https_proxy", "http://p:1"),
            ("NO_PROXY", "a,b"),
        ]);
        assert_eq!(
            found,
            Some(Proxy {
                url: "http://p:1".into(),
                bypass: vec!["a".into(), "b".into()],
            })
        );
        assert_eq!(Proxy::from_environment([("PATH", "/bin")]), None);
    }
}
