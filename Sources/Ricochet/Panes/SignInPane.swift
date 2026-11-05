import SwiftUI

struct SignInPane: View {
  @Binding var settings: SignInSettings
  let redirectURL: String

  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      providerTabs
      InputField("Issuer URL", text: $settings.issuerURL, prompt: settings.provider.issuerPrompt)
      InputField("Client ID", text: $settings.clientID)
      InputField(
        "Client secret", text: $settings.clientSecret,
        description: settings.provider == .oidc
          ? "Leave this empty if you registered the app without a secret." : nil,
        secure: true)
      CopyField(
        "Redirect URL", value: redirectURL,
        description:
          "Add this URL to the app you registered. It changes when you change Remote access.")
    }
  }

  /// Basecoat's boxed `tabs`, one per provider.
  private var providerTabs: some View {
    HStack(spacing: 0) {
      ForEach(Provider.allCases, id: \.self) { provider in
        let selected = settings.provider == provider
        Button {
          settings.provider = provider
        } label: {
          Text(provider.title)
            .font(.system(size: 14, weight: .medium))
            .foregroundStyle(selected ? Theme.foreground : Theme.mutedForeground)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(selected ? Theme.background : .clear)
            .shadow(color: .black.opacity(selected ? 0.06 : 0), radius: 1, y: 1)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
      }
    }
    .padding(3)
    .frame(height: 36)
    .background(Theme.muted)
  }
}
