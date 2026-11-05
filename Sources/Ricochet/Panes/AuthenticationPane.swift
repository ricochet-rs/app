import SwiftUI

struct AuthenticationPane: View {
  @Binding var settings: AuthenticationSettings
  let redirectURL: String
  /// Whether saving signs people in with a different issuer than the one ricochet uses now.
  let changesIssuer: Bool
  /// Opens the Users pane on its People tab.
  let openPeople: () -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      if changesIssuer {
        Callout(
          symbol: "exclamationmark.triangle.fill", tint: Theme.warning,
          title: "You lose access to your current accounts",
          message: "After signing in, change your role in the Users tab."
        ) {
          Button("Open Users", action: openPeople)
            .buttonStyle(.basecoat(.outline, small: true))
        }
      }
      if settings.managed {
        Callout(
          symbol: "checkmark.shield.fill", tint: Theme.success,
          title: "Using ricochet managed auth.",
          message: "Sign in on this Mac with a ricochet account.")
        Button("Configure your own identity provider") { settings.managed = false }
          .buttonStyle(.basecoat(.outline))
      } else {
        ownProvider
      }
    }
  }

  @ViewBuilder private var ownProvider: some View {
    Tabs(selection: $settings.provider, options: Provider.allCases) { $0.title }
    InputField("Issuer URL", text: $settings.issuerURL, prompt: settings.provider.issuerPrompt)
    InputField("Client ID", text: $settings.clientID)
    InputField(
      "Client secret", text: $settings.clientSecret,
      description: settings.provider == .oidc
        ? "Leave this empty if you registered the app without a secret." : nil,
      secure: true)
    CopyField(
      "Redirect URL", value: redirectURL,
      description: "Add this URL to the app you registered. It changes when you change the port.")
    InputField(
      "Display name claim", text: $settings.displayNameClaim, prompt: "preferred_username",
      description: "Ricochet shows this claim as each person's name.")
    InputField(
      "Groups claim", text: $settings.groupsClaim, prompt: "groups",
      description: "Leave this empty and ricochet ignores groups from your identity provider.")
    InputField(
      "Group sync interval", text: $settings.syncInterval, prompt: "Minutes",
      description:
        "How often ricochet refreshes groups, in minutes. Leave this empty and ricochet refreshes them only when someone signs in."
    )
    Button("Use ricochet managed auth") { settings.managed = true }
      .buttonStyle(.basecoat(.link))
  }
}
