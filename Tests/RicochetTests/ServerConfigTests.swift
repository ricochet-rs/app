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
  config.authentication = AuthenticationSettings(
    managed: false, issuerURL: "https://auth.example.com", clientID: "ricochet",
    clientSecret: #"a "quoted" \secret"#)

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
    authentication: AuthenticationSettings(
      managed: false, provider: .entra,
      issuerURL: "https://login.microsoftonline.com/tenant/v2.0", clientID: "id",
      clientSecret: "secret", displayNameClaim: "name", groupsClaim: "groups", syncInterval: "30"),
    server: ServerSettings(port: "7000", licenseKey: "KEY"),
    users: UserSettings(defaultRole: .consumer, requireSignIn: true, allowAvatarUpload: false),
    email: EmailSettings(
      enabled: true, host: "smtp.example.com", port: "2525", encryption: .tls, username: "user",
      password: "password", from: "ricochet <ricochet@example.com>", replyTo: "help@example.com"),
    packages: PackageSettings(
      r: [
        RRepository(name: "CRAN", url: "https://cloud.r-project.org"),
        RRepository(name: "P3M", url: "https://p3m.dev/cran/latest"),
      ],
      julia: [PackageIndex(url: "https://github.com/JuliaRegistries/General.git")],
      python: [PackageIndex(url: "https://pypi.org/simple")],
      allowOverrides: false),
    deployments: DeploymentSettings(maxDeployments: "3", maxAgeDays: "30", maxBundleSize: "1GB"),
    tasks: TaskSettings(maxDuration: "1.5", restoreTimeout: "20", restoreConcurrency: "2"),
    logging: LoggingSettings(
      level: .debug, rotation: .hourly, compress: false, maxFiles: "10", maxAgeDays: "14"),
    auditLog: AuditLogSettings(keepForever: true, retentionDays: "", writeFile: true),
    telemetry: TelemetrySettings(enabled: true, endpoint: "http://localhost:4317"))

  let saved = try save(config, over: "")

  #expect(saved["tasks"]?.table?["max_duration"]?.int == 90)
  #expect(saved["audit"]?.table?["retention_days"]?.string == "forever")
  #expect(ServerConfig(table: saved) == config)
}

@Test func defaultsWriteOnlyManagedAuthAndTheRequiredKeys() throws {
  let saved = try save(ServerConfig(), over: "[otel]\nendpoint = \"http://localhost:4317\"\n")

  #expect(Set(saved.keys) == ["home", "url", "auth", "logs", "network"])
  #expect(Set(saved["auth"]?.table?.keys ?? []) == ["default_role", "oidc"])
  let oidc = saved["auth"]?.table?["oidc"]?.table
  #expect(Set(oidc?.keys ?? []) == ["provider", "issuer_url", "client_id"])
  #expect(oidc?["issuer_url"]?.string == "https://idp.ricochet.rs")
  #expect(oidc?["client_id"]?.string == "398284990531538194")
  #expect(Set(saved["logs"]?.table?.keys ?? []) == ["file_logging"])
  #expect(ServerConfig(table: saved) == ServerConfig())
}

@Test func savedInternalPortsAreKept() throws {
  let network = try TOMLTable(
    string: "api_port = 4321\nreload_port = 4322\nmetrics = \"127.0.0.1:4323\"")
  let ports = NetworkPorts(proxy: 6188, keeping: network)
  #expect([ports.api, ports.reload, ports.metrics] == [4321, 4322, 4323])
}

@Test func switchingToManagedAuthRemovesYourProvidersKeys() throws {
  let saved = try save(
    ServerConfig(),
    over: """
      [auth.oidc]
      issuer_url = "https://auth.example.com"
      client_id = "ricochet"
      client_secret = "secret"
      groups_claim = "groups"
      sync_interval = 30
      """)

  #expect(
    Set(saved["auth"]?.table?["oidc"]?.table?.keys ?? []) == [
      "provider", "issuer_url", "client_id",
    ])
}

@Test func aSecretMeansYourOwnProviderEvenAtTheManagedIssuer() throws {
  let table = try TOMLTable(
    string: """
      [auth.oidc]
      issuer_url = "https://idp.ricochet.rs"
      client_id = "398284990531538194"
      client_secret = "secret"
      """)

  #expect(!AuthenticationSettings(table: table).managed)
}
