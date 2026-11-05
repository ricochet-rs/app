import AppKit
import SwiftUI

enum ButtonVariant {
  case primary
  case outline
  case ghost
  case link
  case destructive
}

/// Basecoat's `btn`, `btn-outline`, `btn-ghost`, `btn-link` and `btn-destructive`, with their `-sm` sizes.
struct BasecoatButtonStyle: ButtonStyle {
  var variant = ButtonVariant.primary
  var small = false
  var fullWidth = false
  var alignment = Alignment.center

  func makeBody(configuration: Configuration) -> some View {
    StyledButton(configuration: configuration, style: self)
  }
}

extension ButtonStyle where Self == BasecoatButtonStyle {
  static func basecoat(
    _ variant: ButtonVariant = .primary, small: Bool = false, fullWidth: Bool = false,
    alignment: Alignment = .center
  ) -> Self {
    BasecoatButtonStyle(variant: variant, small: small, fullWidth: fullWidth, alignment: alignment)
  }
}

private struct StyledButton: View {
  let configuration: ButtonStyleConfiguration
  let style: BasecoatButtonStyle
  @Environment(\.isEnabled) var isEnabled
  @State var hovering = false

  var body: some View {
    if style.variant == .link {
      configuration.label
        .font(.system(size: 14, weight: .medium))
        .foregroundStyle(Theme.primary)
        .underline(hovering)
        .contentShape(Rectangle())
        .opacity(isEnabled ? (configuration.isPressed ? 0.8 : 1) : 0.5)
        .onHover { hovering = isEnabled && $0 }
    } else {
      configuration.label
        .font(.system(size: 14, weight: .medium))
        .lineLimit(1)
        .foregroundStyle(foreground)
        .padding(.horizontal, style.small ? 12 : 16)
        .frame(maxWidth: style.fullWidth ? .infinity : nil, alignment: style.alignment)
        .frame(height: style.small ? 32 : 36)
        .background(background)
        .overlay {
          if style.variant == .outline {
            Rectangle().strokeBorder(Theme.border)
          }
        }
        .contentShape(Rectangle())
        .opacity(isEnabled ? (configuration.isPressed ? 0.8 : 1) : 0.5)
        .onHover { hovering = isEnabled && $0 }
    }
  }

  private var foreground: Color {
    switch style.variant {
    case .primary: Theme.primaryForeground
    case .destructive: .white
    case .outline, .ghost, .link: Theme.foreground
    }
  }

  private var background: Color {
    switch style.variant {
    case .primary: Theme.primary.opacity(hovering ? 0.9 : 1)
    case .destructive: Theme.destructive.opacity(hovering ? 0.9 : 1)
    case .outline: hovering ? Theme.accent : Theme.background
    case .ghost: hovering ? Theme.accent : .clear
    case .link: .clear
    }
  }
}

/// A labelled form row: basecoat's `label`, its control, and the muted description below.
struct Field<Content: View>: View {
  let label: String
  let description: String?
  let content: Content

  init(_ label: String, description: String? = nil, @ViewBuilder content: () -> Content) {
    self.label = label
    self.description = description
    self.content = content()
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 8) {
      Text(label)
        .font(.system(size: 14, weight: .medium))
        .foregroundStyle(Theme.foreground)
        .textSelection(.enabled)
      content
      if let description {
        Text(description)
          .font(.system(size: 13))
          .foregroundStyle(Theme.mutedForeground)
          .fixedSize(horizontal: false, vertical: true)
          .textSelection(.enabled)
      }
    }
  }
}

/// Basecoat's `input` on its own, with the `ring` focus outline.
struct TextInput: View {
  @Binding var text: String
  let prompt: String
  let secure: Bool
  @FocusState private var focused: Bool

  init(text: Binding<String>, prompt: String = "", secure: Bool = false) {
    _text = text
    self.prompt = prompt
    self.secure = secure
  }

  var body: some View {
    Group {
      if secure {
        SecureField(prompt, text: $text, prompt: Text(prompt))
      } else {
        TextField(prompt, text: $text, prompt: Text(prompt))
      }
    }
    .labelsHidden()
    .textFieldStyle(.plain)
    .font(.system(size: 14))
    .foregroundStyle(Theme.foreground)
    .focused($focused)
    .focusEffectDisabled()
    .padding(.horizontal, 12)
    .frame(height: 36)
    .background(Theme.background)
    .overlay(Rectangle().strokeBorder(focused ? Theme.ring : Theme.input))
    .background(Rectangle().stroke(Theme.ring.opacity(focused ? 0.5 : 0), lineWidth: 6))
  }
}

