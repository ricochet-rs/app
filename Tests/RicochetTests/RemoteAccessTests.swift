import Foundation
import TOMLKit
import Testing

@testable import Ricochet

@Test func remoteAccessRoundTripsWithItsTokenKeptApart() throws {
  let folder = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
  defer { try? FileManager.default.removeItem(at: folder) }
  let token = folder.appending(path: RemoteAccessSettings.tokenFileName)
  let settings = RemoteAccessSettings(
    mode: .cloudflare, cloudflareHostname: "ricochet.example.com",
    cloudflareToken: "the-tunnel-token")

  try settings.save(to: folder)

  #expect(RemoteAccessSettings.load(from: folder) == settings)
  let json = try String(
    contentsOf: folder.appending(path: RemoteAccessSettings.fileName), encoding: .utf8)
  #expect(!json.contains("the-tunnel-token"))
  let permissions = try FileManager.default.attributesOfItem(
    atPath: token.path(percentEncoded: false))[.posixPermissions]
  #expect(permissions as? Int == 0o600)

  var off = settings
  off.mode = .off
  try off.save(to: folder)
  #expect(!FileManager.default.fileExists(atPath: token.path(percentEncoded: false)))
}

@Test func aPastedInstallCommandSavesOnlyItsToken() throws {
  let folder = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
  defer { try? FileManager.default.removeItem(at: folder) }
  let settings = RemoteAccessSettings(
    mode: .cloudflare, cloudflareHostname: "ricochet.example.com",
    cloudflareToken: "brew install cloudflared && sudo cloudflared service install eyJhIjoiabc")

  try settings.save(to: folder)

  #expect(RemoteAccessSettings.load(from: folder)?.cloudflareToken == "eyJhIjoiabc")
}

@Test func savedInternalPortsAreKept() throws {
  let network = try TOMLTable(
    string: "api_port = 4321\nreload_port = 4322\nmetrics = \"127.0.0.1:4323\"")
  let ports = NetworkPorts(proxy: 6188, keeping: network)
  #expect([ports.api, ports.reload, ports.metrics] == [4321, 4322, 4323])
}

@Test func urlFollowsTheRemoteAccessMode() {
  let local = "http://localhost:6188"
  #expect(RemoteAccessSettings().url(local: local) == local)
  #expect(
    RemoteAccessSettings(mode: .tailscale, tailscaleHostname: "mac.tail1234.ts.net").url(
      local: local)
      == "https://mac.tail1234.ts.net")
  #expect(
    RemoteAccessSettings(mode: .cloudflare, cloudflareHostname: "https://ricochet.example.com/")
      .url(local: local)
      == "https://ricochet.example.com")
  #expect(
    RemoteAccessSettings(mode: .custom, customURL: "https://proxy.example.com/").url(local: local)
      == "https://proxy.example.com")
  #expect(RemoteAccessSettings(mode: .tailscale).url(local: local) == local)
}

@Test func tailscaleReadinessFollowsItsStatus() {
  func readiness(_ status: String, control: String = "https://controlplane.tailscale.com")
    -> Tailscale.Readiness
  {
    Tailscale.readiness(
      status: Data(status.utf8), prefs: Data(#"{"ControlURL": "\#(control)"}"#.utf8))
  }

  #expect(
    readiness(#"{"BackendState": "NeedsLogin", "Self": null, "CertDomains": null}"#) == .signedOut)
  #expect(
    readiness(
      #"{"BackendState": "Running", "Self": {"DNSName": "mac.ts.example.com.", "CapMap": {}}, "CertDomains": null}"#,
      control: "https://headscale.example.com") == .selfHosted)
  #expect(
    readiness(
      #"{"BackendState": "Running", "Self": {"DNSName": "mac.tail1234.ts.net.", "CapMap": {}}, "CertDomains": null}"#
    ) == .needsHTTPS)
  #expect(
    readiness(
      #"{"BackendState": "Running", "Self": {"DNSName": "mac.tail1234.ts.net.", "CapMap": {"https": null}}, "CertDomains": ["mac.tail1234.ts.net"]}"#
    ) == .needsFunnel)
  #expect(
    readiness(
      #"{"BackendState": "Running", "Self": {"DNSName": "mac.tail1234.ts.net.", "CapMap": {"https": null, "funnel": null}}, "CertDomains": ["mac.tail1234.ts.net"]}"#
    ) == .ready(hostname: "mac.tail1234.ts.net"))
  #expect(readiness("not json") == .signedOut)
}
