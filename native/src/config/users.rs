use super::choice::choice;
use super::section::{SettingsSection, TableRead, TableWrite};
use toml_edit::{Table, TableLike};

choice! {
    Role {
        Admin => ("admin", "Admin"),
        Developer => ("developer", "Developer"),
        Consumer => ("consumer", "Consumer"),
    }
}

/// The rest of `[auth]`: the role new people get, and what they can do without signing in.
#[derive(Clone, Debug, PartialEq)]
pub struct UserSettings {
    pub default_role: Role,
    pub require_sign_in: bool,
    pub allow_avatar_upload: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            default_role: Role::Developer,
            require_sign_in: false,
            allow_avatar_upload: true,
        }
    }
}

impl SettingsSection for UserSettings {
    fn from_table(table: &dyn TableLike) -> Self {
        let auth = table.child("auth");
        Self {
            default_role: auth
                .and_then(|auth| auth.string("default_role"))
                .and_then(|raw| Role::parse(&raw))
                .unwrap_or(Role::Developer),
            require_sign_in: auth
                .and_then(|auth| auth.boolean("require_authentication"))
                .unwrap_or(false),
            allow_avatar_upload: auth
                .and_then(|auth| auth.boolean("allow_avatar_upload"))
                .unwrap_or(true),
        }
    }

    /// Writes `default_role` even as Developer, because ricochet warns at every sign-in when it is absent.
    fn apply(&self, table: &mut Table) {
        let auth = table.section("auth");
        auth.set("default_role", Some(self.default_role.raw()));
        auth.set(
            "require_authentication",
            self.require_sign_in.then_some(true),
        );
        auth.set(
            "allow_avatar_upload",
            (!self.allow_avatar_upload).then_some(false),
        );
    }

    fn problem(&self) -> Option<String> {
        None
    }
}