/// A value you can't edit, with a button that copies it.
struct CopyField: View {
  let label: String
  let value: String
  let description: String?
  @State private var copied = false

  init(_ label: String, value: String, description: String? = nil) {
    self.label = label
    self.value = value
    self.description = description
  }

  var body: some View {
    Field(label, description: description) {
      HStack(spacing: 8) {
        Text(value)
          .font(.system(size: 13, design: .monospaced))
          .foregroundStyle(Theme.foreground)
          .textSelection(.enabled)
          .padding(.horizontal, 12)
          .frame(maxWidth: .infinity, minHeight: 36, alignment: .leading)
          .background(Theme.muted)
          .overlay(Rectangle().strokeBorder(Theme.input))
        Button(copied ? "Copied" : "Copy") {
          NSPasteboard.general.clearContents()
          NSPasteboard.general.setString(value, forType: .string)
          copied = true
          Task {
            try? await Task.sleep(for: .seconds(2))
            copied = false
          }
        }
        .buttonStyle(.basecoat(.outline))
      }
    }
  }
}

/// A labelled `TextInput`.
struct InputField: View {
  let label: String
  @Binding var text: String
  let prompt: String
  let description: String?
  let secure: Bool

  init(
    _ label: String, text: Binding<String>, prompt: String = "", description: String? = nil,
    secure: Bool = false
  ) {
    self.label = label
    _text = text
    self.prompt = prompt
    self.description = description
    self.secure = secure
  }

  var body: some View {
    Field(label, description: description) {
      TextInput(text: $text, prompt: prompt, secure: secure)
    }
  }
}

/// Basecoat's `switch`: a pill, because `rounded-full` ignores the theme's square radius.
struct BasecoatSwitchStyle: ToggleStyle {
  func makeBody(configuration: Configuration) -> some View {
    Button {
      configuration.isOn.toggle()
    } label: {
      Capsule()
        .fill(configuration.isOn ? Theme.primary : Theme.input)
        .frame(width: 32, height: 18)
        .overlay(alignment: configuration.isOn ? .trailing : .leading) {
          Circle()
            .fill(configuration.isOn ? Theme.primaryForeground : Theme.background)
            .frame(width: 14, height: 14)
            .padding(2)
        }
        .animation(.easeOut(duration: 0.15), value: configuration.isOn)
    }
    .buttonStyle(.plain)
  }
}

/// A setting that is on or off, with the switch beside its label.
struct ToggleField: View {
  let label: String
  @Binding var isOn: Bool
  let description: String?

  init(_ label: String, isOn: Binding<Bool>, description: String? = nil) {
    self.label = label
    _isOn = isOn
    self.description = description
  }

  var body: some View {
    HStack(alignment: .top, spacing: 16) {
      VStack(alignment: .leading, spacing: 4) {
        Text(label)
          .font(.system(size: 14, weight: .medium))
          .foregroundStyle(Theme.foreground)
        if let description {
          Text(description)
            .font(.system(size: 13))
            .foregroundStyle(Theme.mutedForeground)
            .fixedSize(horizontal: false, vertical: true)
        }
      }
      .textSelection(.enabled)
      Spacer()
      Toggle(label, isOn: $isOn)
        .labelsHidden()
        .toggleStyle(BasecoatSwitchStyle())
    }
  }
}

/// Basecoat's `select`: a bordered trigger that drops the options down from its leading edge.
struct SelectMenu<Value: Hashable>: View {
  let label: String
  @Binding var selection: Value
  let options: [Value]
  let title: (Value) -> String

  init(
    _ label: String, selection: Binding<Value>, options: [Value],
    title: @escaping (Value) -> String
  ) {
    self.label = label
    _selection = selection
    self.options = options
    self.title = title
  }

  var body: some View {
    Menu {
      Picker(label, selection: $selection) {
        ForEach(options, id: \.self) { option in
          Text(title(option)).tag(option)
        }
      }
      .pickerStyle(.inline)
      .labelsHidden()
    } label: {
      HStack {
        Text(title(selection))
        Spacer()
        Image(systemName: "chevron.up.chevron.down")
          .font(.system(size: 10, weight: .semibold))
          .foregroundStyle(Theme.mutedForeground)
      }
      .font(.system(size: 14))
      .foregroundStyle(Theme.foreground)
      .padding(.horizontal, 12)
      .frame(maxWidth: .infinity)
      .frame(height: 36)
      .background(Theme.background)
      .overlay(Rectangle().strokeBorder(Theme.input))
      .contentShape(Rectangle())
    }
    .menuStyle(.button)
    .buttonStyle(.plain)
    .menuIndicator(.hidden)
  }
}

