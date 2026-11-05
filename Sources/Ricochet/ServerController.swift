import AppKit
import Observation

enum ServerStatus {
    case notInstalled
    case stopped
    case starting
    case running
}

struct Failure: Equatable {
    let title: String
    let message: String
}

/// Installs ricochet-server, starts and stops it, and reports whether it answers.
@MainActor @Observable
final class ServerController {
    private(set) var status = ServerStatus.stopped
    private(set) var installed: ServerRelease?
    private(set) var latest: ServerRelease?
    private(set) var activity: String?
    private(set) var failure: Failure?
    private(set) var isConfigured = FileManager.default.fileExists(atPath: Paths.config.path(percentEncoded: false))
    /// Whether ricochet answers at its public address, or `nil` until the first check.
    private(set) var publicReachable: Bool?

    private static let portKey = "proxyPort"
    private static let urlKey = "url"

    /// Where people open ricochet, which is the public URL when the setup form set one.
    var dashboard: URL {
        UserDefaults.standard.string(forKey: Self.urlKey).flatMap { URL(string: $0) } ?? localURL
    }

    /// Where people outside this Mac reach ricochet, or `nil` when only this Mac does.
    var publicURL: URL? {
        dashboard.host() == "localhost" ? nil : dashboard
    }

    /// Where this Mac reaches ricochet directly, whatever tunnel sits in front of it.
    private var localURL: URL {
        let port = UserDefaults.standard.integer(forKey: Self.portKey)
        return URL(string: "http://localhost:\(port == 0 ? 6188 : port)")!
    }

    var availableUpdate: ServerRelease? {
        guard let latest, let installed, latest.isNewer(than: installed) else { return nil }
        return latest
    }

    init() {
        Task { await self.watch() }
    }

