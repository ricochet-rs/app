import SwiftUI

struct EmailPane: View {
  @Binding var settings: EmailSettings

  private var defaultPort: Int {
    switch settings.encryption {
    case .starttls: 587
    case .tls: 465
    case .none: 25
    }
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      ToggleField(
        "Send email", isOn: $settings.enabled,
        description: "Ricochet sends no email until you turn this on.")
      if settings.enabled {
        InputField("Host", text: $settings.host, prompt: "smtp.example.com")
        SelectField("Encryption", selection: $settings.encryption, options: Encryption.allCases) {
          $0.title
        }
        InputField(
          "Port", text: $settings.port, prompt: String(defaultPort),
          description: "Leave this empty to use \(defaultPort).")
        InputField("Username", text: $settings.username)
        InputField("Password", text: $settings.password, secure: true)
        InputField("From", text: $settings.from, prompt: "ricochet <ricochet@example.com>")
        InputField(
          "Reply-to", text: $settings.replyTo,
          description: "Leave this empty and people reply to the sender.")
      }
    }
  }
}
