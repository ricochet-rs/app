import Foundation
import TOMLKit

/// One pane's share of `ricochet-config.toml`.
protocol SettingsSection {
  /// The pane's answers as saved in `table`, the parsed config file.
  init(table: TOMLTable)

  /// Writes the pane's answers into `table`, leaving every key the pane does not manage alone.
  func apply(to table: TOMLTable)

  /// What the pane still needs before ricochet can start, or `nil` once it is complete.
  var problem: String? { get }
}

extension TOMLTable {
  /// The table at `key`, inserted empty when absent, as a live view: editing it edits this table.
  func section(_ key: String) -> TOMLTable {
    if self[key]?.table == nil { self[key] = TOMLTable() }
    return self[key]?.table ?? TOMLTable()
  }

  /// Sets `key` to `value`, or removes it when `value` is `nil`.
  func set(_ key: String, _ value: TOMLValueConvertible?) {
    if let value {
      self[key] = value
    } else {
      remove(at: key)
    }
  }

  /// Removes the table at `key` once nothing is left in it.
  func removeIfEmpty(_ key: String) {
    if self[key]?.table?.isEmpty == true { remove(at: key) }
  }
}

extension String {
  var trimmed: String { trimmingCharacters(in: .whitespacesAndNewlines) }

  /// The trimmed text, or `nil` when nothing is left.
  var nonEmpty: String? { trimmed.isEmpty ? nil : trimmed }

  var isHTTPURL: Bool { trimmed.hasPrefix("https://") || trimmed.hasPrefix("http://") }

  /// The whole number typed here, or `nil` when the field is empty.
  var wholeNumber: Int? { nonEmpty.flatMap { Int($0) } }

  /// Whether the field is empty or holds a whole number of at least 1.
  var isEmptyOrPositive: Bool { nonEmpty == nil || (wholeNumber ?? 0) > 0 }
}
