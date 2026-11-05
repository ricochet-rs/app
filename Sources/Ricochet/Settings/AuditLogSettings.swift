import TOMLKit

/// `[audit]`: how long ricochet keeps its record of who changed what.
struct AuditLogSettings: Equatable {
  var keepForever = false
  var retentionDays = ""
  var writeFile = false
}

extension AuditLogSettings: SettingsSection {
  init(table: TOMLTable) {
    self.init()
    let audit = table["audit"]?.table
    keepForever = audit?["retention_days"]?.string == "forever"
    retentionDays = audit?["retention_days"]?.int.map(String.init) ?? ""
    writeFile = audit?["file_logging"]?.bool ?? false
  }

  func apply(to table: TOMLTable) {
    let audit = table.section("audit")
    if keepForever {
      audit["retention_days"] = "forever"
    } else {
      audit.set("retention_days", retentionDays.wholeNumber)
    }
    audit.set("file_logging", writeFile ? true : nil)
    table.removeIfEmpty("audit")
  }

  var problem: String? {
    keepForever || retentionDays.isEmptyOrPositive
      ? nil : "Enter how long to keep audit records as a whole number of days."
  }
}
