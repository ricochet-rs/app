import Foundation
import TOMLKit

enum Provider: String, CaseIterable, Hashable {
  case oidc
  case entra

  var title: String {
    switch self {
    case .oidc: "OpenID Connect"
    case .entra: "Microsoft Entra ID"
    }
  }

  var issuerPrompt: String {
    switch self {
    case .oidc: "https://auth.example.com"
    case .entra: "https://login.microsoftonline.com/<tenant-id>/v2.0"
    }
  }
}

/// `[auth.oidc]`: ricochet managed auth, or your own identity provider and the claims ricochet reads from it.
struct AuthenticationSettings: Equatable {
  /// Whether you selected **I agree** on managed auth, which shares your name and email address with ricochet.
  var managed = false
  var provider = Provider.oidc
  var issuerURL = ""
  var clientID = ""
  var clientSecret = ""
  var displayNameClaim = ""
  var groupsClaim = ""
  var syncInterval = ""

  /// The Zitadel app on idp.ricochet.rs that managed auth signs in with.
  static let managedProvider = AuthenticationSettings(
    managed: false, issuerURL: "https://idp.ricochet.rs", clientID: "398284990531538194")
  /// The only port the managed Zitadel app accepts in its redirect URL.
  static let managedPort = "6188"
  /// The policy you agree to when you sign in through managed auth.
  static let privacyPolicy = URL(string: "https://ricochet.rs/legal/privacy/")!

  /// The issuer ricochet signs people in with.
  var issuer: String { managed ? Self.managedProvider.issuerURL : issuerURL.trimmed }
}

extension AuthenticationSettings: SettingsSection {
  init(table: TOMLTable) {
    self.init()
    guard let oidc = table["auth"]?.table?["oidc"]?.table else { return }
    let saved = AuthenticationSettings(
      managed: false,
      provider: oidc["provider"]?.string.flatMap(Provider.init(rawValue:)) ?? .oidc,
      issuerURL: oidc["issuer_url"]?.string ?? "",
      clientID: oidc["client_id"]?.string ?? "",
      clientSecret: oidc["client_secret"]?.string ?? "",
      displayNameClaim: oidc["display_name_claim"]?.string ?? "",
      groupsClaim: oidc["groups_claim"]?.string ?? "",
      syncInterval: oidc["sync_interval"]?.int.map(String.init) ?? "")
    if saved == Self.managedProvider { managed = true } else { self = saved }
  }

  func apply(to table: TOMLTable) {
    let auth = table.section("auth")
    auth.remove(at: "saml")
    let oidc = auth.section("oidc")
    let written = managed ? Self.managedProvider : self
    oidc["provider"] = written.provider.rawValue
    oidc["issuer_url"] = written.issuerURL.trimmed
    oidc["client_id"] = written.clientID.trimmed
    oidc.set("client_secret", written.clientSecret.nonEmpty)
    oidc.set("display_name_claim", written.displayNameClaim.nonEmpty)
    oidc.set("groups_claim", written.groupsClaim.nonEmpty)
    oidc.set("sync_interval", written.syncInterval.wholeNumber)
  }

  var problem: String? {
    if managed { return nil }
    if issuerURL.nonEmpty == nil, clientID.nonEmpty == nil {
      return "Select I agree on Managed auth, or configure your own IdP."
    }
    if !issuerURL.isHTTPURL { return "Enter the issuer URL, starting with https://." }
    if clientID.nonEmpty == nil { return "Enter the client ID." }
    if provider == .entra, clientSecret.nonEmpty == nil {
      return
        "Enter the client secret. Ricochet needs it to read groups and photos from Microsoft Entra ID."
    }
    if !syncInterval.isEmptyOrPositive {
      return "Enter the group sync interval as a whole number of minutes."
    }
    return nil
  }
}
