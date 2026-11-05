import TOMLKit

enum LogLevel: String, CaseIterable, Hashable {
    case trace
    case debug
    case info
    case warn
    case error

    var title: String {
        switch self {
        case .trace: "Trace"
        case .debug: "Debug"
        case .info: "Info"
        case .warn: "Warning"
        case .error: "Error"
        }
    }
}

enum LogRotation: String, CaseIterable, Hashable {
    case everyMinute = "everyminute"
    case hourly
    case daily
    case never

    var title: String {
        switch self {
        case .everyMinute: "Every minute"
        case .hourly: "Hourly"
        case .daily: "Daily"
        case .never: "Never"
        }
    }
}

/// `[logs]`. File logging is always on, because launchd never rotates the stdout file it writes.
struct LoggingSettings: Equatable {
    var level = LogLevel.info
    var rotation = LogRotation.daily
    var compress = true
    var maxFiles = ""
    var maxAgeDays = ""
}

extension LoggingSettings: SettingsSection {
    init(table: TOMLTable) {
        self.init()
        let logs = table["logs"]?.table
        level = logs?["level"]?.string.flatMap(LogLevel.init(rawValue:)) ?? .info
        rotation = logs?["rotation"]?.string.flatMap(LogRotation.init(rawValue:)) ?? .daily
        compress = logs?["compress"]?.bool ?? true
        maxFiles = logs?["max_files"]?.int.map(String.init) ?? ""
        maxAgeDays = logs?["max_age_days"]?.int.map(String.init) ?? ""
    }

    func apply(to table: TOMLTable) {
        let logs = table.section("logs")
        logs["file_logging"] = true
        logs.set("level", level == .info ? nil : level.rawValue)
        logs.set("rotation", rotation == .daily ? nil : rotation.rawValue)
        logs.set("compress", compress ? nil : false)
        logs.set("max_files", maxFiles.wholeNumber)
        logs.set("max_age_days", maxAgeDays.wholeNumber)
    }

    var problem: String? {
        if !maxFiles.isEmptyOrPositive { return "Enter how many log files to keep as a whole number." }
        if !maxAgeDays.isEmptyOrPositive { return "Enter the log age limit as a whole number of days." }
        return nil
    }
}
