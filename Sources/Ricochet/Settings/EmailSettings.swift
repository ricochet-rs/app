import TOMLKit

enum Encryption: String, CaseIterable, Hashable {
  case starttls
  case tls
  case none

  var title: String {
    switch self {
    case .starttls: "STARTTLS"
    case .tls: "TLS"
    case .none: "None"
    }
  }
}

/// `[smtp]`, the mail server ricochet sends email through. Without it, ricochet sends no email.
struct EmailSettings: Equatable {
  var enabled = false
  var host = ""
  var port = ""
  var encryption = Encryption.starttls
  var username = ""
  var password = ""
  var from = ""
  var replyTo = ""
}

extension EmailSettings: SettingsSection {
  init(table: TOMLTable) {
    self.init()
    guard let smtp = table["smtp"]?.table else { return }
    enabled = true
    host = smtp["host"]?.string ?? ""
    port = smtp["port"]?.int.map(String.init) ?? ""
    encryption = smtp["tls"]?.string.flatMap(Encryption.init(rawValue:)) ?? .starttls
    username = smtp["username"]?.string ?? ""
    password = smtp["password"]?.string ?? ""
    from = smtp["from"]?.string ?? ""
    replyTo = smtp["reply_to"]?.string ?? ""
  }

  func apply(to table: TOMLTable) {
    guard enabled else {
      table.remove(at: "smtp")
      return
    }
    let smtp = table.section("smtp")
    smtp["host"] = host.trimmed
    smtp.set("port", port.wholeNumber)
    smtp.set("tls", encryption == .starttls ? nil : encryption.rawValue)
    smtp.set("username", username.nonEmpty)
    smtp.set("password", password.nonEmpty)
    smtp["from"] = from.trimmed
    smtp.set("reply_to", replyTo.nonEmpty)
  }

  var problem: String? {
    guard enabled else { return nil }
    if host.nonEmpty == nil { return "Enter the mail server's host name." }
    if let port = port.nonEmpty, UInt16(port).map({ $0 > 0 }) != true {
      return "Enter a mail server port between 1 and 65535, or leave it empty."
    }
    if (username.nonEmpty == nil) != (password.nonEmpty == nil) {
      return "Enter both a username and a password, or neither."
    }
    if !from.contains("@") {
      return "Enter the sender's address, such as ricochet <ricochet@example.com>."
    }
    if replyTo.nonEmpty != nil, !replyTo.contains("@") {
      return "Enter a reply-to address, or leave it empty."
    }
    return nil
  }
}
