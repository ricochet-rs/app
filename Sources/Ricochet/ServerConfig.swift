import Foundation
import TOMLKit

/// Every pane's answers, stored in `ricochet-config.toml`.
struct ServerConfig: Equatable {
  var authentication = AuthenticationSettings()
  var server = ServerSettings()
  var users = UserSettings()
  var email = EmailSettings()
  var packages = PackageSettings()
  var deployments = DeploymentSettings()
  var tasks = TaskSettings()
  var logging = LoggingSettings()
  var auditLog = AuditLogSettings()
  var telemetry = TelemetrySettings()

  var localURL: String { "http://localhost:\(server.proxyPort ?? 6188)" }

  var redirectURL: String { localURL + "/oauth/callback" }

  private var sections: [any SettingsSection] {
    [
      authentication, server, users, email, packages, deployments, tasks, logging, auditLog,
      telemetry,
    ]
  }

  /// Writes every pane into the config `file`, keeping each setting no pane manages.
  func save(proxyPort: UInt16, to file: URL) throws {
    let existing = (try? String(contentsOf: file, encoding: .utf8)) ?? ""
    do {
      let table = try TOMLTable(string: existing)
      table["home"] = Paths.home.path(percentEncoded: false)
      table["url"] = localURL
      for section in sections { section.apply(to: table) }

      let ports = NetworkPorts(proxy: proxyPort, keeping: table["network"]?.table)
      let network = table.section("network")
      network["proxy_port"] = Int(ports.proxy)
      network["api_port"] = Int(ports.api)
      network["reload_port"] = Int(ports.reload)
      network["metrics"] = "127.0.0.1:\(ports.metrics)"

      guard
        FileManager.default.createFile(
          atPath: file.path(percentEncoded: false), contents: Data(table.convert().utf8),
          attributes: [.posixPermissions: 0o600])
      else { throw CocoaError(.fileWriteUnknown) }
    } catch let error as TOMLParseError {
      throw AppError.invalidConfig(line: error.source.begin.line, reason: error.description)
    }
  }
}

extension ServerConfig {
  /// Every pane's answers as saved in `table`, the parsed `ricochet-config.toml`.
  init(table: TOMLTable) {
    self.init(
      authentication: AuthenticationSettings(table: table),
      server: ServerSettings(table: table),
      users: UserSettings(table: table),
      email: EmailSettings(table: table),
      packages: PackageSettings(table: table),
      deployments: DeploymentSettings(table: table),
      tasks: TaskSettings(table: table),
      logging: LoggingSettings(table: table),
      auditLog: AuditLogSettings(table: table),
      telemetry: TelemetrySettings(table: table))
  }
}
