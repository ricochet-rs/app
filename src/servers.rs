//! The ricochet servers whose content the app shows, and the API keys it signs in to them with.

use crate::connections::Connection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A ricochet server the dashboard can show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    /// What the store files the server's key under.
    pub id: String,
    pub name: String,
    /// Where the server answers, as seen from this computer.
    pub url: String,
    /// Whether this app installed the server, so it can start it and reach it through SSH.
    pub managed: bool,
}

impl Server {
    /// The server this app manages on `connection`, answering on `url`.
    pub fn managed(connection: &Connection, url: String) -> Self {
        let id = match connection {
            Connection::Local => "managed:local".to_owned(),
            Connection::Ssh { host } => format!("managed:ssh:{host}"),
        };
        Self {
            id,
            name: connection.name(),
            url,
            managed: true,
        }
    }

    pub fn remote(url: &str) -> Self {
        let name = url
            .split_once("://")
            .map_or(url, |(_, rest)| rest)
            .to_owned();
        Self {
            id: url.to_owned(),
            name,
            url: url.to_owned(),
            managed: false,
        }
    }
}

/// What the dashboard remembers between launches, in `servers.json` next to `connections.json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Servers {
    /// Servers you added by address.
    #[serde(default)]
    pub urls: Vec<String>,
    /// API keys by server id.
    #[serde(default)]
    keys: BTreeMap<String, String>,
    /// The server shown when you last quit.
    #[serde(default)]
    pub active: Option<String>,
}

impl Servers {
    fn file() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("ricochet-app/servers.json"))
    }

    pub fn load() -> Self {
        Self::file()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Writes the store readable only by you, since it holds API keys.
    pub fn save(&self) {
        let Some(file) = Self::file() else { return };
        if let Some(folder) = file.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        let Ok(text) = serde_json::to_string_pretty(self) else {
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

    pub fn key(&self, server: &Server) -> Option<&str> {
        self.keys.get(&server.id).map(String::as_str)
    }

    pub fn set_key(&mut self, server: &Server, key: Option<String>) {
        match key {
            Some(key) => self.keys.insert(server.id.clone(), key),
            None => self.keys.remove(&server.id),
        };
    }

    /// Adds a server by address and returns it, or the one already saved at that address.
    pub fn add(&mut self, url: &str) -> Server {
        if !self.urls.iter().any(|saved| saved == url) {
            self.urls.push(url.to_owned());
        }
        Server::remote(url)
    }

    pub fn remove(&mut self, server: &Server) {
        self.urls.retain(|url| *url != server.id);
        self.keys.remove(&server.id);
        if self.active.as_deref() == Some(&server.id) {
            self.active = None;
        }
    }

    /// The managed server first, when there is one, then the servers you added.
    pub fn choices(&self, managed: Option<Server>) -> Vec<Server> {
        managed
            .into_iter()
            .chain(self.urls.iter().map(|url| Server::remote(url)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_servers_come_first_and_keys_follow_their_id() {
        let mut servers = Servers::default();
        let remote = servers.add("https://ricochet.example.com");
        servers.add("https://ricochet.example.com");
        servers.set_key(&remote, Some("rico_1".into()));
        let local = Server::managed(&Connection::Local, "http://localhost:6188".into());
        let choices = servers.choices(Some(local.clone()));
        assert_eq!(choices, [local.clone(), remote.clone()]);
        assert_eq!(choices[1].name, "ricochet.example.com");
        assert_eq!(servers.key(&remote), Some("rico_1"));
        assert_eq!(servers.key(&local), None);
    }

    #[test]
    fn removing_a_server_forgets_its_key() {
        let mut servers = Servers::default();
        let remote = servers.add("https://a.example.com");
        servers.set_key(&remote, Some("rico_1".into()));
        servers.active = Some(remote.id.clone());
        servers.remove(&remote);
        assert_eq!(servers, Servers::default());
    }
}
