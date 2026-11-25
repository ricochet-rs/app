//! The ricochet servers whose web UI the app shows, and how you arranged them.

use crate::connections::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A ricochet server the dashboard can show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    /// What the store files the server's name, group and place under.
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
    /// The server shown when you last quit.
    #[serde(default)]
    pub active: Option<String>,
    /// Every instance you named, grouped or moved, in the order you put them.
    #[serde(default)]
    layout: Vec<Placement>,
    /// Every group you made or moved an instance into, so a group stays when it is empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    groups: Vec<String>,
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

    /// Writes the store readable only by you, since it lists the servers you use.
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

    /// Adds a server by address and returns it, or the one already saved at that address.
    pub fn add(&mut self, url: &str) -> Server {
        if !self.urls.iter().any(|saved| saved == url) {
            self.urls.push(url.to_owned());
        }
        Server::remote(url)
    }

    /// `instances` in the order you put them, those you never moved last, each with its placement.
    /// Instances in one group follow each other, as the server menu lists them.
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
        Self::sections(arranged)
            .into_iter()
            .flat_map(|(_, entries)| entries)
            .collect()
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

    /// Writes down the order of `arranged`, so a change leaves the rest in place.
    fn write_down(&mut self, arranged: &[(Instance, Placement)]) {
        self.layout = arranged
            .iter()
            .map(|(_, placement)| placement.clone())
            .collect();
    }

    /// Changes an instance's placement, writing down the current order first so it stays put.
    pub fn place(
        &mut self,
        arranged: &[(Instance, Placement)],
        id: &str,
        change: impl FnOnce(&mut Placement),
    ) {
        self.write_down(arranged);
        if let Some(placement) = self.layout.iter_mut().find(|p| p.id == id) {
            change(placement);
        }
    }

    /// Moves an instance into `group`, just before the instance `before`, or after the group's last instance.
    pub fn move_to(
        &mut self,
        arranged: &[(Instance, Placement)],
        id: &str,
        group: Option<String>,
        before: Option<&str>,
    ) {
        self.write_down(arranged);
        let Some(from) = self.layout.iter().position(|p| p.id == id) else {
            return;
        };
        let mut placement = self.layout.remove(from);
        placement.group = group;
        if let Some(group) = &placement.group
            && !self.groups.contains(group)
        {
            self.groups.push(group.clone());
        }
        let at = before
            .filter(|before| *before != id)
            .and_then(|before| self.layout.iter().position(|p| p.id == before))
            .or_else(|| {
                self.layout
                    .iter()
                    .rposition(|p| p.group == placement.group)
                    .map(|last| last + 1)
            })
            .unwrap_or(self.layout.len());
        self.layout.insert(at, placement);
    }

    /// Renames the group `from` to `to` for every instance in it.
    pub fn rename_group(&mut self, arranged: &[(Instance, Placement)], from: &str, to: &str) {
        self.write_down(arranged);
        for placement in &mut self.layout {
            if placement.group.as_deref() == Some(from) {
                placement.group = Some(to.to_owned());
            }
        }
        for group in &mut self.groups {
            if group == from {
                *group = to.to_owned();
            }
        }
    }

    /// Adds an empty group with the first free name of "New group", "New group 2" and so on, and returns its name.
    pub fn add_group(&mut self, arranged: &[(Instance, Placement)]) -> String {
        let taken = |name: &str| {
            self.groups.iter().any(|group| group == name)
                || arranged
                    .iter()
                    .any(|(_, placement)| placement.group.as_deref() == Some(name))
        };
        let name = std::iter::once("New group".to_owned())
            .chain((2..).map(|n| format!("New group {n}")))
            .find(|name| !taken(name))
            .unwrap_or_default();
        self.groups.push(name.clone());
        name
    }

    /// Forgets a group, which only an empty group should be.
    pub fn remove_group(&mut self, name: &str) {
        self.groups.retain(|group| group != name);
    }

    /// The groups you made that hold none of `arranged`, in the order you made them.
    pub fn empty_groups(&self, arranged: &[(Instance, Placement)]) -> Vec<String> {
        self.groups
            .iter()
            .filter(|group| {
                !arranged
                    .iter()
                    .any(|(_, placement)| placement.group.as_ref() == Some(*group))
            })
            .cloned()
            .collect()
    }

    /// Forgets the name, group and place of an instance you removed.
    pub fn forget_placement(&mut self, id: &str) {
        self.layout.retain(|placement| placement.id != id);
    }

    pub fn remove(&mut self, server: &Server) {
        self.forget_placement(&server.id);
        self.urls.retain(|url| *url != server.id);
        if self.active.as_deref() == Some(&server.id) {
            self.active = None;
        }
    }
}

