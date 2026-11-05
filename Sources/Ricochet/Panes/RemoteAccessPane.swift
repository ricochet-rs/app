import AppKit
import SwiftUI

struct RemoteAccessPane: View {
    @Binding var settings: RemoteAccessSettings
    let localURL: String
    /// Opens the Users pane, where **Require sign-in for every page** lives.
    let showUsers: () -> Void
    @State var tailscale: Tailscale.Readiness?

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SelectField("Reach ricochet through", selection: $settings.mode, options: RemoteAccessMode.allCases) {
                $0.title
            }
            switch settings.mode {
            case .off:
                EmptyView()
            case .tailscale:
                tailscaleSteps
            case .cloudflare:
                cloudflareSteps
            case .custom:
                InputField(
                    "Public URL", text: $settings.customURL, prompt: "https://ricochet.example.com",
                    description: "Enter the https address of the tunnel or proxy you run in front of ricochet.")
            }
            if settings.mode != .off {
                Button("Require sign-in for every page", action: showUsers)
                    .buttonStyle(.basecoat(.link))
            }
        }
        .task(id: settings.mode) { await checkTailscale() }
    }

    @ViewBuilder private var tailscaleSteps: some View {
        if let tailscale {
            Field("Tailscale") {
                VStack(alignment: .leading, spacing: 8) {
                    Text(tailscale.message)
                        .font(.system(size: 14))
                        .foregroundStyle(Theme.foreground)
                        .fixedSize(horizontal: false, vertical: true)
                        .textSelection(.enabled)
                    if tailscale == .needsFunnel {
                        Text(Tailscale.funnelPolicy)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(Theme.foreground)
                            .textSelection(.enabled)
                            .padding(12)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(Theme.muted)
                    }
                    HStack(spacing: 8) {
                        if let link = tailscale.link {
                            Button(link.title) { _ = NSWorkspace.shared.open(link.url) }
                                .buttonStyle(.basecoat(.outline, small: true))
                        }
                        Button("Check again") { Task { await checkTailscale() } }
                            .buttonStyle(.basecoat(.ghost, small: true))
                    }
                }
            }
        } else {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("Checking Tailscale")
            }
            .font(.system(size: 13))
            .foregroundStyle(Theme.mutedForeground)
        }
    }

    @ViewBuilder private var cloudflareSteps: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("A Cloudflare account with a registered domain is required.")
            Text(cloudflaredNote)
                .tint(Theme.primary)
        }
        .font(.system(size: 14))
        .foregroundStyle(Theme.mutedForeground)
        .textSelection(.enabled)

        Step(number: 1, title: "Create a tunnel") {
            StepText("From the dashboard, navigate to **Networking > Tunnels > + Create Tunnel**.")
            Button("Open Cloudflare dashboard") {
                _ = NSWorkspace.shared.open(URL(string: "https://dash.cloudflare.com/")!)
            }
            .buttonStyle(.basecoat(.outline, small: true))
        }

        Step(number: 2, title: "Paste its token") {
            StepText("Paste the install command or just the token.")
            TextInput(text: $settings.cloudflareToken, prompt: "eyJhIjoi…", secure: true)
        }

        Step(number: 3, title: "Add a published application") {
            StepText(
                "In the tunnel, choose **Add a route > Published application**, paste the address below as its service, then enter the hostname you chose here."
            )
            CopyField("Service", value: localURL)
            InputField("Hostname", text: $settings.cloudflareHostname, prompt: "ricochet.example.com")
        }

        Step(number: 4, title: "Connect") {
            StepText("Press **Save and restart**. Ricochet starts `cloudflared`, and Cloudflare marks the tunnel **Healthy**.")
        }

        Text("Cloudflare's free plan caps uploads at 100MB.")
            .font(.system(size: 13))
            .foregroundStyle(Theme.mutedForeground)
            .textSelection(.enabled)
    }

    /// "**Note**: `cloudflared` will be installed if not found", with `cloudflared` as inline code linking to its repository.
    private var cloudflaredNote: AttributedString {
        var note = AttributedString("Note")
        note.font = .system(size: 14, weight: .semibold)
        var name = AttributedString("cloudflared")
        name.font = .system(size: 13, design: .monospaced)
        name.backgroundColor = Theme.muted
        name.link = URL(string: "https://github.com/cloudflare/cloudflared")
        name.underlineStyle = .single
        return note + AttributedString(": ") + name + AttributedString(" will be installed if not found.")
    }

    /// Asks Tailscale what still stands in the way, and keeps the `ts.net` name once nothing does.
    private func checkTailscale() async {
        guard settings.mode == .tailscale else { return }
        tailscale = nil
        let readiness = await Tailscale.readiness()
        tailscale = readiness
        if case .ready(let hostname) = readiness { settings.tailscaleHostname = hostname }
    }
}

/// One numbered step of a setup you finish outside the app.
private struct Step<Content: View>: View {
    let number: Int
    let title: String
    @ViewBuilder let content: Content

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Text(String(number))
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(Theme.foreground)
                .frame(width: 22, height: 22)
                .background(Theme.muted)
            VStack(alignment: .leading, spacing: 8) {
                Text(title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(Theme.foreground)
                    .textSelection(.enabled)
                content
            }
        }
    }
}

/// A sentence in a step, with Markdown for bold labels, selectable so you can copy it.
private struct StepText: View {
    let text: LocalizedStringKey

    init(_ text: LocalizedStringKey) {
        self.text = text
    }

    var body: some View {
        Text(text)
            .font(.system(size: 14))
            .foregroundStyle(Theme.foreground)
            .fixedSize(horizontal: false, vertical: true)
            .textSelection(.enabled)
    }
}
