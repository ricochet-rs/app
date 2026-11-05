import TOMLKit

enum Role: String, CaseIterable, Hashable {
    case admin
    case developer
    case consumer

    var title: String { rawValue.capitalized }
}

/// The rest of `[auth]`: the role new people get, and how ricochet reads their groups.
struct UserSettings: Equatable {
    var defaultRole = Role.developer
    var requireSignIn = false
    var allowAvatarUpload = true
    var displayNameClaim = ""
    var groupsClaim = ""
    var syncInterval = ""
}

extension UserSettings: SettingsSection {
    init(table: TOMLTable) {
        self.init()
        let auth = table["auth"]?.table
        defaultRole = auth?["default_role"]?.string.flatMap(Role.init(rawValue:)) ?? .developer
        requireSignIn = auth?["require_authentication"]?.bool ?? false
        allowAvatarUpload = auth?["allow_avatar_upload"]?.bool ?? true
        let oidc = auth?["oidc"]?.table
        displayNameClaim = oidc?["display_name_claim"]?.string ?? ""
        groupsClaim = oidc?["groups_claim"]?.string ?? ""
        syncInterval = oidc?["sync_interval"]?.int.map(String.init) ?? ""
    }

    /// Writes `default_role` even as Developer, because ricochet warns at every sign-in when it is absent.
    func apply(to table: TOMLTable) {
        let auth = table.section("auth")
        auth["default_role"] = defaultRole.rawValue
        auth.set("require_authentication", requireSignIn ? true : nil)
        auth.set("allow_avatar_upload", allowAvatarUpload ? nil : false)
        let oidc = auth.section("oidc")
        oidc.set("display_name_claim", displayNameClaim.nonEmpty)
        oidc.set("groups_claim", groupsClaim.nonEmpty)
        oidc.set("sync_interval", syncInterval.wholeNumber)
    }

    var problem: String? {
        syncInterval.isEmptyOrPositive ? nil : "Enter the group sync interval as a whole number of minutes."
    }
}
