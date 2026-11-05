import Foundation

struct CommandResult: Sendable {
    let status: Int32
    let output: String
}

enum Command {
    /// Runs an executable off the main actor and returns its exit status with stdout and stderr combined.
    /// `environment` adds to, and overrides, the app's own environment.
    static func run(
        _ executable: String, _ arguments: [String], environment: [String: String] = [:]
    ) async throws -> CommandResult {
        try await Task.detached {
            let process = Process()
            process.executableURL = URL(filePath: executable)
            process.arguments = arguments
            process.environment = ProcessInfo.processInfo.environment.merging(environment) { _, added in added }
            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = pipe
            try process.run()
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            return CommandResult(status: process.terminationStatus, output: String(decoding: data, as: UTF8.self))
        }.value
    }

    /// Runs an executable off the main actor and returns only its stdout, for output meant to be parsed.
    static func output(
        _ executable: String, _ arguments: [String], environment: [String: String] = [:]
    ) async throws -> Data {
        try await Task.detached {
            let process = Process()
            process.executableURL = URL(filePath: executable)
            process.arguments = arguments
            process.environment = ProcessInfo.processInfo.environment.merging(environment) { _, added in added }
            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = FileHandle.nullDevice
            try process.run()
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            return data
        }.value
    }
}
