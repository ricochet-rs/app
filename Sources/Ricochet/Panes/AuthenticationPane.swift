import SwiftUI

struct AuthenticationPane: View {
  @Binding var settings: AuthenticationSettings
  let redirectURL: String
  /// Whether saving signs people in with a different issuer than the one ricochet uses now.
  let changesIssuer: Bool
  /// Opens the Users pane on its People tab.
  let openPeople: () -> Void

  @State private var choseOwnProvider = false
  @State private var isConfiguringOwnProvider = false

  private var usesOwnProvider: Bool {
    !settings.managed
      && (choseOwnProvider || settings.issuerURL.nonEmpty != nil
        || settings.clientID.nonEmpty != nil)
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 20) {
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
      if isConfiguringOwnProvider {
        Button {
          isConfiguringOwnProvider = false
        } label: {
          Label("Back", systemImage: "chevron.left")
        }
        .buttonStyle(.basecoat(.link, small: true))
        ownProvider
      } else {
        VStack(spacing: 12) {
          ChoiceCard(
            symbol: "checkmark.shield", title: "Managed auth",
            message:
              "Sign in on this Mac with a ricochet account. You share your name and email address with ricochet.",
            isSelected: settings.managed, select: agree
          ) {
            HStack(spacing: 12) {
              Button(action: agree) {
                if settings.managed {
                  Label("Agreed", systemImage: "checkmark")
                } else {
                  Text("I agree")
                }
              }
              .buttonStyle(.basecoat(settings.managed ? .outline : .primary, small: true))
              Button("Privacy Policy") {
                _ = NSWorkspace.shared.open(AuthenticationSettings.privacyPolicy)
              }
              .buttonStyle(.basecoat(.link, small: true))
            }
          }
          ChoiceCard(
            symbol: "key", title: "Your own IdP",
            message: "Sign in through Okta, Entra ID, Keycloak or another OpenID Connect provider.",
            isSelected: usesOwnProvider, select: configureOwnProvider
          ) {
            Button(action: configureOwnProvider) {
              HStack(spacing: 6) {
                Text("Configure")
                Image(systemName: "chevron.right")
              }
            }
            .buttonStyle(.basecoat(.outline, small: true))
          }
        }
        .frame(maxWidth: 760)
      }
    }
  }

  private func agree() {
    settings.managed = true
  }

  private func configureOwnProvider() {
    settings.managed = false
    choseOwnProvider = true
    isConfiguringOwnProvider = true
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
  }
}

/// A large card for one way to sign in: clicking anywhere on it does what its button does.
private struct ChoiceCard<Actions: View>: View {
  let symbol: String
  let title: String
  let message: String
  let isSelected: Bool
  let select: () -> Void
  @ViewBuilder let actions: Actions

  @State private var isHovered = false

  var body: some View {
    VStack(spacing: 10) {
      Image(systemName: symbol)
        .font(.system(size: 28, weight: .light))
        .foregroundStyle(isSelected ? Theme.ring : Theme.foreground)
      Text(title)
        .font(.system(size: 15, weight: .semibold))
        .foregroundStyle(Theme.foreground)
      Text(message)
        .font(.system(size: 13))
        .foregroundStyle(Theme.mutedForeground)
        .multilineTextAlignment(.center)
        .fixedSize(horizontal: false, vertical: true)
      actions
        .padding(.top, 6)
    }
    .padding(20)
    .frame(maxWidth: .infinity, minHeight: 150)
    .background(isHovered && !isSelected ? Theme.muted : Theme.card)
    .overlay(
      Rectangle().strokeBorder(
        isSelected ? Theme.ring : Theme.border, lineWidth: isSelected ? 2 : 1)
    )
    .contentShape(Rectangle())
    .onTapGesture(perform: select)
    .onHover { isHovered = $0 }
  }
}
