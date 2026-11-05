import TOMLKit

/// `[otel]`, the OpenTelemetry collector ricochet sends traces to.
struct TelemetrySettings: Equatable {
    var enabled = false
    var endpoint = ""
}

extension TelemetrySettings: SettingsSection {
    init(table: TOMLTable) {
        self.init()
        guard let saved = table["otel"]?.table?["endpoint"]?.string else { return }
        enabled = true
        endpoint = saved
    }

    func apply(to table: TOMLTable) {
        if enabled {
            table.section("otel")["endpoint"] = endpoint.trimmed
        } else {
            table.remove(at: "otel")
        }
    }

    var problem: String? {
        enabled && !endpoint.isHTTPURL ? "Enter the collector's URL, such as http://localhost:4317." : nil
    }
}
