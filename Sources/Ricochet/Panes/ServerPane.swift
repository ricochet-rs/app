import SwiftUI

struct ServerPane: View {
    @Binding var settings: ServerSettings
    let localURL: String
    let isInstalled: Bool
    let install: () -> Void
    let uninstall: () -> Void
    @State var confirmingUninstall = false

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            InputField("Port", text: $settings.port, description: "On this Mac, ricochet answers at \(localURL).")
            InputField(
                "License key", text: $settings.licenseKey,
                description: "Leave this empty to use the Community Edition.")
            Group {
                if isInstalled {
                    Button("Uninstall ricochet") { confirmingUninstall = true }
                        .buttonStyle(.basecoat(.destructive))
                } else {
                    Button("Install ricochet", action: install)
                        .buttonStyle(.basecoat())
                }
            }
            .padding(.top, 16)
        }
        .confirmationDialog("Uninstall ricochet?", isPresented: $confirmingUninstall, titleVisibility: .visible) {
            Button("Uninstall", role: .destructive, action: uninstall)
            Button("Cancel", role: .cancel) {}
        } message: {
            Text(
                "This app stops ricochet and moves it, with its content, database, caches and encryption keys, to the Trash. You can restore them until you empty the Trash."
            )
        }
    }
}
