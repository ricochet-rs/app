use super::choice::choice;
use super::section::{Field, SettingsSection, TableRead, TableWrite};
use toml_edit::{Table, TableLike};

choice! {
    Encryption {
        StartTls => ("starttls", "STARTTLS"),
        Tls => ("tls", "TLS"),
        None => ("none", "None"),
    }
}

/// `[smtp]`, the mail server ricochet sends email through. Without it, ricochet sends no email.
#[derive(Clone, Debug, PartialEq)]
pub struct EmailSettings {
    pub enabled: bool,
    pub host: String,
    pub port: String,
    pub encryption: Encryption,
    pub username: String,
    pub password: String,
    pub from: String,
    pub reply_to: String,
}

impl Default for EmailSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: String::new(),
            encryption: Encryption::StartTls,
            username: String::new(),
            password: String::new(),
            from: String::new(),
            reply_to: String::new(),
        }
    }
}

impl SettingsSection for EmailSettings {
    fn from_table(table: &dyn TableLike) -> Self {
        let Some(smtp) = table.child("smtp") else {
            return Self::default();
        };
        Self {
            enabled: true,
            host: smtp.string("host").unwrap_or_default(),
            port: smtp.int("port").map(|n| n.to_string()).unwrap_or_default(),
            encryption: smtp
                .string("tls")
                .and_then(|raw| Encryption::parse(&raw))
                .unwrap_or(Encryption::StartTls),
            username: smtp.string("username").unwrap_or_default(),
            password: smtp.string("password").unwrap_or_default(),
            from: smtp.string("from").unwrap_or_default(),
            reply_to: smtp.string("reply_to").unwrap_or_default(),
        }
    }

    fn apply(&self, table: &mut Table) {
        if !self.enabled {
            table.remove("smtp");
            return;
        }
        let smtp = table.section("smtp");
        smtp.set("host", Some(self.host.trimmed()));
        smtp.set("port", self.port.whole_number());
        smtp.set(
            "tls",
            (self.encryption != Encryption::StartTls).then(|| self.encryption.raw()),
        );
        smtp.set("username", self.username.non_empty());
        smtp.set("password", self.password.non_empty());
        smtp.set("from", Some(self.from.trimmed()));
        smtp.set("reply_to", self.reply_to.non_empty());
    }

    fn problem(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        if self.host.non_empty().is_none() {
            return Some("Enter the mail server host.".into());
        }
        if let Some(port) = self.port.non_empty()
            && port.parse::<u16>().map_or(true, |port| port == 0)
        {
            return Some("Mail server port must be between 1 and 65535.".into());
        }
        if self.username.non_empty().is_none() != self.password.non_empty().is_none() {
            return Some("Enter both a username and a password, or neither.".into());
        }
        if !self.from.contains('@') {
            return Some("Enter a From address, such as ricochet <ricochet@example.com>.".into());
        }
        if self.reply_to.non_empty().is_some() && !self.reply_to.contains('@') {
            return Some("Reply-to must be an email address.".into());
        }
        None
    }
}
