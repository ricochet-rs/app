import Foundation

/// Someone who signed in to ricochet, as `ricochet-server user list` reports them.
struct Person: Decodable, Identifiable, Equatable {
  let id: String
  let email: String?
  let displayName: String?
  let avatarURL: URL?
  let role: Role

  var name: String { displayName ?? email ?? id }

  private enum CodingKeys: String, CodingKey {
    case id, email, role
    case displayName = "display_name"
    case avatarURL = "avatar_url"
  }

  /// Everyone who signed in to ricochet.
  static func all() async throws -> [Person] {
    struct Entry: Decodable {
      let user: Person
    }
    let output = try await userCommand(["list", "--format", "json"])
    // Ricochet prints `No users found.` instead of `[]` when nobody has signed in.
    guard output.hasPrefix("[") else { return [] }
    return try JSONDecoder().decode([Entry].self, from: Data(output.utf8)).map(\.user)
  }

  /// Gives this person `role`, which ricochet applies from their next request without a restart.
  func setRole(_ role: Role) async throws {
    _ = try await Self.userCommand(["set-role", id, role.rawValue])
  }

  private static func userCommand(_ arguments: [String]) async throws -> String {
    let result = try await Command.run(
      Paths.binary.path(percentEncoded: false),
      ["user", "--config", Paths.config.path(percentEncoded: false)] + arguments)
    let output = result.output.trimmingCharacters(in: .whitespacesAndNewlines)
    guard result.status == 0 else { throw AppError.userCommand(output) }
    return output
  }
}
