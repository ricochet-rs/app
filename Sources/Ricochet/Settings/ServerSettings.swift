import TOMLKit

/// The port ricochet serves on and the license it runs under.
struct ServerSettings: Equatable {
    var port = "6188"
    var licenseKey = ""

    var proxyPort: UInt16? {
        UInt16(port.trimmed).flatMap { $0 > 0 ? $0 : nil }
    }
}

extension ServerSettings: SettingsSection {
    init(table: TOMLTable) {
        self.init()
        if let saved = table["network"]?.table?["proxy_port"]?.int { port = String(saved) }
        licenseKey = table["license"]?.table?["key"]?.string ?? ""
    }

    func apply(to table: TOMLTable) {
        table.section("license").set("key", licenseKey.nonEmpty)
        table.removeIfEmpty("license")
    }

    var problem: String? {
        proxyPort == nil ? "Enter a port between 1 and 65535." : nil
    }
}
