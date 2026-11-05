import Foundation

/// Cloudflare Tunnel, which runs `cloudflared` as its own launch agent outside ricochet's sandbox.
enum CloudflareTunnel {
  private static let archive = URL(
    string:
      "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-arm64.tgz"
  )!

  /// Installs `cloudflared` when it is missing and runs it with the saved token, restarting it so a new token applies.
  static func start() async throws {
    if !FileManager.default.isExecutableFile(atPath: Paths.cloudflared.path(percentEncoded: false))
    {
      try await Archive.installExecutable(
        "cloudflared", from: archive, to: Paths.cloudflared, name: "cloudflared")
    }
    try LaunchAgent.tunnel.write([
      "ProgramArguments": [
        Paths.cloudflared.path(percentEncoded: false), "tunnel", "--no-autoupdate", "run",
        "--token-file", Paths.cloudflaredToken.path(percentEncoded: false),
      ],
      "RunAtLoad": true,
      "KeepAlive": true,
      "StandardOutPath": Paths.cloudflaredLog.path(percentEncoded: false),
      "StandardErrorPath": Paths.cloudflaredLog.path(percentEncoded: false),
    ])
    if await LaunchAgent.tunnel.isLoaded() {
      try await LaunchAgent.tunnel.restart()
    } else {
      try await LaunchAgent.tunnel.start()
    }
  }

  static func stop() async throws {
    try await LaunchAgent.tunnel.remove()
  }
}
