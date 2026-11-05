import AppKit
import Observation

enum ServerStatus {
  case notInstalled
  case stopped
  case starting
  case running
}

struct Failure: Equatable {
  let title: String
  let message: String
}

/// Installs ricochet-server, starts and stops it, and reports whether it answers.
@MainActor @Observable
final class ServerController {
  private(set) var status = ServerStatus.stopped
  private(set) var installed: ServerRelease?
  private(set) var latest: ServerRelease?
  private(set) var activity: String?
  private(set) var failure: Failure?
  private(set) var isConfigured = FileManager.default.fileExists(
    atPath: Paths.config.path(percentEncoded: false))

  private static let portKey = "proxyPort"

  /// Where you open ricochet on this Mac.
  var dashboard: URL {
    let port = UserDefaults.standard.integer(forKey: Self.portKey)
    return URL(string: "http://localhost:\(port == 0 ? 6188 : port)")!
  }

  var availableUpdate: ServerRelease? {
    guard let latest, let installed, latest.isNewer(than: installed) else { return nil }
    return latest
  }

  init() {
    Task { await self.watch() }
  }

  func setUp(_ config: ServerConfig) async {
    await perform("Checking your settings", failing: "Ricochet didn't finish setting up") {
      guard let proxyPort = config.server.proxyPort else { throw AppError.portInUse(0) }
      // Ricochet itself holds the port it already uses, so only a new port can belong to another app.
      if Int(proxyPort) != UserDefaults.standard.integer(forKey: Self.portKey),
        !NetworkPorts.isFree(proxyPort)
      {
        throw AppError.portInUse(proxyPort)
      }
      activity = "Stopping ricochet"
      try await LaunchAgent.server.stop()
      if installed == nil {
        let release = try await ServerRelease.latest()
        activity = "Downloading ricochet \(release.version)"
        try await release.install()
        installed = await ServerRelease.installed()
      }

      activity = "Writing your configuration"
      try FileManager.default.createDirectory(
        at: Paths.home, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
      try installBundledFiles()
      try config.save(proxyPort: proxyPort, to: Paths.config)
      UserDefaults.standard.set(Int(proxyPort), forKey: Self.portKey)
      try LaunchAgent.writeServerJob()

      activity = "Starting ricochet"
      try await LaunchAgent.server.start()
      try await waitUntilRunning()
      isConfigured = true
      _ = NSWorkspace.shared.open(dashboard)
    }
  }

  func start() async {
    await perform("Starting ricochet", failing: "Ricochet didn't start") {
      try await LaunchAgent.server.start()
      try await waitUntilRunning()
    }
  }

  func stop() async {
    await perform("Stopping ricochet", failing: "Ricochet didn't stop") {
      try await LaunchAgent.server.stop()
    }
  }

  func restart() async {
    await perform("Restarting ricochet", failing: "Ricochet didn't restart") {
      try await LaunchAgent.server.restart()
      try await waitUntilRunning()
    }
  }

  func update(to release: ServerRelease) async {
    await perform(
      "Updating to ricochet \(release.version)", failing: "Ricochet didn't finish updating"
    ) {
      try await release.install()
      try installBundledFiles()
      installed = await ServerRelease.installed()
      if await LaunchAgent.server.isLoaded() {
        try await LaunchAgent.server.restart()
        try await waitUntilRunning()
      }
    }
  }

  func dismissFailure() {
    failure = nil
  }

  /// Downloads the latest ricochet-server without configuring or starting it, which **Install and start** does later.
  func install() async {
    await perform("Checking for the latest ricochet", failing: "This app couldn't install ricochet")
    {
      let release = try await ServerRelease.latest()
      activity = "Downloading ricochet \(release.version)"
      try await release.install()
      installed = await ServerRelease.installed()
    }
  }

  /// Stops everything the app started and moves ricochet and its data to the Trash, keeping this app for a fresh setup.
  func uninstall() async {
    await perform("Uninstalling ricochet", failing: "Ricochet didn't finish uninstalling") {
      try await LaunchAgent.server.remove()
      for item in [Paths.home, Paths.appSupport, Paths.binary]
      where FileManager.default.fileExists(atPath: item.path(percentEncoded: false)) {
        try FileManager.default.trashItem(at: item, resultingItemURL: nil)
      }
      UserDefaults.standard.removeObject(forKey: Self.portKey)
      installed = nil
      isConfigured = false
    }
  }

  private func perform(_ activity: String, failing title: String, _ work: () async throws -> Void)
    async
  {
    self.activity = activity
    failure = nil
    do {
      try await work()
    } catch {
      failure = Failure(title: title, message: error.localizedDescription)
      let entry =
        "\(Date.now.ISO8601Format()) \(title) while \(self.activity ?? activity): \(error)\n"
      try? FileManager.default.createDirectory(
        at: Paths.appSupport, withIntermediateDirectories: true)
      if let log = try? FileHandle(forWritingTo: Paths.appLog) {
        _ = try? log.seekToEnd()
        try? log.write(contentsOf: Data(entry.utf8))
        try? log.close()
      } else {
        FileManager.default.createFile(
          atPath: Paths.appLog.path(percentEncoded: false), contents: Data(entry.utf8))
      }
    }
    self.activity = nil
    await refreshStatus()
  }

  private func watch() async {
    installed = await ServerRelease.installed()
    if isConfigured {
      try? installBundledFiles()
      await perform("Restarting ricochet", failing: "Ricochet didn't restart") {
        guard try LaunchAgent.writeServerJob(), await LaunchAgent.server.isLoaded() else { return }
        try await LaunchAgent.server.stop()
        try await LaunchAgent.server.start()
        try await waitUntilRunning()
      }
    }
    var checkedReleases = Date.distantPast
    while !Task.isCancelled {
      if checkedReleases.timeIntervalSinceNow < -12 * 60 * 60 {
        latest = try? await ServerRelease.latest()
        checkedReleases = .now
      }
      if activity == nil { await refreshStatus() }
      try? await Task.sleep(for: .seconds(3))
    }
  }

  private func refreshStatus() async {
    if installed == nil {
      status = .notInstalled
    } else if await answers() {
      status = .running
    } else if await LaunchAgent.server.isLoaded() {
      status = .starting
    } else {
      status = .stopped
    }
  }

  /// Whether ricochet's `/health` returns 200 within two seconds.
  private func answers() async -> Bool {
    let request = URLRequest(
      url: dashboard.appending(path: "health"), cachePolicy: .reloadIgnoringLocalCacheData,
      timeoutInterval: 2)
    guard let (_, response) = try? await URLSession.shared.data(for: request) else { return false }
    return (response as? HTTPURLResponse)?.statusCode == 200
  }

  private func waitUntilRunning() async throws {
    for _ in 0..<60 {
      if await answers() { return }
      try await Task.sleep(for: .seconds(1))
    }
    let log = (try? String(contentsOf: Paths.stderr, encoding: .utf8)) ?? ""
    throw AppError.didNotStart(log.split(separator: "\n").suffix(8).joined(separator: "\n"))
  }

  /// Copies renv and the launch agent's launcher out of this app, so moving the app never breaks ricochet.
  private func installBundledFiles() throws {
    let manager = FileManager.default
    let contents = Bundle.main.bundleURL.appending(path: "Contents")
    for (bundled, installed) in [
      (contents.appending(path: "Resources/renv.tar.gz"), Paths.renv),
      (contents.appending(path: "Helpers/ricochet"), Paths.launcher),
    ]
    where manager.fileExists(atPath: bundled.path(percentEncoded: false))
      && !manager.contentsEqual(
        atPath: bundled.path(percentEncoded: false), andPath: installed.path(percentEncoded: false))
    {
      try manager.createDirectory(
        at: installed.deletingLastPathComponent(), withIntermediateDirectories: true)
      try? manager.removeItem(at: installed)
      try manager.copyItem(at: bundled, to: installed)
    }
    // launchd can't run a quarantined executable that Apple hasn't notarized.
    removexattr(Paths.launcher.path(percentEncoded: false), "com.apple.quarantine", 0)
  }
}
