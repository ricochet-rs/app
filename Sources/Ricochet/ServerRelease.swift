import Foundation

/// A ricochet-server release built for Apple Silicon.
struct ServerRelease: Sendable, Equatable {
    let version: String

    static func latest() async throws -> ServerRelease {
        struct Index: Decodable {
            let latest: String
        }
        let index = URL(string: "https://ricochet.rs/releases.json")!
        let (data, _) = try await URLSession.shared.data(from: index)
        let latest = try JSONDecoder().decode(Index.self, from: data).latest
        return ServerRelease(version: latest.hasPrefix("v") ? String(latest.dropFirst()) : latest)
    }

    /// The installed release, read from `ricochet-server --version`, which prints `ricochet 0.25.0 (2026-11-02)`.
    static func installed() async -> ServerRelease? {
        let binary = Paths.binary.path(percentEncoded: false)
        guard FileManager.default.isExecutableFile(atPath: binary),
            let result = try? await Command.run(binary, ["--version"]),
            result.status == 0
        else { return nil }
        let words = result.output.split(separator: " ")
        return words.count > 1 ? ServerRelease(version: String(words[1])) : nil
    }

    func isNewer(than other: ServerRelease) -> Bool {
        version.compare(other.version, options: .numeric) == .orderedDescending
    }

    /// Downloads this release and puts its binary at `Paths.binary`, replacing any older one.
    func install() async throws {
        let archive = URL(
            string: "https://s3.eu-central-003.backblazeb2.com/ricochet-server/\(version)/ricochet-\(version)-aarch64-macos.tar.gz"
        )!
        try await Archive.installExecutable(
            "ricochet-server", from: archive, to: Paths.binary, name: "ricochet-server \(version)")
    }
}
