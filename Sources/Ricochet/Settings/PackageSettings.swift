import Foundation
import TOMLKit

/// A named R repository, such as `CRAN`.
struct RRepository: Identifiable, Equatable {
  let id = UUID()
  var name = ""
  var url = ""

  static func == (lhs: Self, rhs: Self) -> Bool { lhs.name == rhs.name && lhs.url == rhs.url }
}

/// A Python package index or Julia registry URL.
struct PackageIndex: Identifiable, Equatable {
  let id = UUID()
  var url = ""

  static func == (lhs: Self, rhs: Self) -> Bool { lhs.url == rhs.url }
}

/// `[repositories]`, where deployments install packages from.
/// toml++ keeps a table's keys in name order, so ricochet reads the R repositories in name order too.
struct PackageSettings: Equatable {
  var r = [RRepository]()
  var python = [PackageIndex]()
  var julia = [PackageIndex]()
  var allowOverrides = true
}

extension PackageSettings: SettingsSection {
  init(table: TOMLTable) {
    self.init()
    let repositories = table["repositories"]?.table
    if let named = repositories?["r"]?.table {
      r = named.keys.map { RRepository(name: $0, url: named[$0]?.string ?? "") }
    }
    python = repositories?["python"]?.array?.map { PackageIndex(url: $0.string ?? "") } ?? []
    julia = repositories?["julia"]?.array?.map { PackageIndex(url: $0.string ?? "") } ?? []
    allowOverrides = repositories?["allow_overrides"]?.bool ?? true
  }

  func apply(to table: TOMLTable) {
    let repositories = table.section("repositories")
    let named = TOMLTable()
    for repository in r { named[repository.name.trimmed] = repository.url.trimmed }
    repositories.set("r", named.isEmpty ? nil : named)
    repositories.set("python", python.isEmpty ? nil : python.map(\.url.trimmed))
    repositories.set("julia", julia.isEmpty ? nil : julia.map(\.url.trimmed))
    repositories.set("allow_overrides", allowOverrides ? nil : false)
    table.removeIfEmpty("repositories")
  }

  var problem: String? {
    if r.contains(where: { $0.name.nonEmpty == nil }) {
      return "Name every R repository, such as CRAN."
    }
    if Set(r.map(\.name.trimmed)).count != r.count {
      return "Give each R repository a different name."
    }
    let urls = r.map(\.url) + python.map(\.url) + julia.map(\.url)
    if urls.contains(where: { !$0.isHTTPURL }) {
      return "Start every repository URL with https://."
    }
    return nil
  }
}
