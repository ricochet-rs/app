//! Every pane's answers, stored in `ricochet-config.toml`.

mod audit;
mod authentication;
mod choice;
mod deployments;
mod email;
mod logging;
mod packages;
mod proxy;
pub mod section;
mod server;
mod tasks;
mod telemetry;
mod users;

pub use audit::AuditLogSettings;
pub use authentication::{AuthenticationSettings, Provider};
pub use deployments::DeploymentSettings;
pub use email::{EmailSettings, Encryption};
pub use logging::{LogLevel, LogRotation, LoggingSettings};
pub use packages::{PackageIndex, PackageSettings, RRepository};
pub use proxy::{Proxy, ProxyMode, ProxySettings, split_hosts};
pub use server::ServerSettings;
pub use tasks::TaskSettings;
pub use telemetry::TelemetrySettings;
pub use users::{Role, UserSettings};

use crate::error::{AppError, Result};
use crate::ports::NetworkPorts;
use section::{SettingsSection, TableWrite};
use std::path::Path;
use toml_edit::{DocumentMut, TableLike, value};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerConfig {
    pub authentication: AuthenticationSettings,
    pub server: ServerSettings,
    pub users: UserSettings,
    pub email: EmailSettings,
    pub packages: PackageSettings,
    pub deployments: DeploymentSettings,
    pub tasks: TaskSettings,
    pub logging: LoggingSettings,
    pub audit_log: AuditLogSettings,
    pub telemetry: TelemetrySettings,
    pub proxy: ProxySettings,
}

impl ServerConfig {
    pub fn local_url(&self) -> String {
        format!(
            "http://localhost:{}",
            self.server.proxy_port().unwrap_or(6188)
        )
    }

    pub fn redirect_url(&self) -> String {
        self.local_url() + "/oauth/callback"
    }

    /// Every pane's answers as saved in `table`, the parsed `ricochet-config.toml`.
    pub fn from_table(table: &dyn TableLike) -> Self {
        Self {
            authentication: AuthenticationSettings::from_table(table),
            server: ServerSettings::from_table(table),
            users: UserSettings::from_table(table),
            email: EmailSettings::from_table(table),
            packages: PackageSettings::from_table(table),
            deployments: DeploymentSettings::from_table(table),
            tasks: TaskSettings::from_table(table),
            logging: LoggingSettings::from_table(table),
            audit_log: AuditLogSettings::from_table(table),
            telemetry: TelemetrySettings::from_table(table),
            proxy: ProxySettings::from_table(table),
        }
    }

    /// The config text with every pane written into `existing`, keeping each setting no pane manages.
    /// `is_free` says whether a port could be bound on the computer ricochet runs on.
    pub fn render(
        &self,
        existing: &str,
        proxy_port: u16,
        home: &Path,
        is_free: &dyn Fn(u16) -> bool,
    ) -> Result<String> {
        let mut document = existing.parse::<DocumentMut>().map_err(|error| {
            let line = error
                .span()
                .map_or(1, |span| existing[..span.start].matches('\n').count() + 1);
            AppError::InvalidConfig {
                line,
                reason: error.message().to_owned(),
            }
        })?;
        let table = document.as_table_mut();
        table.insert("home", value(home.to_string_lossy().as_ref()));
        table.insert("url", value(self.local_url()));
        self.authentication.apply(table);
        self.server.apply(table);
        self.users.apply(table);
        self.email.apply(table);
        self.packages.apply(table);
        self.deployments.apply(table);
        self.tasks.apply(table);
        self.logging.apply(table);
        self.audit_log.apply(table);
        self.telemetry.apply(table);
        self.proxy.apply(table);

        let ports = NetworkPorts::new(
            proxy_port,
            table.get("network").and_then(|n| n.as_table_like()),
            is_free,
        );
        ports.write(table.section("network"));
        Ok(document.to_string())
    }

    /// Config text with each internal port that another app now holds, or that is an old default, moved to a free one.
    /// Returns `None` when every port can stay or `existing` names no ports, and must run only while ricochet is stopped.
    pub fn move_taken_ports(existing: &str, is_free: &dyn Fn(u16) -> bool) -> Option<String> {
        let mut document = existing.parse::<DocumentMut>().ok()?;
        let network = document.get_mut("network")?.as_table_like_mut()?;
        let saved = NetworkPorts::saved(&*network)?;
        let ports = NetworkPorts::rechecked(saved.proxy, Some(&*network), is_free);
        if ports == saved {
            return None;
        }
        ports.write(network);
        Some(document.to_string())
    }

