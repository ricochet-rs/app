import Foundation

enum Archive {
    /// Downloads the `.tar.gz` at `url` and installs the executable `member` from it at `destination`.
    static func installExecutable(_ member: String, from url: URL, to destination: URL, name: String) async throws {
        let (download, response) = try await URLSession.shared.download(from: url)
        defer { try? FileManager.default.removeItem(at: download) }
        guard (response as? HTTPURLResponse)?.statusCode == 200 else { throw AppError.download(name) }

        let staging = FileManager.default.temporaryDirectory.appending(path: UUID().uuidString)
        try FileManager.default.createDirectory(at: staging, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: staging) }
        let tar = try await Command.run(
            "/usr/bin/tar",
            ["-xzf", download.path(percentEncoded: false), "-C", staging.path(percentEncoded: false)])
        guard tar.status == 0 else { throw AppError.download(name) }

        try FileManager.default.createDirectory(
            at: destination.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? FileManager.default.removeItem(at: destination)
        try FileManager.default.moveItem(at: staging.appending(path: member), to: destination)
        try FileManager.default.setAttributes(
            [.posixPermissions: 0o755], ofItemAtPath: destination.path(percentEncoded: false))
    }
}
