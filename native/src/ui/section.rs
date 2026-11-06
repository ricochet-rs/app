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
}

impl SetupSection {
    pub const ALL: [Self; 10] = [
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
    ];

    pub fn is_required(self) -> bool {
        matches!(self, Self::Authentication | Self::Server)
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Authentication => "Authentication",
            Self::Server => "Server",
            Self::Users => "Users",
            Self::Email => "Email",
            Self::Packages => "Packages",
            Self::Deployments => "Deployments",
            Self::Tasks => "Tasks",
            Self::Logging => "Logging",
            Self::AuditLog => "Audit log",
            Self::Telemetry => "OpenTelemetry",
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
        }
    }

    pub fn summary(self) -> Option<&'static str> {
        match self {
            Self::Authentication => None,
            Self::Server => Some("Ricochet runs on this computer whenever you're logged in."),
            Self::Users => Some("Change each person's role, and choose the role new people get."),
            Self::Email => Some(
                "Ricochet sends email through this mail server, such as notices about failed deployments.",
            ),
            Self::Packages => {
                Some("Choose where ricochet installs R, Julia, and Python packages from.")
            }
            Self::Deployments => Some(
                "Choose how many old deployments ricochet keeps and the largest bundle people can deploy.",
            ),
            Self::Tasks => Some("Choose how long ricochet lets a task or a package install run."),
            Self::Logging => Some("Ricochet writes its logs to ~/.local/share/ricochet/logs."),
            Self::AuditLog => {
                Some("Ricochet records who changed what. Choose how long it keeps those records.")
            }
            Self::Telemetry => {
                Some("Ricochet sends traces to an OpenTelemetry collector over gRPC.")
            }
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
