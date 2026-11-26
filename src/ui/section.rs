use crate::config::ServerConfig;
use crate::config::section::SettingsSection;

/// A pane in the setup window, one per settings section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupSection {
    Authentication,
    Server,
    Users,
    Email,
    Packages,
    Deployments,
    Tasks,
    Logging,
    AuditLog,
    Telemetry,
    Proxy,
    DangerZone,
}

impl SetupSection {
    pub const ALL: [Self; 12] = [
        Self::Authentication,
        Self::Server,
        Self::Users,
        Self::Email,
        Self::Packages,
        Self::Deployments,
        Self::Tasks,
        Self::Logging,
        Self::AuditLog,
        Self::Telemetry,
        Self::Proxy,
        Self::DangerZone,
    ];

    /// The pages in the sidebar's Server submenu.
    pub const SERVER_PAGES: [Self; 4] =
        [Self::Server, Self::Proxy, Self::Logging, Self::DangerZone];

    /// The pages in the sidebar's Optional group.
    pub const OPTIONAL: [Self; 7] = [
        Self::Users,
        Self::Email,
        Self::Packages,
        Self::Deployments,
        Self::Tasks,
        Self::AuditLog,
        Self::Telemetry,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Authentication => "Authentication",
            Self::Server => "Server",
            Self::DangerZone => "Danger zone",
            Self::Users => "Users",
            Self::Email => "Email",
            Self::Packages => "Package installation",
            Self::Deployments => "Deployments",
            Self::Tasks => "Tasks",
            Self::Logging => "Logging",
            Self::AuditLog => "Audit log",
            Self::Telemetry => "OpenTelemetry",
            Self::Proxy => "Network proxy",
        }
    }

    /// The Phosphor icon beside the pane in the sidebar.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Authentication => "key",
            Self::Server => "hard-drives",
            Self::Users => "users",
            Self::Email => "envelope-simple",
            Self::Packages => "package",
            Self::Deployments => "clock-counter-clockwise",
            Self::Tasks => "timer",
            Self::Logging => "file-text",
            Self::AuditLog => "clipboard-text",
            Self::Telemetry => "pulse",
            Self::Proxy => "globe",
            Self::DangerZone => "warning",
        }
    }

    /// What the pane is for.
    pub fn summary(self) -> Option<String> {
        if self == Self::Server {
            return Some(if cfg!(any(target_os = "macos", windows)) {
                "Ricochet runs on this machine whenever you're logged in.".to_owned()
            } else {
                "Ricochet runs on this machine as a systemd service.".to_owned()
            });
        }
        self.fixed_summary().map(str::to_owned)
    }

    fn fixed_summary(self) -> Option<&'static str> {
        match self {
            Self::Authentication | Self::Server => None,
            Self::Users => Some("Manage user roles and access."),
            Self::Email => Some("Configure the mail server ricochet uses to send notifications."),
            Self::Packages => Some("Manage default package repository sources and overrides."),
            Self::Deployments => Some("Manage deployment history and bundle size limits."),
            Self::Tasks => Some("Manage task and package install limits."),
            Self::Logging => Some("ricochet writes its logs to `~/.local/share/ricochet/logs`."),
            Self::AuditLog => Some("Manage audit log retention and storage."),
            Self::Telemetry => Some("Send traces to an OpenTelemetry collector over gRPC."),
            Self::Proxy => Some(
                "Only needed when this machine reaches the internet through a proxy. ricochet uses it to install R, Julia and Python packages and to deploy from Git, and the app uses it to download ricochet.",
            ),
            Self::DangerZone => Some("Browse ricochet's files, delete its data, or uninstall it."),
        }
    }

    pub fn problem(self, config: &ServerConfig) -> Option<String> {
        match self {
            Self::Authentication => config.authentication.problem(),
            Self::Server => config.server.problem(),
            Self::Users => config.users.problem(),
            Self::Email => config.email.problem(),
            Self::Packages => config.packages.problem(),
            Self::Deployments => config.deployments.problem(),
            Self::Tasks => config.tasks.problem(),
            Self::Logging => config.logging.problem(),
            Self::AuditLog => config.audit_log.problem(),
            Self::Telemetry => config.telemetry.problem(),
            Self::Proxy => config.proxy.problem(),
            Self::DangerZone => None,
        }
    }

    /// The first pane that still needs an answer, with what it needs.
    pub fn first_problem(config: &ServerConfig) -> Option<(Self, String)> {
        Self::ALL
            .into_iter()
            .find_map(|section| Some((section, section.problem(config)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_setup_needs_authentication_first() {
        let (section, _) = SetupSection::first_problem(&ServerConfig::default()).unwrap();
        assert_eq!(section, SetupSection::Authentication);
    }

    #[test]
    fn managed_auth_completes_every_pane() {
        let mut config = ServerConfig::default();
        config.authentication.managed = true;
        assert_eq!(SetupSection::first_problem(&config), None);
    }
}
