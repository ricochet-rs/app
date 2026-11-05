import Foundation

/// A launchd job in your login session, which launchd starts at login and restarts when it exits.
struct LaunchAgent {
  let label: String
  /// Where launchd writes the job's stderr, cleared before each start so it shows only that run.
  let errorLog: URL

  static let server = LaunchAgent(label: "rs.ricochet.server", errorLog: Paths.stderr)

  var plist: URL { Paths.userHome.appending(path: "Library/LaunchAgents/\(label).plist") }
  private var domain: String { "gui/\(getuid())" }
  private var service: String { "\(domain)/\(label)" }

  /// Writes the job described by `settings` under this label, returning whether the file changed.
  @discardableResult
  func write(_ settings: [String: Any]) throws -> Bool {
    var job = settings
    job["Label"] = label
    let data = try PropertyListSerialization.data(fromPropertyList: job, format: .xml, options: 0)
    if (try? Data(contentsOf: plist)) == data { return false }
    try FileManager.default.createDirectory(
      at: plist.deletingLastPathComponent(), withIntermediateDirectories: true)
    try data.write(to: plist, options: .atomic)
    return true
  }

  func isLoaded() async -> Bool {
    (try? await Command.run("/bin/launchctl", ["print", service]))?.status == 0
  }

  /// Loads the job, which starts it now and at every login.
  func start() async throws {
    _ = try await Command.run("/bin/launchctl", ["enable", service])
    if await isLoaded() { return }
    try? FileManager.default.removeItem(at: errorLog)
    let result = try await Command.run(
      "/bin/launchctl", ["bootstrap", domain, plist.path(percentEncoded: false)])
    guard result.status == 0 else { throw AppError.launchAgent(result.output) }
  }

  /// Unloads the job and disables it, so it stays stopped across logins until `start`.
  func stop() async throws {
    _ = try await Command.run("/bin/launchctl", ["disable", service])
    guard await isLoaded() else { return }
    let result = try await Command.run("/bin/launchctl", ["bootout", service])
    guard result.status == 0 else { throw AppError.launchAgent(result.output) }
    for _ in 0..<40 {
      guard await isLoaded() else { return }
      try await Task.sleep(for: .milliseconds(250))
    }
  }

  func restart() async throws {
    try? FileManager.default.removeItem(at: errorLog)
    let result = try await Command.run("/bin/launchctl", ["kickstart", "-k", service])
    guard result.status == 0 else { throw AppError.launchAgent(result.output) }
  }

  /// Stops the job and deletes it, so nothing starts it again.
  func remove() async throws {
    try await stop()
    try? FileManager.default.removeItem(at: plist)
  }
}

extension LaunchAgent {
  private static let sandboxExec = "/usr/bin/sandbox-exec"

  /// Seatbelt rules for ricochet-server and every process it starts, which inherit them.
  /// Nothing under `/Users` or `/Volumes` is readable except the listed folders, and only ricochet's folder is writable.
  private static let sandboxProfile = """
    (version 1)
    (allow default)
    (deny appleevent-send)
    (deny file-read-data (subpath "/Users") (subpath "/Volumes"))
    (allow file-read-data
        (subpath (param "RICOCHET_HOME"))
        (subpath (param "LOCAL_BIN"))
        (subpath (param "UV_PYTHONS"))
        (subpath (param "JULIAUP_BIN"))
        (subpath (param "JULIAUP_DATA")))
    (deny file-write*)
    (allow file-write*
        (subpath (param "RICOCHET_HOME"))
        (subpath (param "JULIAUP_DATA"))
        (subpath "/private/tmp")
        (subpath "/private/var/folders")
        (subpath "/dev"))
    """

  /// Writes the job that runs ricochet-server in its sandbox, returning whether the file changed.
  @discardableResult
  static func writeServerJob() throws -> Bool {
    guard FileManager.default.isExecutableFile(atPath: sandboxExec) else {
      throw AppError.sandboxUnavailable
    }
    let localBin = Paths.userHome.appending(path: ".local/bin")
    let uvPythons = Paths.userHome.appending(path: ".local/share/uv/python")
    let juliaupBin = Paths.userHome.appending(path: ".juliaup")
    let juliaDepot = Paths.userHome.appending(path: ".julia")
    let parameters = [
      ("RICOCHET_HOME", Paths.home),
      ("LOCAL_BIN", localBin),
      ("UV_PYTHONS", uvPythons),
      ("JULIAUP_BIN", juliaupBin),
      ("JULIAUP_DATA", juliaDepot.appending(path: "juliaup")),
    ].flatMap { ["-D", "\($0.0)=\($0.1.path(percentEncoded: false))"] }
    let path = [
      localBin.path(percentEncoded: false),
      juliaupBin.appending(path: "bin").path(percentEncoded: false),
      "/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin",
    ]
    try FileManager.default.createDirectory(
      at: Paths.sandboxHome, withIntermediateDirectories: true)
    return try server.write([
      "ProgramArguments": [Paths.launcher.path(percentEncoded: false)] + parameters + [
        "-p", sandboxProfile,
        Paths.binary.path(percentEncoded: false), "start",
        "--config", Paths.config.path(percentEncoded: false),
      ],
      "WorkingDirectory": Paths.home.path(percentEncoded: false),
      "EnvironmentVariables": [
        "HOME": Paths.sandboxHome.path(percentEncoded: false),
        "PATH": path.joined(separator: ":"),
        "UV_PYTHON_INSTALL_DIR": uvPythons.path(percentEncoded: false),
        "JULIAUP_DEPOT_PATH": juliaDepot.path(percentEncoded: false),
      ],
      "AssociatedBundleIdentifiers": "rs.ricochet.mac",
      "RunAtLoad": true,
      "KeepAlive": ["SuccessfulExit": false],
      "StandardOutPath": Paths.stdout.path(percentEncoded: false),
      "StandardErrorPath": Paths.stderr.path(percentEncoded: false),
    ])
  }
}
