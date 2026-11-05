import SwiftUI

struct PackagesPane: View {
  @Binding var settings: PackageSettings

  var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      Field("R repositories", description: "Ricochet tries them in name order.") {
        VStack(alignment: .leading, spacing: 8) {
          ForEach($settings.r) { $repository in
            HStack(spacing: 8) {
              TextInput(text: $repository.name, prompt: "CRAN")
                .frame(width: 140)
              TextInput(text: $repository.url, prompt: "https://cloud.r-project.org")
              RemoveButton { settings.r.removeAll { $0.id == repository.id } }
            }
          }
          Button("Add R repository") { settings.r.append(RRepository()) }
            .buttonStyle(.basecoat(.outline, small: true))
        }
      }
      Field("Julia registries") {
        IndexList(
          entries: $settings.julia, prompt: "https://github.com/JuliaRegistries/General.git",
          addTitle: "Add Julia registry")
      }
      Field("Python indexes") {
        IndexList(
          entries: $settings.python, prompt: "https://pypi.org/simple", addTitle: "Add Python index"
        )
      }
      ToggleField(
        "Let developers choose their own", isOn: $settings.allowOverrides,
        description: "Developers can add or replace repositories in an item's _ricochet.toml.")
    }
  }
}

private struct IndexList: View {
  @Binding var entries: [PackageIndex]
  let prompt: String
  let addTitle: String

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      ForEach($entries) { $entry in
        HStack(spacing: 8) {
          TextInput(text: $entry.url, prompt: prompt)
          RemoveButton { entries.removeAll { $0.id == entry.id } }
        }
      }
      Button(addTitle) { entries.append(PackageIndex()) }
        .buttonStyle(.basecoat(.outline, small: true))
    }
  }
}

private struct RemoveButton: View {
  let action: () -> Void

  var body: some View {
    Button(action: action) {
      Image(systemName: "trash")
    }
    .buttonStyle(.basecoat(.ghost))
    .help("Remove")
    .accessibilityLabel("Remove")
  }
}
