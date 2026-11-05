/// A pane in the setup window, one per settings section.
enum SetupSection: CaseIterable, Identifiable {
  case signIn
  case server
  case remoteAccess
  case users
  case email
  case packages
  case deployments
  case tasks
  case logging
  case auditLog
  case telemetry

  var id: Self { self }

  var isRequired: Bool { self == .signIn || self == .server }

  var title: String {
    switch self {
    case .signIn: "Sign-in"
    case .server: "Server"
    case .remoteAccess: "Remote access"
    case .users: "Users"
    case .email: "Email"
    case .packages: "Packages"
    case .deployments: "Deployments"
    case .tasks: "Tasks"
    case .logging: "Logging"
    case .auditLog: "Audit log"
    case .telemetry: "OpenTelemetry"
    }
  }

  var summary: String {
    switch self {
    case .signIn:
      "Register an app with your identity provider, then enter its details. The first person to sign in becomes the admin."
    case .server: "Ricochet runs on this Mac whenever you're logged in."
    case .remoteAccess: "Let people outside this Mac reach ricochet at an https address."
    case .users: "Choose the role new people get and how ricochet reads their groups."
    case .email:
      "Ricochet sends email through this mail server, such as notices about failed deployments."
    case .packages: "Choose where ricochet installs R, Julia, and Python packages from."
    case .deployments:
      "Choose how many old deployments ricochet keeps and the largest bundle people can deploy."
    case .tasks: "Choose how long ricochet lets a task or a package install run."
    case .logging: "Ricochet writes its logs to ~/.local/share/ricochet/logs."
    case .auditLog: "Ricochet records who changed what. Choose how long it keeps those records."
    case .telemetry: "Ricochet sends traces to an OpenTelemetry collector over gRPC."
    }
  }

  var symbol: String {
    switch self {
    case .signIn: "person.badge.key"
    case .server: "server.rack"
    case .remoteAccess: "globe"
    case .users: "person.2"
    case .email: "envelope"
    case .packages: "shippingbox"
    case .deployments: "clock.arrow.circlepath"
    case .tasks: "timer"
    case .logging: "doc.text"
    case .auditLog: "list.bullet.clipboard"
    case .telemetry: "waveform.path.ecg"
    }
  }

  func problem(in config: ServerConfig) -> String? {
    switch self {
    case .signIn: config.signIn.problem
    case .server: config.server.problem
    case .remoteAccess: config.remoteAccess.problem
    case .users: config.users.problem
    case .email: config.email.problem
    case .packages: config.packages.problem
    case .deployments: config.deployments.problem
    case .tasks: config.tasks.problem
    case .logging: config.logging.problem
    case .auditLog: config.auditLog.problem
    case .telemetry: config.telemetry.problem
    }
  }
}
