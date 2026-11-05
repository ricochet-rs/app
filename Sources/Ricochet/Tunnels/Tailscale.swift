import Foundation

/// Tailscale Funnel, which publishes ricochet at this Mac's `ts.net` name through the Tailscale app.
enum Tailscale {
    /// The CLI inside Tailscale.app, or the standalone one.
    static var cli: String? {
        [
            "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
            "/opt/homebrew/bin/tailscale",
            "/usr/local/bin/tailscale",
        ].first { FileManager.default.isExecutableFile(atPath: $0) }
    }

    /// Without this, the binary inside Tailscale.app opens Tailscale's window whenever no terminal launched it.
    private static let cliEnvironment = ["TAILSCALE_BE_CLI": "1"]

    /// What still stands between this Mac and a working funnel.
    enum Readiness: Equatable {
        case notInstalled
        case signedOut
        case selfHosted
        case needsHTTPS
        case needsFunnel
        case ready(hostname: String)

        var message: String {
            switch self {
            case .notInstalled: "Install Tailscale on this Mac and sign in."
            case .signedOut: "Open Tailscale and sign in."
            case .selfHosted:
                "You run your tailnet on your own control server, such as Headscale, and Tailscale only offers Funnel on its own servers. Choose Cloudflare Tunnel instead."
            case .needsHTTPS: "Turn on HTTPS certificates in your tailnet's DNS settings."
            case .needsFunnel: "Allow Funnel for this Mac by adding this to your tailnet's access controls:"
            case .ready(let hostname): "Ricochet will answer at https://\(hostname)."
            }
        }

        var link: (title: String, url: URL)? {
            switch self {
            case .notInstalled: ("Download Tailscale", URL(string: "https://tailscale.com/download/mac")!)
            case .signedOut: ("Open Tailscale", URL(filePath: "/Applications/Tailscale.app"))
            case .needsHTTPS: ("Open DNS settings", URL(string: "https://login.tailscale.com/admin/dns")!)
            case .needsFunnel: ("Open access controls", URL(string: "https://login.tailscale.com/admin/acls/file")!)
            case .selfHosted, .ready: nil
            }
        }
    }

    /// The access control entry that lets every member's devices use Funnel.
    static let funnelPolicy = """
        "nodeAttrs": [
          { "target": ["autogroup:member"], "attr": ["funnel"] }
        ]
        """

    /// The fields of `tailscale status --json` that decide readiness.
    private struct Status: Decodable {
        struct Node: Decodable {
            let dnsName: String
            let capMap: [String: Ignored]?

            enum CodingKeys: String, CodingKey {
                case dnsName = "DNSName"
                case capMap = "CapMap"
            }
        }

        /// Any JSON value, read only so its key can be checked.
        struct Ignored: Decodable {
            init(from decoder: Decoder) throws {}
        }

        let backendState: String
        let node: Node?
        let certDomains: [String]?

        enum CodingKeys: String, CodingKey {
            case backendState = "BackendState"
            case node = "Self"
            case certDomains = "CertDomains"
        }
    }

    /// The control server from `tailscale debug prefs`, which is Tailscale's own unless you run Headscale.
    private struct Prefs: Decodable {
        let controlURL: String?

        enum CodingKeys: String, CodingKey {
            case controlURL = "ControlURL"
        }
    }

    static func readiness() async -> Readiness {
        guard let cli else { return .notInstalled }
        guard let status = try? await Command.output(cli, ["status", "--json"], environment: cliEnvironment)
        else { return .signedOut }
        let prefs = (try? await Command.output(cli, ["debug", "prefs"], environment: cliEnvironment)) ?? Data()
        return readiness(status: status, prefs: prefs)
    }

    static func readiness(status: Data, prefs: Data) -> Readiness {
        guard let status = try? JSONDecoder().decode(Status.self, from: status),
            status.backendState == "Running", let node = status.node
        else { return .signedOut }
        let control = (try? JSONDecoder().decode(Prefs.self, from: prefs))?.controlURL.flatMap { URL(string: $0)?.host() }
        if let control, !control.hasSuffix("tailscale.com") { return .selfHosted }
        guard status.certDomains?.isEmpty == false else { return .needsHTTPS }
        guard node.capMap?.keys.contains(where: { $0.contains("funnel") }) == true else { return .needsFunnel }
        return .ready(hostname: node.dnsName.hasSuffix(".") ? String(node.dnsName.dropLast()) : node.dnsName)
    }

    /// Publishes ricochet's `port` on this Mac's `ts.net` name, replacing whatever port Funnel served there before.
    static func startFunnel(port: UInt16) async throws {
        guard let cli else { throw AppError.tailscale(Readiness.notInstalled.message) }
        let result = try await Command.run(cli, ["funnel", "--bg", String(port)], environment: cliEnvironment)
        guard result.status == 0 else { throw AppError.tailscale(result.output.trimmed) }
    }

    /// Turns off only the funnel on port 443, never `funnel reset`, which would also remove funnels you set up yourself.
    static func stopFunnel() async {
        guard let cli else { return }
        _ = try? await Command.run(cli, ["funnel", "--https=443", "off"], environment: cliEnvironment)
    }
}
