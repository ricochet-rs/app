use super::choice::choice;
use super::section::{Field, SettingsSection, TableRead, TableWrite};
use toml_edit::{Table, TableLike};

choice! {
    Provider {
        Oidc => ("oidc", "OpenID Connect"),
        Entra => ("entra", "Microsoft Entra ID"),
    }
}

impl Provider {
    pub fn issuer_prompt(self) -> &'static str {
        match self {
            Self::Oidc => "https://auth.example.com",
            Self::Entra => "https://login.microsoftonline.com/<tenant-id>/v2.0",
        }
    }
}

/// `[auth.oidc]`: ricochet managed auth, or your own identity provider and the claims ricochet reads from it.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthenticationSettings {
    /// Whether you selected **I agree** on managed auth, which shares your name and email address with ricochet.
    pub managed: bool,
    pub provider: Provider,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub display_name_claim: String,
    pub groups_claim: String,
    pub sync_interval: String,
}

impl Default for AuthenticationSettings {
    fn default() -> Self {
        Self {
            managed: false,
            provider: Provider::Oidc,
            issuer_url: String::new(),
            client_id: String::new(),
            client_secret: String::new(),
            display_name_claim: String::new(),
            groups_claim: String::new(),
            sync_interval: String::new(),
        }
    }
}

impl AuthenticationSettings {
    /// The only port the managed Zitadel app accepts in its redirect URL.
    pub const MANAGED_PORT: &'static str = "6188";
    /// The policy you agree to when you sign in through managed auth.
    pub const PRIVACY_POLICY: &'static str = "https://ricochet.rs/legal/privacy/";

    /// The Zitadel app on idp.ricochet.rs that managed auth signs in with.
    pub fn managed_provider() -> Self {
        Self {
            issuer_url: "https://idp.ricochet.rs".into(),
            client_id: "398284990531538194".into(),
            ..Self::default()
        }
    }

    /// The issuer ricochet signs people in with.
    pub fn issuer(&self) -> String {
        if self.managed {
            Self::managed_provider().issuer_url
        } else {
            self.issuer_url.trimmed().to_owned()
        }
    }
}

impl SettingsSection for AuthenticationSettings {
    fn from_table(table: &dyn TableLike) -> Self {
        let Some(oidc) = table.child("auth").and_then(|auth| auth.child("oidc")) else {
            return Self::default();
        };
        let saved = Self {
            managed: false,
            provider: oidc
                .string("provider")
                .and_then(|raw| Provider::parse(&raw))
                .unwrap_or(Provider::Oidc),
            issuer_url: oidc.string("issuer_url").unwrap_or_default(),
            client_id: oidc.string("client_id").unwrap_or_default(),
            client_secret: oidc.string("client_secret").unwrap_or_default(),
            display_name_claim: oidc.string("display_name_claim").unwrap_or_default(),
            groups_claim: oidc.string("groups_claim").unwrap_or_default(),
            sync_interval: oidc
                .int("sync_interval")
                .map(|n| n.to_string())
                .unwrap_or_default(),
        };
        if saved == Self::managed_provider() {
            Self {
                managed: true,
                ..Self::default()
            }
        } else {
            saved
        }
    }

    fn apply(&self, table: &mut Table) {
        let auth = table.section("auth");
        auth.remove("saml");
        let oidc = auth.section("oidc");
        let written = if self.managed {
            Self::managed_provider()
        } else {
            self.clone()
        };
        oidc.set("provider", Some(written.provider.raw()));
        oidc.set("issuer_url", Some(written.issuer_url.trimmed()));
        oidc.set("client_id", Some(written.client_id.trimmed()));
        oidc.set("client_secret", written.client_secret.non_empty());
        oidc.set("display_name_claim", written.display_name_claim.non_empty());
        oidc.set("groups_claim", written.groups_claim.non_empty());
        oidc.set("sync_interval", written.sync_interval.whole_number());
    }

    fn problem(&self) -> Option<String> {
        if self.managed {
            return None;
        }
        if self.issuer_url.non_empty().is_none() && self.client_id.non_empty().is_none() {
            return Some("Select I agree on Managed auth, or configure your own IdP.".into());
        }
        if !self.issuer_url.is_http_url() {
            return Some("Enter the issuer URL, starting with https://.".into());
        }
        if self.client_id.non_empty().is_none() {
            return Some("Enter the client ID.".into());
        }
        if self.provider == Provider::Entra && self.client_secret.non_empty().is_none() {
            return Some(
                "Enter the client secret. Ricochet needs it to read groups and photos from Microsoft Entra ID."
                    .into(),
            );
        }
        if !self.sync_interval.is_empty_or_positive() {
            return Some("Enter the group sync interval as a whole number of minutes.".into());
        }
        None
    }
}