    /// The answers in config text read from a computer, or `None` when it does not parse.
    pub fn parse(text: &str) -> Option<Self> {
        let document = text.parse::<DocumentMut>().ok()?;
        Some(Self::from_table(document.as_table()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use section::TableRead;

    fn save(config: &ServerConfig, existing: &str) -> DocumentMut {
        let text = config
            .render(
                existing,
                config.server.proxy_port().unwrap_or(6188),
                Path::new("/home/someone/.local/share/ricochet"),
                &|_| true,
            )
            .unwrap();
        text.parse().unwrap()
    }

    fn keys(table: &dyn TableLike) -> Vec<String> {
        let mut keys: Vec<String> = table.iter().map(|(key, _)| key.to_owned()).collect();
        keys.sort();
        keys
    }

    #[test]
    fn saving_keeps_settings_the_form_does_not_manage() {
        let config = ServerConfig {
            authentication: AuthenticationSettings {
                issuer_url: "https://auth.example.com".into(),
                client_id: "ricochet".into(),
                client_secret: r#"a "quoted" \secret"#.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let saved = save(
            &config,
            r#"
[database]
url = "sqlite://elsewhere.db"

[auth.oidc]
issuer_url = "https://old.example.com"

[auth.saml]
entity_id = "https://old.example.com"
"#,
        );
        let table = saved.as_table() as &dyn TableLike;

        assert_eq!(
            table.child("database").unwrap().string("url").unwrap(),
            "sqlite://elsewhere.db"
        );
        assert!(table.child("auth").unwrap().child("saml").is_none());
        assert_eq!(ServerConfig::from_table(table), config);
    }

    #[test]
    fn every_pane_round_trips() {
        let config = ServerConfig {
            authentication: AuthenticationSettings {
                managed: false,
                provider: Provider::Entra,
                issuer_url: "https://login.microsoftonline.com/tenant/v2.0".into(),
                client_id: "id".into(),
                client_secret: "secret".into(),
                display_name_claim: "name".into(),
                groups_claim: "groups".into(),
                sync_interval: "30".into(),
            },
            server: ServerSettings {
                port: "7000".into(),
                license_key: "KEY".into(),
            },
            users: UserSettings {
                default_role: Role::Consumer,
                require_sign_in: true,
                allow_avatar_upload: false,
            },
            email: EmailSettings {
                enabled: true,
                host: "smtp.example.com".into(),
                port: "2525".into(),
                encryption: Encryption::Tls,
                username: "user".into(),
                password: "password".into(),
                from: "ricochet <ricochet@example.com>".into(),
                reply_to: "help@example.com".into(),
            },
            packages: PackageSettings {
                r: vec![
                    RRepository {
                        name: "CRAN".into(),
                        url: "https://cloud.r-project.org".into(),
                    },
                    RRepository {
                        name: "P3M".into(),
                        url: "https://p3m.dev/cran/latest".into(),
                    },
                ],
                julia: vec![PackageIndex {
                    url: "https://github.com/JuliaRegistries/General.git".into(),
                }],
                python: vec![PackageIndex {
                    url: "https://pypi.org/simple".into(),
                }],
                allow_overrides: false,
            },
            deployments: DeploymentSettings {
                max_deployments: "3".into(),
                max_age_days: "30".into(),
                max_bundle_size: "1GB".into(),
            },
            tasks: TaskSettings {
                max_duration: "1.5".into(),
                restore_timeout: "20".into(),
                restore_concurrency: "2".into(),
            },
            logging: LoggingSettings {
                level: LogLevel::Debug,
                rotation: LogRotation::Hourly,
                compress: false,
                max_files: "10".into(),
                max_age_days: "14".into(),
            },
            audit_log: AuditLogSettings {
                keep_forever: true,
                retention_days: String::new(),
                write_file: true,
            },
            telemetry: TelemetrySettings {
                enabled: true,
                endpoint: "http://localhost:4317".into(),
            },
            proxy: ProxySettings {
                mode: ProxyMode::Manual,
                url: "http://proxy.example.com:8080".into(),
                bypass: ".corp.example.com".into(),
            },
        };

        let saved = save(&config, "");
        let table = saved.as_table() as &dyn TableLike;

        assert_eq!(table.child("tasks").unwrap().int("max_duration"), Some(90));
        assert_eq!(
            table
                .child("audit")
                .unwrap()
                .string("retention_days")
                .unwrap(),
            "forever"
        );
        assert_eq!(ServerConfig::from_table(table), config);
    }

    #[test]
    fn managed_auth_writes_only_the_required_keys() {
        let config = ServerConfig {
            authentication: AuthenticationSettings {
                managed: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let saved = save(&config, "[otel]\nendpoint = \"http://localhost:4317\"\n");
        let table = saved.as_table() as &dyn TableLike;

        assert_eq!(keys(table), ["auth", "home", "logs", "network", "url"]);
        let auth = table.child("auth").unwrap();
        assert_eq!(keys(auth), ["default_role", "oidc"]);
        let oidc = auth.child("oidc").unwrap();
        assert_eq!(keys(oidc), ["client_id", "issuer_url", "provider"]);
        assert_eq!(
            oidc.string("issuer_url").unwrap(),
            "https://idp.ricochet.rs"
        );
        assert_eq!(oidc.string("client_id").unwrap(), "398284990531538194");
        assert_eq!(keys(table.child("logs").unwrap()), ["file_logging"]);
        assert_eq!(ServerConfig::from_table(table), config);
        assert!(
            !ServerConfig::from_table(&toml_edit::Table::new())
                .authentication
                .managed
        );
    }

    #[test]
    fn saved_internal_ports_are_kept() {
        let network: DocumentMut =
            "api_port = 4321\nreload_port = 4322\nmetrics = \"127.0.0.1:4323\""
                .parse()
                .unwrap();
        let ports = NetworkPorts::new(6188, Some(network.as_table()), &|_| true);
        assert_eq!([ports.api, ports.reload, ports.metrics], [4321, 4322, 4323]);
    }

    #[test]
    fn a_port_another_app_took_moves_and_the_rest_stay() {
        let existing = "home = \"/h\"\n\n[network]\nproxy_port = 6188\napi_port = 50000\nreload_port = 50001\nmetrics = \"127.0.0.1:50002\"\n";
        assert_eq!(ServerConfig::move_taken_ports(existing, &|_| true), None);
        let moved = ServerConfig::move_taken_ports(existing, &|port| port != 50001).unwrap();
        let document: DocumentMut = moved.parse().unwrap();
        let table: &dyn TableLike = document.as_table();
        let network = table.child("network").unwrap();
        assert_eq!(network.int("proxy_port"), Some(6188));
        assert_eq!(network.int("api_port"), Some(50000));
        assert_ne!(network.int("reload_port"), Some(50001));
        assert_eq!(
            network.string("metrics").as_deref(),
            Some("127.0.0.1:50002")
        );
        assert_eq!(table.string("home").as_deref(), Some("/h"));
    }

    #[test]
    fn switching_to_managed_auth_removes_your_providers_keys() {
        let config = ServerConfig {
            authentication: AuthenticationSettings {
                managed: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let saved = save(
            &config,
            r#"
[auth.oidc]
issuer_url = "https://auth.example.com"
client_id = "ricochet"
client_secret = "secret"
groups_claim = "groups"
sync_interval = 30
"#,
        );
        let oidc = saved.as_table().child_of("auth", "oidc");
        assert_eq!(keys(oidc), ["client_id", "issuer_url", "provider"]);
    }

    #[test]
    fn a_secret_means_your_own_provider_even_at_the_managed_issuer() {
        let table: DocumentMut = r#"
[auth.oidc]
issuer_url = "https://idp.ricochet.rs"
client_id = "398284990531538194"
client_secret = "secret"
"#
        .parse()
        .unwrap();
        assert!(!AuthenticationSettings::from_table(table.as_table()).managed);
    }

    #[test]
    fn r_repositories_reach_ricochet_sorted_by_name() {
        let config = ServerConfig {
            packages: PackageSettings {
                r: vec![
                    RRepository {
                        name: "P3M".into(),
                        url: "https://p3m.dev".into(),
                    },
                    RRepository {
                        name: "CRAN".into(),
                        url: "https://cloud.r-project.org".into(),
                    },
                ],
                ..Default::default()
            },
            ..Default::default()
        };
        let saved = save(&config, "").to_string();
        assert!(saved.find("CRAN").unwrap() < saved.find("P3M").unwrap());
    }

    #[test]
    fn the_default_r_repository_shows_but_is_never_written() {
        let config = ServerConfig::default();
        assert_eq!(config.packages.r, PackageSettings::default_r());
        let saved = save(&config, "").to_string();
        assert!(!saved.contains("[repositories.r]"), "{saved}");
        assert!(!saved.contains("cran.rpkgs.com"), "{saved}");
    }

    #[test]
    fn a_broken_config_names_the_line() {
        let error = ServerConfig::default()
            .render("ok = 1\nbroken = = 2\n", 6188, Path::new("/home"), &|_| {
                true
            })
            .unwrap_err();
        assert!(
            matches!(error, AppError::InvalidConfig { line: 2, .. }),
            "{error:?}"
        );
    }

    trait ChildOf {
        fn child_of(&self, parent: &str, child: &str) -> &dyn TableLike;
    }

    impl ChildOf for toml_edit::Table {
        fn child_of(&self, parent: &str, child: &str) -> &dyn TableLike {
            (self as &dyn TableLike)
                .child(parent)
                .unwrap()
                .child(child)
                .unwrap()
        }
    }
}
