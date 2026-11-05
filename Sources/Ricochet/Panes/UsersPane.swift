import SwiftUI

enum UsersTab: CaseIterable, Hashable {
  case settings
  case people

  var title: String {
    switch self {
    case .settings: "Settings"
    case .people: "People"
    }
  }
}

struct UsersPane: View {
  @Binding var settings: UserSettings
  @Binding var tab: UsersTab
  /// Whether ricochet has a database to read people from.
  let isConfigured: Bool
  let openRicochet: () -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      Tabs(selection: $tab, options: UsersTab.allCases) { $0.title }
      switch tab {
      case .settings:
        SelectField(
          "Role for new people", selection: $settings.defaultRole, options: Role.allCases,
          description: "The first person to sign in still becomes the admin."
        ) { $0.title }
        ToggleField(
          "Require sign-in for every page", isOn: $settings.requireSignIn,
          description: "When this is off, people can browse without signing in.")
        ToggleField(
          "Allow profile pictures", isOn: $settings.allowAvatarUpload,
          description: "People can upload their own profile picture.")
      case .people:
        PeopleList(isConfigured: isConfigured, openRicochet: openRicochet)
      }
    }
  }
}

/// Everyone who signed in, each with a menu that changes their role as soon as you pick one.
private struct PeopleList: View {
  let isConfigured: Bool
  let openRicochet: () -> Void
  @State private var people: [Person]?
  @State private var changing = false
  @State private var failure: Failure?

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      if let failure {
        ErrorAlert(title: failure.title, message: failure.message) { self.failure = nil }
      }
      if let people, !people.isEmpty {
        VStack(spacing: 0) {
          ForEach(people) { person in
            row(person)
            if person != people.last {
              Rectangle().fill(Theme.border).frame(height: 1)
            }
          }
        }
        .overlay(Rectangle().strokeBorder(Theme.border))
      } else if !isConfigured || people != nil {
        HStack(spacing: 8) {
          Text("No one has signed in yet.")
            .font(.system(size: 14))
            .foregroundStyle(Theme.mutedForeground)
            .textSelection(.enabled)
          if isConfigured {
            Button("Open ricochet", action: openRicochet)
              .buttonStyle(.basecoat(.link))
          }
        }
      } else if failure == nil {
        ProgressView().controlSize(.small)
      }
    }
    .task(id: isConfigured) { await load() }
  }

  private func row(_ person: Person) -> some View {
    HStack(spacing: 12) {
      Avatar(person: person)
      VStack(alignment: .leading, spacing: 2) {
        Text(person.name)
          .font(.system(size: 14, weight: .medium))
          .foregroundStyle(Theme.foreground)
        if let email = person.email, email != person.name {
          Text(email)
            .font(.system(size: 13))
            .foregroundStyle(Theme.mutedForeground)
        }
      }
      .lineLimit(1)
      .textSelection(.enabled)
      Spacer()
      SelectMenu(
        "Role",
        selection: Binding(
          get: { person.role },
          set: { role in Task { await change(person, to: role) } }),
        options: Role.allCases
      ) { $0.title }
      .frame(width: 140)
      .disabled(changing)
    }
    .padding(.horizontal, 16)
    .padding(.vertical, 12)
  }

  private func load() async {
    guard isConfigured else { return }
    do {
      people = try await Person.all()
    } catch {
      failure = Failure(
        title: "Ricochet couldn't list people", message: error.localizedDescription)
    }
  }

  private func change(_ person: Person, to role: Role) async {
    guard role != person.role else { return }
    changing = true
    do {
      try await person.setRole(role)
    } catch {
      failure = Failure(
        title: "Ricochet couldn't change \(person.name)'s role",
        message: error.localizedDescription)
    }
    await load()
    changing = false
  }
}

/// A person's picture in a circle, or their initial when they have none.
private struct Avatar: View {
  let person: Person

  var body: some View {
    AsyncImage(url: person.avatarURL) { image in
      image.resizable().scaledToFill()
    } placeholder: {
      Text(person.name.prefix(1).uppercased())
        .font(.system(size: 13, weight: .medium))
        .foregroundStyle(Theme.mutedForeground)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.muted)
    }
    .frame(width: 32, height: 32)
    .clipShape(Circle())
  }
}
