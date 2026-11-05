import Foundation
import TOMLKit
import Testing

@testable import Ricochet

private func save(_ config: ServerConfig, over existing: String) throws -> TOMLTable {
  let file = FileManager.default.temporaryDirectory.appending(path: "\(UUID().uuidString).toml")
  defer { try? FileManager.default.removeItem(at: file) }
  try existing.write(to: file, atomically: true, encoding: .utf8)
  try config.save(proxyPort: config.server.proxyPort ?? 6188, to: file)
  let permissions = try FileManager.default.attributesOfItem(
    atPath: file.path(percentEncoded: false))[.posixPermissions]
  #expect(permissions as? Int == 0o600)
  return try TOMLTable(string: String(contentsOf: file, encoding: .utf8))
}

@Test func savingKeepsSettingsTheFormDoesNotManage() throws {
  var config = ServerConfig()
  config.signIn.issuerURL = "https://auth.example.com"
  config.signIn.clientID = "ricochet"
  config.signIn.clientSecret = #"a "quoted" \secret"#
  config.remoteAccess = RemoteAccessSettings(
    mode: .custom, customURL: "https://ricochet.example.com")

  let saved = try save(
    config,
    over: """
      [database]
      url = "sqlite://elsewhere.db"

      [auth.oidc]
      issuer_url = "https://old.example.com"

      [auth.saml]
      entity_id = "https://old.example.com"
      """)

  #expect(saved["database"]?.table?["url"]?.string == "sqlite://elsewhere.db")
  #expect(saved["auth"]?.table?["saml"] == nil)
  #expect(ServerConfig(table: saved) == config)
}

@Test func everyPaneRoundTrips() throws {
  let config = ServerConfig(
    signIn: SignInSettings(
      provider: .entra, issuerURL: "https://login.microsoftonline.com/tenant/v2.0", clientID: "id",
      clientSecret: "secret"),
    server: ServerSettings(port: "7000", licenseKey: "KEY"),
    users: UserSettings(
      defaultRole: .consumer, requireSignIn: true, allowAvatarUpload: false,
      displayNameClaim: "name",
      groupsClaim: "groups", syncInterval: "30"),
    email: EmailSettings(
      enabled: true, host: "smtp.example.com", port: "2525", encryption: .tls, username: "user",
      password: "password", from: "ricochet <ricochet@example.com>", replyTo: "help@example.com"),
    packages: PackageSettings(
      r: [
        RRepository(name: "CRAN", url: "https://cloud.r-project.org"),
        RRepository(name: "P3M", url: "https://p3m.dev/cran/latest"),
      ],
      python: [PackageIndex(url: "https://pypi.org/simple")],
      julia: [PackageIndex(url: "https://github.com/JuliaRegistries/General.git")],
      allowOverrides: false),
    deployments: DeploymentSettings(maxDeployments: "3", maxAgeDays: "30", maxBundleSize: "1GB"),
    tasks: TaskSettings(maxDuration: "1.5", restoreTimeout: "20", restoreConcurrency: "2"),
    logging: LoggingSettings(
      level: .debug, rotation: .hourly, compress: false, maxFiles: "10", maxAgeDays: "14"),
    auditLog: AuditLogSettings(keepForever: true, retentionDays: "", writeFile: true),
    telemetry: TelemetrySettings(enabled: true, endpoint: "http://localhost:4317"),
    remoteAccess: RemoteAccessSettings(mode: .custom, customURL: "https://ricochet.example.com"))

  let saved = try save(config, over: "")

  #expect(saved["tasks"]?.table?["max_duration"]?.int == 90)
  #expect(saved["audit"]?.table?["retention_days"]?.string == "forever")
  #expect(ServerConfig(table: saved) == config)
}

@Test func defaultsWriteOnlyTheRequiredKeys() throws {
  var config = ServerConfig()
  config.signIn.issuerURL = "https://auth.example.com"
  config.signIn.clientID = "ricochet"

  let saved = try save(config, over: "[otel]\nendpoint = \"http://localhost:4317\"\n")

  #expect(Set(saved.keys) == ["home", "url", "auth", "logs", "network"])
  #expect(Set(saved["auth"]?.table?.keys ?? []) == ["default_role", "oidc"])
  #expect(
    Set(saved["auth"]?.table?["oidc"]?.table?.keys ?? []) == [
      "provider", "issuer_url", "client_id",
    ])
  #expect(Set(saved["logs"]?.table?.keys ?? []) == ["file_logging"])
}
