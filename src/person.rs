use crate::backend::Backend;
use crate::config::Role;
use crate::error::{AppError, Result};
use serde::{Deserialize, Deserializer};

/// Someone who signed in to ricochet, as `ricochet-server user list` reports them.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Person {
    pub id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    #[serde(deserialize_with = "role")]
    pub role: Role,
}

fn role<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Role, D::Error> {
    let raw = String::deserialize(deserializer)?;
    Role::parse(&raw).ok_or_else(|| serde::de::Error::custom(format!("unknown role `{raw}`")))
}

impl Person {
    pub fn name(&self) -> &str {
        self.display_name
            .as_deref()
            .or(self.email.as_deref())
            .unwrap_or(&self.id)
    }

    /// Everyone who signed in to ricochet.
    pub fn all(backend: &dyn Backend) -> Result<Vec<Self>> {
        let output = backend.users(&["list", "--format", "json"])?;
        parse_list(&output)
    }

    /// Gives this person `role`, which ricochet applies from their next request without a restart.
    pub fn set_role(&self, backend: &dyn Backend, role: Role) -> Result<()> {
        backend
            .users(&["set-role", &self.id, role.raw()])
            .map(|_| ())
    }
}

fn parse_list(output: &str) -> Result<Vec<Person>> {
    // Ricochet prints `No users found.` instead of `[]` when nobody has signed in.
    if !output.starts_with('[') {
        return Ok(Vec::new());
    }
    #[derive(Deserialize)]
    struct Entry {
        user: Person,
    }
    let entries: Vec<Entry> =
        serde_json::from_str(output).map_err(|error| AppError::UserCommand(error.to_string()))?;
    Ok(entries.into_iter().map(|entry| entry.user).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nobody_signed_in_yet() {
        assert!(parse_list("No users found.").unwrap().is_empty());
    }

    #[test]
    fn people_come_from_the_user_field() {
        let people = parse_list(
            r#"[{"user":{"id":"u1","email":"a@example.com","display_name":null,"avatar_url":null,"role":"admin"}}]"#,
        )
        .unwrap();
        assert_eq!(people[0].name(), "a@example.com");
        assert_eq!(people[0].role, Role::Admin);
    }
}
