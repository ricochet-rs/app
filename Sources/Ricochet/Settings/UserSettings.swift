import TOMLKit

enum Role: String, CaseIterable, Hashable, Decodable {
  case admin
  case developer
  case consumer

  var title: String { rawValue.capitalized }
}

/// The rest of `[auth]`: the role new people get, and what they can do without signing in.
struct UserSettings: Equatable {
  var defaultRole = Role.developer
  var requireSignIn = false
  var allowAvatarUpload = true
}

extension UserSettings: SettingsSection {
  init(table: TOMLTable) {
    self.init()
    let auth = table["auth"]?.table
    defaultRole = auth?["default_role"]?.string.flatMap(Role.init(rawValue:)) ?? .developer
    requireSignIn = auth?["require_authentication"]?.bool ?? false
    allowAvatarUpload = auth?["allow_avatar_upload"]?.bool ?? true
  }

  /// Writes `default_role` even as Developer, because ricochet warns at every sign-in when it is absent.
  func apply(to table: TOMLTable) {
    let auth = table.section("auth")
    auth["default_role"] = defaultRole.rawValue
    auth.set("require_authentication", requireSignIn ? true : nil)
    auth.set("allow_avatar_upload", allowAvatarUpload ? nil : false)
  }

  var problem: String? { nil }
}
