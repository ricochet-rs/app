import TOMLKit

/// `[tasks]` and `[restore]`: how long a run or a package install may take, in minutes on screen and seconds on disk.
struct TaskSettings: Equatable {
    var maxDuration = ""
    var restoreTimeout = ""
    var restoreConcurrency = ""

    private static func minutes(_ seconds: Int?) -> String {
        guard let seconds else { return "" }
        return seconds % 60 == 0 ? String(seconds / 60) : String(Double(seconds) / 60)
    }

    private static func seconds(_ minutes: String) -> Int? {
        minutes.nonEmpty.flatMap(Double.init).map { Int(($0 * 60).rounded()) }
    }
}

extension TaskSettings: SettingsSection {
    init(table: TOMLTable) {
        self.init()
        maxDuration = Self.minutes(table["tasks"]?.table?["max_duration"]?.int)
        let restore = table["restore"]?.table
        restoreTimeout = Self.minutes(restore?["timeout"]?.int)
        restoreConcurrency = restore?["concurrency"]?.int.map(String.init) ?? ""
    }

    func apply(to table: TOMLTable) {
        table.section("tasks").set("max_duration", Self.seconds(maxDuration))
        table.removeIfEmpty("tasks")
        let restore = table.section("restore")
        restore.set("timeout", Self.seconds(restoreTimeout))
        restore.set("concurrency", restoreConcurrency.wholeNumber)
        table.removeIfEmpty("restore")
    }

    var problem: String? {
        for minutes in [maxDuration, restoreTimeout] where minutes.nonEmpty != nil {
            guard let seconds = Self.seconds(minutes), seconds > 0, seconds <= Int(UInt32.max) else {
                return "Enter time limits as a number of minutes greater than 0."
            }
        }
        if let concurrency = restoreConcurrency.nonEmpty, UInt16(concurrency).map({ $0 > 0 }) != true {
            return "Enter how many package installs ricochet runs at once, from 1 to 65535."
        }
        return nil
    }
}
