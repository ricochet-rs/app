import SwiftUI

struct MenuView: View {
  @Environment(ServerController.self) private var server
  @Environment(\.openWindow) private var openWindow

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      HStack {
        VStack(alignment: .leading, spacing: 4) {
          Text("Ricochet")
            .font(.system(size: 14, weight: .semibold))
            .foregroundStyle(Theme.foreground)
          Text(server.installed.map { "Version \($0.version)" } ?? "Not installed")
            .font(.system(size: 12))
            .foregroundStyle(Theme.mutedForeground)
        }
        Spacer()
        statusBadge
      }
      .padding(16)

      if let publicURL = server.publicURL {
        HStack(spacing: 8) {
          Circle()
            .fill(publicColor)
            .frame(width: 6, height: 6)
          Text(publicURL.host() ?? publicURL.absoluteString)
            .lineLimit(1)
            .truncationMode(.middle)
          Spacer()
          Text(publicLabel)
            .foregroundStyle(Theme.mutedForeground)
        }
        .font(.system(size: 12))
        .foregroundStyle(Theme.foreground)
        .padding(.horizontal, 16)
        .padding(.bottom, 12)
      }

      Rectangle().fill(Theme.border).frame(height: 1)

      VStack(alignment: .leading, spacing: 12) {
        if let activity = server.activity {
          HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(activity)
          }
          .font(.system(size: 13))
          .foregroundStyle(Theme.mutedForeground)
        }
        if let failure = server.failure {
          ErrorAlert(title: failure.title, message: failure.message, dismiss: server.dismissFailure)
        }
        if let release = server.availableUpdate {
          HStack {
            Text("You can update to \(release.version)")
              .font(.system(size: 13))
              .foregroundStyle(Theme.mutedForeground)
            Spacer()
            Button("Update") { Task { await server.update(to: release) } }
              .buttonStyle(.basecoat(.outline, small: true))
          }
        }
        if server.isConfigured {
          controls
        } else {
          Button("Set up ricochet", action: openSetup)
            .buttonStyle(.basecoat(fullWidth: true))
        }
      }
      .disabled(server.activity != nil)
      .padding(16)

      Rectangle().fill(Theme.border).frame(height: 1)

      VStack(alignment: .leading, spacing: 2) {
        if server.isConfigured {
          Button("Show logs") {
            let hasLogs = FileManager.default.fileExists(
              atPath: Paths.logs.path(percentEncoded: false))
            _ = NSWorkspace.shared.open(hasLogs ? Paths.logs : Paths.home)
          }
          Button("Edit configuration", action: openSetup)
            .disabled(server.activity != nil)
        }
        Button("Quit") { NSApp.terminate(nil) }
        Text("Ricochet keeps running after you quit this app.")
          .font(.system(size: 12))
          .foregroundStyle(Theme.mutedForeground)
          .padding(.horizontal, 12)
          .padding(.vertical, 6)
      }
      .buttonStyle(.basecoat(.ghost, small: true, fullWidth: true, alignment: .leading))
      .padding(8)
    }
    .frame(width: 300)
    .background(Theme.background)
  }

  @ViewBuilder private var controls: some View {
    Button("Open ricochet") { _ = NSWorkspace.shared.open(server.dashboard) }
      .buttonStyle(.basecoat(fullWidth: true))
      .disabled(server.status != .running)
    HStack(spacing: 8) {
      if server.status == .stopped {
        Button("Start") { Task { await server.start() } }
      } else {
        Button("Stop") { Task { await server.stop() } }
      }
      Button("Restart") { Task { await server.restart() } }
        .disabled(server.status == .stopped)
    }
    .buttonStyle(.basecoat(.outline, fullWidth: true))
  }

  private var statusBadge: some View {
    HStack(spacing: 6) {
      Circle().fill(statusColor).frame(width: 6, height: 6)
      Text(statusLabel)
    }
    .font(.system(size: 12, weight: .medium))
    .foregroundStyle(Theme.foreground)
    .padding(.horizontal, 8)
    .padding(.vertical, 2)
    .overlay(Rectangle().strokeBorder(Theme.border))
  }

  private var statusLabel: String {
    switch server.status {
    case .running: "Running"
    case .starting: "Starting"
    case .stopped: "Stopped"
    case .notInstalled: "Not set up"
    }
  }

  private var statusColor: Color {
    switch server.status {
    case .running: Theme.success
    case .starting: Theme.warning
    case .stopped, .notInstalled: Theme.mutedForeground
    }
  }

  private var publicLabel: String {
    switch server.publicReachable {
    case true?: "Reachable"
    case false?: "Not reachable"
    case nil: "Checking"
    }
  }

  private var publicColor: Color {
    switch server.publicReachable {
    case true?: Theme.success
    case false?: Theme.destructive
    case nil: Theme.mutedForeground
    }
  }

  private func openSetup() {
    openWindow(id: SetupView.windowID)
    NSApp.activate()
  }
}
