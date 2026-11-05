import SwiftUI

@main
struct RicochetApp: App {
  @NSApplicationDelegateAdaptor private var appDelegate: AppDelegate
  @State private var server = ServerController()

  var body: some Scene {
    MenuBarExtra {
      MenuView()
        .environment(server)
    } label: {
      MenuBarLabel(running: server.status == .running)
    }
    .menuBarExtraStyle(.window)

    Window("Set up ricochet", id: SetupView.windowID) {
      SetupView()
        .environment(server)
    }
    .defaultSize(width: 760, height: 600)
    .windowResizability(.contentMinSize)
    .defaultLaunchBehavior(.presented)
  }
}

extension Notification.Name {
  /// Posted when you click Ricochet in the Dock while none of its windows are showing.
  static let openSetupWindow = Notification.Name("rs.ricochet.openSetupWindow")
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
  func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
    if !hasVisibleWindows { NotificationCenter.default.post(name: .openSetupWindow, object: nil) }
    return true
  }
}

/// The rabbit in the menu bar. It also opens the setup window for a Dock click, because the menu bar
/// item is the one view that is always on screen to receive it.
private struct MenuBarLabel: View {
  let running: Bool
  @Environment(\.openWindow) private var openWindow

  var body: some View {
    icon
      .onReceive(NotificationCenter.default.publisher(for: .openSetupWindow)) { _ in
        openWindow(id: SetupView.windowID)
      }
  }

  @ViewBuilder private var icon: some View {
    if let url = Bundle.main.url(
      forResource: running ? "MenuBarRunning" : "MenuBarStopped", withExtension: "pdf"),
      let rabbit = NSImage(contentsOf: url)
    {
      Image(nsImage: Self.template(rabbit))
    } else {
      Image(systemName: running ? "r.circle.fill" : "r.circle")
    }
  }

  /// The rabbit at menu bar size, drawn by macOS in black or white to match the menu bar.
  private static func template(_ image: NSImage) -> NSImage {
    image.size = NSSize(width: 18, height: 18)
    image.isTemplate = true
    return image
  }
}