/// A labelled `SelectMenu`.
struct SelectField<Value: Hashable>: View {
  let label: String
  @Binding var selection: Value
  let options: [Value]
  let title: (Value) -> String
  let description: String?

  init(
    _ label: String, selection: Binding<Value>, options: [Value], description: String? = nil,
    title: @escaping (Value) -> String
  ) {
    self.label = label
    _selection = selection
    self.options = options
    self.title = title
    self.description = description
  }

  var body: some View {
    Field(label, description: description) {
      SelectMenu(label, selection: $selection, options: options, title: title)
    }
  }
}

/// Basecoat's boxed `tabs`, one button per option.
struct Tabs<Value: Hashable>: View {
  @Binding var selection: Value
  let options: [Value]
  let title: (Value) -> String

  init(
    selection: Binding<Value>, options: [Value], title: @escaping (Value) -> String
  ) {
    _selection = selection
    self.options = options
    self.title = title
  }

  var body: some View {
    HStack(spacing: 0) {
      ForEach(options, id: \.self) { option in
        let selected = selection == option
        Button {
          selection = option
        } label: {
          Text(title(option))
            .font(.system(size: 14, weight: .medium))
            .foregroundStyle(selected ? Theme.foreground : Theme.mutedForeground)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(selected ? Theme.background : .clear)
            .shadow(color: .black.opacity(selected ? 0.06 : 0), radius: 1, y: 1)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
      }
    }
    .padding(3)
    .frame(height: 36)
    .background(Theme.muted)
  }
}

/// Basecoat's `alert`: an icon, a title, a sentence and any buttons below it.
struct Callout<Actions: View>: View {
  let symbol: String
  let tint: Color
  let title: String
  let message: String
  let actions: Actions

  init(
    symbol: String, tint: Color, title: String, message: String,
    @ViewBuilder actions: () -> Actions
  ) {
    self.symbol = symbol
    self.tint = tint
    self.title = title
    self.message = message
    self.actions = actions()
  }

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: 12) {
      Image(systemName: symbol)
        .foregroundStyle(tint)
      VStack(alignment: .leading, spacing: 4) {
        Text(title)
          .font(.system(size: 14, weight: .medium))
          .textSelection(.enabled)
        Text(message)
          .font(.system(size: 13))
          .foregroundStyle(Theme.mutedForeground)
          .fixedSize(horizontal: false, vertical: true)
          .textSelection(.enabled)
        actions
      }
      Spacer(minLength: 0)
    }
    .foregroundStyle(Theme.foreground)
    .padding(.horizontal, 16)
    .padding(.vertical, 12)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(Theme.card)
    .overlay(Rectangle().strokeBorder(Theme.border))
  }
}

extension Callout where Actions == EmptyView {
  init(symbol: String, tint: Color, title: String, message: String) {
    self.init(symbol: symbol, tint: tint, title: title, message: message) { EmptyView() }
  }
}

/// Basecoat's `alert-destructive`, with a button that dismisses it.
struct ErrorAlert: View {
  let title: String
  let message: String
  let dismiss: () -> Void

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: 12) {
      Image(systemName: "exclamationmark.circle")
      VStack(alignment: .leading, spacing: 4) {
        Text(title)
          .font(.system(size: 14, weight: .medium))
          .textSelection(.enabled)
        Text(message)
          .font(.system(size: 13))
          .opacity(0.9)
          .lineLimit(6)
          .textSelection(.enabled)
        Button("Copy full message") {
          NSPasteboard.general.clearContents()
          NSPasteboard.general.setString("\(title)\n\(message)", forType: .string)
        }
        .buttonStyle(.basecoat(.link))
        .font(.system(size: 13))
      }
      Spacer()
      Button(action: dismiss) {
        Image(systemName: "xmark")
          .font(.system(size: 11, weight: .semibold))
      }
      .buttonStyle(.plain)
      .foregroundStyle(Theme.mutedForeground)
      .help("Dismiss")
      .accessibilityLabel("Dismiss")
    }
    .foregroundStyle(Theme.destructive)
    .padding(.horizontal, 16)
    .padding(.vertical, 12)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(Theme.card)
    .overlay(Rectangle().strokeBorder(Theme.border))
  }
}