    func setUp(_ config: ServerConfig) async {
        var config = config
        await perform("Checking your settings", failing: "Ricochet didn't finish setting up") {
            guard let proxyPort = config.server.proxyPort else { throw AppError.portInUse(0) }
            // Ricochet itself holds the port it already uses, so only a new port can belong to another app.
            if Int(proxyPort) != UserDefaults.standard.integer(forKey: Self.portKey), !NetworkPorts.isFree(proxyPort) {
                throw AppError.portInUse(proxyPort)
            }
            if config.remoteAccess.mode == .tailscale {
                activity = "Checking Tailscale"
                let readiness = await Tailscale.readiness()
                guard case .ready(let hostname) = readiness else { throw AppError.tailscale(readiness.message) }
                config.remoteAccess.tailscaleHostname = hostname
            }
            activity = "Stopping ricochet"
            try await LaunchAgent.server.stop()
            if installed == nil {
                let release = try await ServerRelease.latest()
                activity = "Downloading ricochet \(release.version)"
                try await release.install()
                installed = await ServerRelease.installed()
            }

            activity = "Writing your configuration"
            try FileManager.default.createDirectory(
                at: Paths.home, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            try installRenv()
            try config.save(proxyPort: proxyPort, to: Paths.config)
            UserDefaults.standard.set(Int(proxyPort), forKey: Self.portKey)
            try LaunchAgent.writeServerJob()

            activity = "Starting ricochet"
            try await LaunchAgent.server.start()
            try await waitUntilRunning()
            isConfigured = true

            activity = "Connecting remote access"
            let previous = RemoteAccessSettings.load(from: Paths.appSupport)?.mode ?? .off
            try config.remoteAccess.save(to: Paths.appSupport)
            try await config.remoteAccess.connect(replacing: previous, port: proxyPort)
            UserDefaults.standard.set(config.url, forKey: Self.urlKey)
            publicReachable = nil
            _ = NSWorkspace.shared.open(dashboard)
        }
    }

    func start() async {
        await perform("Starting ricochet", failing: "Ricochet didn't start") {
            try await LaunchAgent.server.start()
            try await waitUntilRunning()
        }
    }

    func stop() async {
        await perform("Stopping ricochet", failing: "Ricochet didn't stop") {
            try await LaunchAgent.server.stop()
        }
    }

    func restart() async {
        await perform("Restarting ricochet", failing: "Ricochet didn't restart") {
            try await LaunchAgent.server.restart()
            try await waitUntilRunning()
        }
    }

    func update(to release: ServerRelease) async {
        await perform("Updating to ricochet \(release.version)", failing: "Ricochet didn't finish updating") {
            try await release.install()
            try installRenv()
            installed = await ServerRelease.installed()
            if await LaunchAgent.server.isLoaded() {
                try await LaunchAgent.server.restart()
                try await waitUntilRunning()
            }
        }
    }

    func dismissFailure() {
        failure = nil
    }

    /// Downloads the latest ricochet-server without configuring or starting it, which **Install and start** does later.
    func install() async {
        await perform("Checking for the latest ricochet", failing: "This app couldn't install ricochet") {
            let release = try await ServerRelease.latest()
            activity = "Downloading ricochet \(release.version)"
            try await release.install()
            installed = await ServerRelease.installed()
        }
    }

    /// Stops everything the app started and moves ricochet and its data to the Trash, keeping this app for a fresh setup.
    func uninstall() async {
        await perform("Uninstalling ricochet", failing: "Ricochet didn't finish uninstalling") {
            if RemoteAccessSettings.load(from: Paths.appSupport)?.mode == .tailscale { await Tailscale.stopFunnel() }
            for agent in [LaunchAgent.tunnel, LaunchAgent.server]
            where FileManager.default.fileExists(atPath: agent.plist.path(percentEncoded: false)) {
                try await agent.remove()
            }
            for item in [Paths.home, Paths.appSupport, Paths.binary, Paths.cloudflared]
            where FileManager.default.fileExists(atPath: item.path(percentEncoded: false)) {
                try FileManager.default.trashItem(at: item, resultingItemURL: nil)
            }
            UserDefaults.standard.removeObject(forKey: Self.portKey)
            UserDefaults.standard.removeObject(forKey: Self.urlKey)
            installed = nil
            isConfigured = false
            publicReachable = nil
        }
    }

    private func perform(_ activity: String, failing title: String, _ work: () async throws -> Void) async {
        self.activity = activity
        failure = nil
        do {
            try await work()
        } catch {
            failure = Failure(title: title, message: error.localizedDescription)
            let entry = "\(Date.now.ISO8601Format()) \(title) while \(self.activity ?? activity): \(error)\n"
            try? FileManager.default.createDirectory(at: Paths.appSupport, withIntermediateDirectories: true)
            if let log = try? FileHandle(forWritingTo: Paths.appLog) {
                _ = try? log.seekToEnd()
                try? log.write(contentsOf: Data(entry.utf8))
                try? log.close()
            } else {
                FileManager.default.createFile(atPath: Paths.appLog.path(percentEncoded: false), contents: Data(entry.utf8))
            }
        }
        self.activity = nil
        await refreshStatus()
    }

    private func watch() async {
        installed = await ServerRelease.installed()
        if isConfigured {
            try? installRenv()
            await perform("Restarting ricochet", failing: "Ricochet didn't restart") {
                guard try LaunchAgent.writeServerJob(), await LaunchAgent.server.isLoaded() else { return }
                try await LaunchAgent.server.stop()
                try await LaunchAgent.server.start()
                try await waitUntilRunning()
            }
        }
        var checkedReleases = Date.distantPast
        var checkedPublic = Date.distantPast
        while !Task.isCancelled {
            if checkedReleases.timeIntervalSinceNow < -12 * 60 * 60 {
                latest = try? await ServerRelease.latest()
                checkedReleases = .now
            }
            if activity == nil { await refreshStatus() }
            if let publicURL, checkedPublic.timeIntervalSinceNow < -30 {
                publicReachable = await answers(at: publicURL, within: 5)
                checkedPublic = .now
            }
            try? await Task.sleep(for: .seconds(3))
        }
    }

    private func refreshStatus() async {
        if installed == nil {
            status = .notInstalled
        } else if await answers(at: localURL, within: 2) {
            status = .running
        } else if await LaunchAgent.server.isLoaded() {
            status = .starting
        } else {
            status = .stopped
        }
    }

    /// Whether ricochet's `/health` at `url` returns 200 within `seconds`.
    private func answers(at url: URL, within seconds: TimeInterval) async -> Bool {
        let request = URLRequest(
            url: url.appending(path: "health"), cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: seconds)
        guard let (_, response) = try? await URLSession.shared.data(for: request) else { return false }
        return (response as? HTTPURLResponse)?.statusCode == 200
    }

    private func waitUntilRunning() async throws {
        for _ in 0..<60 {
            if await answers(at: localURL, within: 2) { return }
            try await Task.sleep(for: .seconds(1))
        }
        let log = (try? String(contentsOf: Paths.stderr, encoding: .utf8)) ?? ""
        throw AppError.didNotStart(log.split(separator: "\n").suffix(8).joined(separator: "\n"))
    }

    /// Copies the renv bundled with this app to where ricochet-server installs it from.
    private func installRenv() throws {
        guard let bundled = Bundle.main.url(forResource: "renv", withExtension: "tar.gz") else { return }
        let manager = FileManager.default
        if manager.contentsEqual(
            atPath: bundled.path(percentEncoded: false), andPath: Paths.renv.path(percentEncoded: false))
        {
            return
        }
        try manager.createDirectory(at: Paths.renv.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? manager.removeItem(at: Paths.renv)
        try manager.copyItem(at: bundled, to: Paths.renv)
    }
}
