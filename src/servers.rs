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

/// A ricochet instance in the server menu: a machine this app manages, or a server added by address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instance {
    Machine(Connection),
    Added(Server),
}

impl Instance {
    /// What the store files the instance's name, group and place under.
    pub fn id(&self) -> String {
        match self {
            Self::Machine(connection) => Server::managed(connection, String::new()).id,
            Self::Added(server) => server.id.clone(),
        }
    }

    pub fn default_name(&self) -> String {
        match self {
            Self::Machine(connection) => connection.name(),
            Self::Added(server) => server.name.clone(),
        }
    }

    /// The Phosphor icon beside the instance.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Machine(Connection::Local) => "desktop",
            Self::Machine(Connection::Ssh { .. }) => "hard-drives",
            Self::Added(_) => "globe",
        }
    }
}

/// Where you put an instance: its own name, its group, and by its index in the list, its place.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// A group's name, or `None` for instances in no group, and its instances in order.
pub type Section = (Option<String>, Vec<(Instance, Placement)>);

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
    /// Every instance you named, grouped or moved, in the order you put them.
    #[serde(default)]
    layout: Vec<Placement>,
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

    /// `instances` in the order you put them, those you never moved last, each with its placement.
    pub fn arrange(&self, instances: Vec<Instance>) -> Vec<(Instance, Placement)> {
        let position = |id: &str| {
            self.layout
                .iter()
                .position(|placement| placement.id == id)
                .unwrap_or(usize::MAX)
        };
        let mut arranged: Vec<(Instance, Placement)> = instances
            .into_iter()
            .map(|instance| {
                let id = instance.id();
                let placement = self
                    .layout
                    .iter()
                    .find(|placement| placement.id == id)
                    .cloned()
                    .unwrap_or(Placement {
                        id,
                        ..Placement::default()
                    });
                (instance, placement)
            })
            .collect();
        arranged.sort_by_key(|(_, placement)| position(&placement.id));
        arranged
    }

    /// The arranged instances split into their groups, in the order each group first appears.
    pub fn sections(arranged: Vec<(Instance, Placement)>) -> Vec<Section> {
        let mut sections: Vec<Section> = Vec::new();
        for entry in arranged {
            let group = entry.1.group.clone();
            match sections.iter_mut().find(|(name, _)| *name == group) {
                Some((_, entries)) => entries.push(entry),
                None => sections.push((group, vec![entry])),
            }
        }
        sections
    }

    /// Changes an instance's placement, writing down the current order first so it stays put.
    pub fn place(
        &mut self,
        arranged: &[(Instance, Placement)],
        id: &str,
        change: impl FnOnce(&mut Placement),
    ) {
        self.layout = arranged
            .iter()
            .map(|(_, placement)| placement.clone())
            .collect();
        if let Some(placement) = self.layout.iter_mut().find(|p| p.id == id) {
            change(placement);
        }
    }

    /// Moves an instance one place up (`-1`) or down (`1`) among `arranged`.
    pub fn shift(&mut self, arranged: &[(Instance, Placement)], id: &str, offset: isize) {
        self.layout = arranged
            .iter()
            .map(|(_, placement)| placement.clone())
            .collect();
        let Some(from) = self.layout.iter().position(|p| p.id == id) else {
            return;
        };
        let to = from as isize + offset;
        if (0..self.layout.len() as isize).contains(&to) {
            self.layout.swap(from, to as usize);
        }
    }

    /// Forgets the name, group and place of an instance you removed.
    pub fn forget_placement(&mut self, id: &str) {
        self.layout.retain(|placement| placement.id != id);
    }

    pub fn remove(&mut self, server: &Server) {
        self.forget_placement(&server.id);
        self.urls.retain(|url| *url != server.id);
        self.keys.remove(&server.id);
        if self.active.as_deref() == Some(&server.id) {
            self.active = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instances_keep_the_order_and_groups_you_give_them() {
        let local = Instance::Machine(Connection::Local);
        let devbox = Instance::Machine(Connection::Ssh {
            host: "devbox".into(),
        });
        let remote = Instance::Added(Server::remote("https://a.example.com"));
        let all = || vec![local.clone(), devbox.clone(), remote.clone()];
        let mut servers = Servers::default();
        let arranged = servers.arrange(all());
        servers.shift(&arranged, &remote.id(), -1);
        let arranged = servers.arrange(all());
        servers.place(&arranged, &devbox.id(), |p| {
            p.group = Some("Clients".into());
            p.name = Some("Client box".into());
        });
        let arranged = servers.arrange(all());
        let order: Vec<String> = arranged.iter().map(|(i, _)| i.id()).collect();
        assert_eq!(order, [local.id(), remote.id(), devbox.id()]);
        let sections = Servers::sections(arranged);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[1].0.as_deref(), Some("Clients"));
        assert_eq!(sections[1].1[0].1.name.as_deref(), Some("Client box"));
        let text = serde_json::to_string(&servers).unwrap();
        assert_eq!(serde_json::from_str::<Servers>(&text).unwrap(), servers);
    }

    #[test]
    fn servers_are_added_once_and_keys_follow_their_id() {
        let mut servers = Servers::default();
        let remote = servers.add("https://ricochet.example.com");
        servers.add("https://ricochet.example.com");
        servers.set_key(&remote, Some("rico_1".into()));
        let local = Server::managed(&Connection::Local, "http://localhost:6188".into());
        assert_eq!(servers.urls, ["https://ricochet.example.com"]);
        assert_eq!(remote.name, "ricochet.example.com");
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
