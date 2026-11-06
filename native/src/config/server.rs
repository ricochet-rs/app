use super::section::{Field, SettingsSection, TableRead, TableWrite};
use toml_edit::{Table, TableLike};

/// The port ricochet serves on and the license it runs under.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerSettings {
    pub port: String,
    pub license_key: String,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            port: "6188".into(),
            license_key: String::new(),
        }
    }
}

impl ServerSettings {
    pub fn proxy_port(&self) -> Option<u16> {
        self.port
            .trimmed()
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)
    }
}

impl SettingsSection for ServerSettings {
    fn from_table(table: &dyn TableLike) -> Self {
        let mut settings = Self::default();
        if let Some(saved) = table
            .child("network")
            .and_then(|network| network.int("proxy_port"))
        {
            settings.port = saved.to_string();
        }
        settings.license_key = table
            .child("license")
            .and_then(|license| license.string("key"))
            .unwrap_or_default();
        settings
    }

    fn apply(&self, table: &mut Table) {
        table
            .section("license")
            .set("key", self.license_key.non_empty());
        table.remove_if_empty("license");
    }

    fn problem(&self) -> Option<String> {
        self.proxy_port()
            .is_none()
            .then(|| "Enter a port between 1 and 65535.".to_owned())
    }
}
