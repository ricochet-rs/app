import SwiftUI
import TOMLKit

struct SetupView: View {
  static let windowID = "setup"

  @Environment(ServerController.self) private var server
  @Environment(\.dismissWindow) private var dismissWindow
  @State private var section = SetupSection.authentication
  @State private var usersTab = UsersTab.settings
  @State private var config = ServerConfig()
  /// The issuer ricochet signs people in with now, or `nil` before the first setup.
  @State private var savedIssuer: String?
  @State private var problem: String?

  var body: some View {
    NavigationSplitView {
      sidebar
    } detail: {
      detail
    }
    .frame(minWidth: 680, minHeight: 480)
    .navigationTitle(server.isConfigured ? "Edit configuration" : "Set up ricochet")
    .onChange(of: config) { problem = nil }
    .onChange(of: config.authentication.managed) { _, managed in
      if managed { config.server.port = AuthenticationSettings.managedPort }
    }
    .onAppear {
      NSApp.activate()
      if let saved = try? String(contentsOf: Paths.config, encoding: .utf8),
        let table = try? TOMLTable(string: saved)
      {
        config = ServerConfig(table: table)
        savedIssuer = config.authentication.issuer
      }
    }
  }

  /// Basecoat's `sidebar`, grouped into required and optional panes, with a dot on each pane that needs an answer.
  private var sidebar: some View {
    VStack(alignment: .leading, spacing: 2) {
      ForEach([true, false], id: \.self) { required in
        Text(required ? "Required" : "Optional")
          .font(.system(size: 12, weight: .medium))
          .foregroundStyle(Theme.mutedForeground)
          .padding(.horizontal, 12)
          .padding(.top, required ? 4 : 16)
          .padding(.bottom, 4)
        ForEach(SetupSection.allCases.filter { $0.isRequired == required }) { item in
          Button {
            section = item
          } label: {
            HStack(spacing: 8) {
              Image(systemName: item.symbol)
                .frame(width: 16)
              Text(item.title)
              Spacer()
              if item.problem(in: config) != nil {
                Circle()
                  .fill(Theme.destructive)
                  .frame(width: 6, height: 6)
              }
            }
          }
          .buttonStyle(.basecoat(.ghost, small: true, fullWidth: true, alignment: .leading))
          .background(section == item ? Theme.accent : .clear)
        }
      }
      Spacer()
    }
    .padding(8)
    .frame(maxHeight: .infinity)
    .background(Theme.sidebar)
    .navigationSplitViewColumnWidth(min: 170, ideal: 200, max: 240)
  }

  private var detail: some View {
    VStack(spacing: 0) {
      ScrollView {
        VStack(alignment: .leading, spacing: 24) {
          VStack(alignment: .leading, spacing: 6) {
            Text(section.title)
              .font(.system(size: 20, weight: .semibold))
              .foregroundStyle(Theme.foreground)
            if let summary = section.summary {
              Text(summary)
                .font(.system(size: 14))
                .foregroundStyle(Theme.mutedForeground)
                .fixedSize(horizontal: false, vertical: true)
            }
          }
          .textSelection(.enabled)
          pane
            .disabled(server.activity != nil)
        }
        .padding(24)
        .frame(maxWidth: .infinity, alignment: .leading)
      }

      Rectangle().fill(Theme.border).frame(height: 1)

      VStack(alignment: .leading, spacing: 12) {
        if let problem {
          ErrorAlert(title: "Check your answers", message: problem) { self.problem = nil }
        } else if let failure = server.failure {
          ErrorAlert(title: failure.title, message: failure.message, dismiss: server.dismissFailure)
        }
        HStack(spacing: 12) {
          if let activity = server.activity {
            ProgressView().controlSize(.small)
            Text(activity)
              .font(.system(size: 13))
              .foregroundStyle(Theme.mutedForeground)
          }
          Spacer()
          Button(saveTitle, action: submit)
            .buttonStyle(.basecoat())
            .keyboardShortcut(.defaultAction)
            .disabled(server.activity != nil)
        }
      }
      .padding(16)
    }
    .background(Theme.background)
  }

  @ViewBuilder private var pane: some View {
    switch section {
    case .authentication:
      AuthenticationPane(
        settings: $config.authentication, redirectURL: config.redirectURL,
        changesIssuer: savedIssuer.map { $0 != config.authentication.issuer } ?? false
      ) {
        usersTab = .people
        section = .users
      }
    case .server:
      ServerPane(
        settings: $config.server, localURL: config.localURL,
        portLocked: config.authentication.managed, isInstalled: server.installed != nil,
        install: { Task { await server.install() } },
        uninstall: { Task { await server.uninstall() } })
    case .users:
      UsersPane(
        settings: $config.users, tab: $usersTab, isConfigured: server.isConfigured,
        openRicochet: { _ = NSWorkspace.shared.open(server.dashboard) })
    case .email: EmailPane(settings: $config.email)
    case .packages: PackagesPane(settings: $config.packages)
    case .deployments: DeploymentsPane(settings: $config.deployments)
    case .tasks: TasksPane(settings: $config.tasks)
    case .logging: LoggingPane(settings: $config.logging)
    case .auditLog: AuditLogPane(settings: $config.auditLog)
    case .telemetry: TelemetryPane(settings: $config.telemetry)
    }
  }

  private var saveTitle: String {
    if server.isConfigured { return "Save and restart" }
    return server.installed == nil ? "Install and start" : "Save and start"
  }

  /// Starts the install, or opens the first pane that still needs an answer.
  private func submit() {
    if let unanswered = SetupSection.allCases.first(where: { $0.problem(in: config) != nil }) {
      section = unanswered
      problem = unanswered.problem(in: config)
      return
    }
    Task {
      await server.setUp(config)
      if server.failure == nil { dismissWindow(id: Self.windowID) }
    }
  }
}
