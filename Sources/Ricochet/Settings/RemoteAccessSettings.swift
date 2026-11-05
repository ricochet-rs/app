import Foundation

enum RemoteAccessMode: String, Codable, CaseIterable, Hashable {
    case off
    case tailscale
    case cloudflare
    case custom

    var title: String {
        switch self {
        case .off: "Only this Mac"
        case .tailscale: "Tailscale Funnel"
        case .cloudflare: "Cloudflare Tunnel"
        case .custom: "Your own address"
        }
    }
}

/// How people outside this Mac reach ricochet.
/// The app keeps these itself, because ricochet only reads the `url` they produce.
struct RemoteAccessSettings: Codable, Equatable {
    var mode = RemoteAccessMode.off
    var tailscaleHostname = ""
    var cloudflareHostname = ""
    var customURL = ""
    var cloudflareToken = ""

    static let fileName = "remote-access.json"
    static let tokenFileName = "cloudflared-token"

    private enum CodingKeys: String, CodingKey {
        case mode, tailscaleHostname, cloudflareHostname, customURL
    }

    /// The address people open ricochet at, or `local` when nothing outside this Mac reaches it.
    func url(local: String) -> String {
        switch mode {
        case .off: local
        case .tailscale: tailscaleHostname.nonEmpty.map { "https://\($0)" } ?? local
        case .cloudflare: cloudflareHost.map { "https://\($0)" } ?? local
        case .custom: customURL.nonEmpty.map { $0.hasSuffix("/") ? String($0.dropLast()) : $0 } ?? local
        }
    }

    /// The tunnel token alone, even when someone pasted the whole install command Cloudflare shows around it.
    var tunnelToken: String? {
        cloudflareToken.split(whereSeparator: \.isWhitespace).last.map(String.init)
    }

    /// The Cloudflare hostname without a scheme or trailing slash pasted along with it.
    private var cloudflareHost: String? {
        var host = cloudflareHostname.trimmed
        for scheme in ["https://", "http://"] where host.hasPrefix(scheme) { host.removeFirst(scheme.count) }
        while host.hasSuffix("/") { host.removeLast() }
        return host.isEmpty ? nil : host
    }

    var problem: String? {
        switch mode {
        case .off:
            return nil
        case .tailscale:
            return tailscaleHostname.nonEmpty == nil ? "Finish the Tailscale steps on the Remote access pane." : nil
        case .cloudflare:
            guard let host = cloudflareHost, host.contains("."), !host.contains("/") else {
                return "Enter the hostname you gave the tunnel, such as ricochet.example.com."
            }
            return tunnelToken == nil ? "Paste the tunnel token from the Cloudflare dashboard." : nil
        case .custom:
            guard let customURL = customURL.nonEmpty, let origin = URLComponents(string: customURL),
                let scheme = origin.scheme, scheme == "http" || scheme == "https",
                let host = origin.host, !host.isEmpty,
                origin.user == nil, origin.password == nil,
                origin.path.isEmpty || origin.path == "/",
                origin.query == nil, origin.fragment == nil
            else {
                return "Enter just the address, like https://ricochet.example.com, with nothing after it."
            }
            return nil
        }
    }

    /// The settings saved in `folder`, or `nil` before the first save.
    static func load(from folder: URL) -> RemoteAccessSettings? {
        guard let data = try? Data(contentsOf: folder.appending(path: fileName)),
            var settings = try? JSONDecoder().decode(Self.self, from: data)
        else { return nil }
        settings.cloudflareToken =
            (try? String(contentsOf: folder.appending(path: tokenFileName), encoding: .utf8)) ?? ""
        return settings
    }

    /// Saves these settings in `folder`, with the Cloudflare token in a file of its own, both readable only by you.
    func save(to folder: URL) throws {
        try FileManager.default.createDirectory(
            at: folder, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        try Self.writePrivately(JSONEncoder().encode(self), to: folder.appending(path: Self.fileName))
        let token = folder.appending(path: Self.tokenFileName)
        if mode == .cloudflare, let value = tunnelToken {
            try Self.writePrivately(Data(value.utf8), to: token)
        } else {
            try? FileManager.default.removeItem(at: token)
        }
    }

    private static func writePrivately(_ data: Data, to file: URL) throws {
        guard
            FileManager.default.createFile(
                atPath: file.path(percentEncoded: false), contents: data, attributes: [.posixPermissions: 0o600])
        else { throw CocoaError(.fileWriteUnknown) }
    }

    /// Turns off the tunnel the `previous` mode started, then starts this mode's tunnel for ricochet on `port`.
    func connect(replacing previous: RemoteAccessMode, port: UInt16) async throws {
        if previous == .tailscale, mode != .tailscale { await Tailscale.stopFunnel() }
        if previous == .cloudflare, mode != .cloudflare { try await CloudflareTunnel.stop() }
        switch mode {
        case .tailscale: try await Tailscale.startFunnel(port: port)
        case .cloudflare: try await CloudflareTunnel.start()
        case .off, .custom: break
        }
    }
}
