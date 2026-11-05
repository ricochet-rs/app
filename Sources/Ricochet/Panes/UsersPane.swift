import SwiftUI

struct UsersPane: View {
    @Binding var settings: UserSettings

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
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
            InputField(
                "Display name claim", text: $settings.displayNameClaim, prompt: "preferred_username",
                description: "Ricochet shows this claim as each person's name.")
            InputField(
                "Groups claim", text: $settings.groupsClaim, prompt: "groups",
                description: "Leave this empty and ricochet ignores groups from your identity provider.")
            InputField(
                "Group sync interval", text: $settings.syncInterval, prompt: "Minutes",
                description:
                    "How often ricochet refreshes groups, in minutes. Leave this empty and ricochet refreshes them only when someone signs in."
            )
        }
    }
}
