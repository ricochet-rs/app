import TOMLKit

/// `[retention]`: how many old deployments ricochet keeps, and how large a bundle may be.
struct DeploymentSettings: Equatable {
  var maxDeployments = ""
  var maxAgeDays = ""
  var maxBundleSize = ""
}

extension DeploymentSettings: SettingsSection {
  init(table: TOMLTable) {
    self.init()
    let retention = table["retention"]?.table
    maxDeployments = retention?["max_deployments"]?.int.map(String.init) ?? ""
    maxAgeDays = retention?["max_age_days"]?.int.map(String.init) ?? ""
    maxBundleSize =
      retention?["max_bundle_size"]?.string ?? retention?["max_bundle_size"]?.int.map(String.init)
      ?? ""
  }

  func apply(to table: TOMLTable) {
    let retention = table.section("retention")
    retention.set("max_deployments", maxDeployments.wholeNumber)
    retention.set("max_age_days", maxAgeDays.wholeNumber)
    retention.set("max_bundle_size", maxBundleSize.nonEmpty)
    table.removeIfEmpty("retention")
  }

  var problem: String? {
    if !maxDeployments.isEmptyOrPositive {
      return "Enter how many deployments to keep as a whole number."
    }
    if !maxAgeDays.isEmptyOrPositive {
      return "Enter the deployment age limit as a whole number of days."
    }
    if let size = maxBundleSize.nonEmpty {
      let number = size.prefix { $0.isNumber || $0 == "." }
      let unit = size.dropFirst(number.count).trimmingCharacters(in: .whitespaces).lowercased()
      let units: Set = ["", "b", "kb", "mb", "gb", "tb", "kib", "mib", "gib", "tib"]
      if Double(number) == nil || !units.contains(unit) {
        return "Enter the largest bundle as a size, such as 100MB or 1GB."
      }
    }
    return nil
  }
}
