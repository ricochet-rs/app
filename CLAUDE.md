# CLAUDE.md

## Shared instructions

Before inspecting repository files, planning, editing, or running project commands, load the latest shared `ricochet-rs` instructions.

1. Use `~/.cache/ricochet-rs/agent-instructions` as the shared checkout.
2. If the checkout is absent, clone `git@github.com:ricochet-rs/agent-instructions.git` there.
3. Fetch `origin/main` and check it out in detached-HEAD mode.
4. Verify that cached `HEAD` exactly matches `origin/main`.
5. Read `instructions/global.md` from the shared checkout.
6. Read `skills/development-flow/SKILL.md` before changing code.

If authentication, synchronization, HEAD validation, or a required read fails, stop before modifying this repository and report the failure clearly.

## Scope

The app runs ricochet for one person on one Apple Silicon Mac.
Never write a `[backend]` section, because content runs as processes on the Mac and Kubernetes and containers are out of scope.
Offer OpenID Connect and Entra only: ricochet refuses a SAML assertion consumer URL that is not `https`, and the server here serves `http://localhost`.

## App

Keep the app a regular app, never `LSUIElement`, because a menu-bar-only app has no Dock icon or ⌘-Tab entry, so its window falls behind other apps with nothing to bring it back.
Reopen the setup window for a Dock click through `MenuBarLabel`, because the menu bar item is the only view always on screen to call `openWindow`.

## Server process

Run `ricochet-server start` in the foreground under the launch agent, never with `--daemon`, because launchd cannot supervise a process that forks away from it.
Write an absolute `home` and pass `--config`, because launchd starts the job in `/` and ricochet's `home` defaults to the working directory.
Turn on `logs.file_logging`, because launchd appends stdout to its file forever and never rotates it.
Probe the proxy port only when the person changed it, before stopping the server, and with `SO_REUSEADDR`, because ricochet's own just-closed connections make its port look taken right after it stops.
Keep the ports already saved in `[network]` instead of probing again, so a save never moves ricochet's internal ports.

## Configuration

`ricochet-config.toml` is the only store of the setup form's answers.
Read and write it only through TOMLKit's `TOMLTable`, never by parsing or assembling TOML text.
Give each config section a `SettingsSection` in `Sources/Ricochet/Settings/`, a pane in `Sources/Ricochet/Panes/` and a `SetupSection` case, and list it in `ServerConfig.sections` and `ServerConfig.init(table:)`.
Write a key only when it differs from ricochet's default, so a later ricochet default still applies; `auth.default_role` is the exception, because ricochet warns at every sign-in without it.
Edit a section through `TOMLTable.section`, because assigning a table copies it, so edits made to that Swift object afterwards never reach the document.
Fill a new table key by key rather than with `TOMLTable(dictionary)`, which resolves to TOMLKit's throwing `Encodable` initializer.
toml++ keeps a table's keys in name order, so the R repositories reach ricochet sorted by name whatever order they were typed in.

## Authentication

Hold the port at 6188 under managed auth, because Zitadel accepts only `http://localhost:6188/oauth/callback` as the redirect for the Web app in `AuthenticationSettings.managedProvider`.
Warn whenever the form changes the issuer, because ricochet keys each user by the bare OIDC `sub` and gives everyone who signs in through a new identity provider a new account with the default role.
Read and change people with `ricochet-server user --config <file>`, which the app can run beside the server because ricochet writes SQLite directly, and expect `list` to print `No users found.` instead of JSON when nobody has signed in.

## Sandbox

Run `ricochet-server` under `sandbox-exec` with the profile in `LaunchAgent.swift`, and keep ricochet's source unchanged: every process the server starts inherits the profile.
Never let the profile write `~/Library/LaunchAgents` or `~/.local/bin`, because content could rewrite the job or the binary and leave the sandbox at the next start.
Pass folders to the profile as `-D` parameters rather than writing them into its text, so a path never needs SBPL escaping.
Deny `file-read-data` under `/Users`, never `file-read*`, because R and Python resolve a path by reading the metadata of every parent folder.
Set `HOME` inside ricochet's folder, and point `JULIAUP_DEPOT_PATH` and `UV_PYTHON_INSTALL_DIR` at the real ones, because the server finds juliaup's Julia installs and uv's Pythons through them.

## Uninstall

Move what uninstall removes to the Trash, never delete it, because ricochet's folder holds the only keys that decrypt its environment variables.
Keep the app itself on uninstall; it returns to the setup state so you can install ricochet again.
Leave juliaup's Julia installs and uv's Pythons alone, because the person installed those, not the app.

## renv

Keep `renv_version` in the `justfile` equal to `RENV_VERSION` in ricochet's `utils/vendor-versions.sh`.
Ricochet installs whichever tarball the app copies, but reinstalls renv only when its own pin changes.

## CI

Run git-sv and commitlint on the Linux agent and only Swift builds on the Apple Silicon agent, because that agent runs the `local` backend, where `image` names the shell and containers do not exist.
Pass `manifest_flags` from the `justfile` to every `swift build` and `swift test`, because the Apple Silicon agent runs each step in a sandbox, where `sandbox-exec` can't nest and root can't write its own module cache, which that agent's compiler, a newer build than its SDK, has to fill.
Keep `dmg.yaml` depending on `release`, because `gh release upload` needs the release `release.yaml` creates.
Write Crow metadata in brace form (`${CI_COMMIT_TAG}`) and shell-local variables bare or with `$$`, because Crow expands `${...}` before the shell runs, as ricochet's root `CLAUDE.md` explains.
Make every release step safe to restart: edit the release when it exists, and upload the `.dmg` with `--clobber`.
`.gitsv/config.yaml` and `.commitlintrc.json` are copies of ricochet's; change them in both repositories together.

## Style

Make a person or ricochet the subject of every sentence in the app, written the way a person would say it: "Leave this empty and ricochet keeps every file", never "Files are kept" or "Debug writes more".
Leave a setting out of the app when ricochet ignores it on macOS, as it does `resources.default_memory_limit`.
Put `.textSelection(.enabled)` on every `Text` outside a button, because SwiftUI text on macOS can't be selected or copied otherwise.
Build a dropdown from `Menu`, never `.popover`, because macOS centres a popover on its trigger with an arrow instead of dropping it from the field's edge.
Link to a setting rather than nudging people toward it.
Copy colour tokens from ricochet's `ricochet-ui/style/theme.css` as OKLCH triples, and keep every corner square, because that theme sets `--radius: 0rem`.
Run `just fmt` before every commit, because CI fails on anything `just lint` reports.
Format with swift-format's defaults and keep no `.swift-format` file.
Validate with `just build` and `just test`, never a bare `swift build`, because only the `justfile` sets `SDKROOT` and the Swift Testing paths the Command Line Tools need.
Keep `SDKROOT` on the macOS 26 SDK until the Command Line Tools ship a `SwiftUIMacros` plugin, because `@State` is a macro in the 27 SDK and the Command Line Tools 27 have no plugin to expand it.
Run `just icons` after changing `Support/logo.svg` and commit what it renders, because `just app` copies those files and needs no `rsvg-convert`.
