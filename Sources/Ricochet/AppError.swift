import Foundation

enum AppError: LocalizedError {
  case portInUse(UInt16)
  case download(String)
  case launchAgent(String)
  case didNotStart(String)
  case sandboxUnavailable
  case invalidConfig(line: Int, reason: String)
  case tailscale(String)

  var errorDescription: String? {
    switch self {
    case .tailscale(let message):
      message
    case .invalidConfig(let line, let reason):
      "Ricochet can't read line \(line) of ricochet-config.toml: \(reason). Fix the file, then save again."
    case .sandboxUnavailable:
      "Ricochet can't find /usr/bin/sandbox-exec, so it can't start inside its sandbox."
    case .portInUse(let port):
      "Another app is using port \(port). Choose a different port."
    case .download(let name):
      "Ricochet couldn't download \(name). Check your internet connection and try again."
    case .launchAgent(let output):
      "Ricochet couldn't start in the background. \(output)"
    case .didNotStart(let log):
      log.isEmpty
        ? "Ricochet didn't respond within a minute. Select Show logs in the menu bar for details."
        : "Ricochet stopped before it was ready:\n\(log)"
    }
  }
}