/// The address of a server as typed, with `https://` added when no scheme is given, or `None` when it isn't one.
pub fn normalize_url(text: &str) -> Option<String> {
    let text = text.trim();
    let (scheme, rest) = text.split_once("://").unwrap_or(("https", text));
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let rest = rest.trim_end_matches('/');
    let host = rest.split(['/', ':', '?', '#']).next()?;
    let valid = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']'));
    valid.then(|| format!("{scheme}://{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_gain_https_and_lose_trailing_slashes() {
        assert_eq!(
            normalize_url(" ricochet.example.com/ ").as_deref(),
            Some("https://ricochet.example.com")
        );
        assert_eq!(
            normalize_url("http://localhost:6188").as_deref(),
            Some("http://localhost:6188")
        );
        assert_eq!(normalize_url("ftp://example.com"), None);
        assert_eq!(normalize_url("https://"), None);
        assert_eq!(normalize_url("not a host"), None);
    }

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
        servers.move_to(&arranged, &remote.id(), None, Some(&devbox.id()));
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
    fn instances_move_into_a_group_before_another_or_at_its_end() {
        let local = Instance::Machine(Connection::Local);
        let devbox = Instance::Machine(Connection::Ssh {
            host: "devbox".into(),
        });
        let remote = Instance::Added(Server::remote("https://a.example.com"));
        let all = || vec![local.clone(), devbox.clone(), remote.clone()];
        let order = |servers: &Servers| -> Vec<String> {
            servers.arrange(all()).iter().map(|(i, _)| i.id()).collect()
        };
        let empty = |servers: &Servers| servers.empty_groups(&servers.arrange(all()));
        let move_to =
            |servers: &mut Servers, id: String, group: Option<&str>, before: Option<String>| {
                let arranged = servers.arrange(all());
                servers.move_to(&arranged, &id, group.map(str::to_owned), before.as_deref());
            };
        let mut servers = Servers::default();
        let group = servers.add_group(&[]);
        assert_eq!(group, "New group");
        assert_eq!(empty(&servers), [group.as_str()]);
        move_to(&mut servers, local.id(), Some(&group), None);
        move_to(&mut servers, remote.id(), Some(&group), None);
        assert_eq!(order(&servers), [devbox.id(), local.id(), remote.id()]);
        assert!(empty(&servers).is_empty());
        move_to(&mut servers, remote.id(), Some(&group), Some(local.id()));
        assert_eq!(order(&servers), [devbox.id(), remote.id(), local.id()]);
        move_to(&mut servers, remote.id(), None, Some(devbox.id()));
        move_to(&mut servers, local.id(), None, None);
        assert_eq!(order(&servers), [remote.id(), devbox.id(), local.id()]);
        assert_eq!(empty(&servers), [group.as_str()]);
        assert_eq!(servers.add_group(&[]), "New group 2");
        servers.remove_group(&group);
        assert_eq!(empty(&servers), ["New group 2"]);
    }

    #[test]
    fn renaming_a_group_renames_it_for_each_instance() {
        let local = Instance::Machine(Connection::Local);
        let remote = Instance::Added(Server::remote("https://a.example.com"));
        let all = || vec![local.clone(), remote.clone()];
        let mut servers = Servers::default();
        for instance in [&local, &remote] {
            let arranged = servers.arrange(all());
            servers.place(&arranged, &instance.id(), |p| {
                p.group = Some("Clients".into())
            });
        }
        let arranged = servers.arrange(all());
        servers.rename_group(&arranged, "Clients", "Customers");
        let sections = Servers::sections(servers.arrange(all()));
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0.as_deref(), Some("Customers"));
        assert_eq!(sections[0].1.len(), 2);
    }

    #[test]
    fn servers_are_added_once() {
        let mut servers = Servers::default();
        let remote = servers.add("https://ricochet.example.com");
        servers.add("https://ricochet.example.com");
        assert_eq!(servers.urls, ["https://ricochet.example.com"]);
        assert_eq!(remote.name, "ricochet.example.com");
    }

    #[test]
    fn removing_a_server_forgets_it() {
        let mut servers = Servers::default();
        let remote = servers.add("https://a.example.com");
        servers.active = Some(remote.id.clone());
        servers.remove(&remote);
        assert_eq!(servers, Servers::default());
    }
}
