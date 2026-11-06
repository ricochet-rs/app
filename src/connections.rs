//! The computers ricochet can be managed on: this one, and SSH hosts you saved or already have in your SSH config.

use crate::backend::{Backend, LocalBackend, SshBackend};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Connection {
    Local,
    Ssh { host: String },
}

impl Connection {
    pub fn name(&self) -> String {
        match self {
            Self::Local => "This machine".into(),
            Self::Ssh { host } => host.clone(),
        }
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Ssh { .. })
    }

    /// Opens the connection, which for SSH logs in and checks the computer can run ricochet.
    pub fn connect(&self) -> Result<Arc<dyn Backend>> {
        Ok(match self {
            Self::Local => Arc::new(LocalBackend::new()),
            Self::Ssh { host } => Arc::new(SshBackend::connect(host)?),
        })
    }
}

/// What this app remembers between launches.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Store {
    /// SSH hosts you added.
    #[serde(default)]
    pub saved: Vec<String>,
    /// The computer you were managing when you last quit.
    #[serde(default)]
    pub active: Option<Connection>,
}

impl Store {
    fn file() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("ricochet-app/connections.json"))
    }

    pub fn load() -> Self {
        Self::file()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(file) = Self::file() else { return };
        if let Some(folder) = file.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(file, text);
        }
    }

    /// Remembers `connection` as the active one, and its host if it is new.
    pub fn remember(&mut self, connection: &Connection) {
        if let Connection::Ssh { host } = connection
            && !self.saved.contains(host)
        {
            self.saved.push(host.clone());
        }
        self.active = Some(connection.clone());
    }

    /// Every connection to offer: this computer, hosts you saved, then hosts from your SSH config.
    pub fn choices(&self, discovered: &[String]) -> Vec<Connection> {
        let mut hosts: Vec<&String> = Vec::new();
        for host in self.saved.iter().chain(discovered) {
            if !hosts.contains(&host) {
                hosts.push(host);
            }
        }
        std::iter::once(Connection::Local)
            .chain(
                hosts
                    .into_iter()
                    .map(|host| Connection::Ssh { host: host.clone() }),
            )
            .collect()
    }
}

/// The host aliases in an SSH config, without patterns such as `*`.
pub fn hosts_in_ssh_config(text: &str) -> Vec<String> {
    let mut hosts = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        if !words
            .next()
            .is_some_and(|keyword| keyword.eq_ignore_ascii_case("host"))
        {
            continue;
        }
        for alias in words {
            let is_pattern = alias.contains(['*', '?', '!']);
            if !is_pattern
                && crate::backend::is_valid_host(alias)
                && !hosts.iter().any(|h| h == alias)
            {
                hosts.push(alias.to_owned());
            }
        }
    }
    hosts
}

/// The host aliases in `~/.ssh/config`.
pub fn discover_ssh_hosts() -> Vec<String> {
    dirs::home_dir()
        .and_then(|home| std::fs::read_to_string(home.join(".ssh/config")).ok())
        .map(|text| hosts_in_ssh_config(&text))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_come_from_host_lines_without_patterns() {
        let config = "Host *\n  ForwardAgent no\nhost devbox box2\n  HostName 10.0.0.2\nHost *.internal !bad\nHost devbox\n";
        assert_eq!(hosts_in_ssh_config(config), ["devbox", "box2"]);
    }

    #[test]
    fn choices_start_with_this_computer_and_dedupe_hosts() {
        let store = Store {
            saved: vec!["devbox".into()],
            active: None,
        };
        let choices = store.choices(&["devbox".into(), "box2".into()]);
        assert_eq!(
            choices,
            [
                Connection::Local,
                Connection::Ssh {
                    host: "devbox".into()
                },
                Connection::Ssh {
                    host: "box2".into()
                }
            ]
        );
    }

    #[test]
    fn the_store_round_trips_through_json() {
        let mut store = Store::default();
        store.remember(&Connection::Ssh {
            host: "devbox".into(),
        });
        let text = serde_json::to_string(&store).unwrap();
        assert_eq!(serde_json::from_str::<Store>(&text).unwrap(), store);
        assert_eq!(store.saved, ["devbox"]);
    }
}
