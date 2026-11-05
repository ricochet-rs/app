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

/// `[auth.oidc]`'s connection to the identity provider.
struct SignInSettings: Equatable {
    var provider = Provider.oidc
    var issuerURL = ""
    var clientID = ""
    var clientSecret = ""
}

extension SignInSettings: SettingsSection {
    init(table: TOMLTable) {
        self.init()
        let oidc = table["auth"]?.table?["oidc"]?.table
        provider = oidc?["provider"]?.string.flatMap(Provider.init(rawValue:)) ?? .oidc
        issuerURL = oidc?["issuer_url"]?.string ?? ""
        clientID = oidc?["client_id"]?.string ?? ""
        clientSecret = oidc?["client_secret"]?.string ?? ""
    }

    func apply(to table: TOMLTable) {
        let auth = table.section("auth")
        auth.remove(at: "saml")
        let oidc = auth.section("oidc")
        oidc["provider"] = provider.rawValue
        oidc["issuer_url"] = issuerURL.trimmed
        oidc["client_id"] = clientID.trimmed
        oidc.set("client_secret", clientSecret.nonEmpty)
    }

    var problem: String? {
        if !issuerURL.isHTTPURL { return "Enter the issuer URL, starting with https://." }
        if clientID.nonEmpty == nil { return "Enter the client ID." }
        if provider == .entra, clientSecret.nonEmpty == nil {
            return "Enter the client secret. Ricochet needs it to read groups and photos from Microsoft Entra ID."
        }
        return nil
    }
}
