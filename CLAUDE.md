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

The app is a simple way to install ricochet's Community Edition for local development, and to install and configure ricochet on remote Linux hosts over SSH.
It builds for Apple Silicon macOS, x86_64 Linux and x86_64 Windows.
Never write a `[backend]` section, because content runs as processes on the machine and Kubernetes and containers are out of scope.
The content area in `src/ui/dashboard/` shows a server's own web UI, for the managed server and for any server added by address.
List managed machines and added servers in the one server menu of `SetupView`, and show settings only for a managed machine, so the window never has two switchers that both mean "which server".
List only the machines you connected to in that menu, never every host in the SSH config, which only the add-machine step offers.
Offer OpenID Connect and Entra only: ricochet refuses a SAML assertion consumer URL that is not `https`, and the server here serves `http://localhost`.

## App

Keep `LSUIElement` out of `Support/Info.plist`, because without a Dock icon or ⌘-Tab entry the window falls behind other apps with nothing to bring it back.
Compile the systemd code on every platform, because the SSH backend drives a Linux server's systemd from macOS and Windows too.

## Server process

Run `ricochet-server start` in the foreground under the launch agent or systemd unit, never with `--daemon`, because neither can supervise a process that forks away from it.
Write an absolute `home` and pass `--config`, because launchd starts the job in `/` and ricochet's `home` defaults to the working directory.
Turn on `logs.file_logging`, because launchd appends stdout to its file forever and never rotates it.
Probe the proxy port only when the person changed it, before stopping the server, and with `SO_REUSEADDR`, because ricochet's own just-closed connections make its port look taken right after it stops.
Keep the ports already saved in `[network]` instead of probing again, so a save never moves ricochet's internal ports.
Give the SSH tunnel its own connection with `ControlPath=none`, because a shared control master takes over the forward, lets `ssh` exit, and keeps the port after the app quits.
Start the server on Windows through a `Run` entry under `HKCU` and `ricochet-launcher.exe`, because that needs no admin rights, and the launcher hides the console window and restarts the server after a crash.
Spawn every process through `command::new`, because Windows flashes a console window for each console program the app runs.
Leave SSH connection sharing off on Windows, because its OpenSSH has no `ControlMaster`.

## Configuration

`ricochet-config.toml` is the only store of the setup form's answers.
Read and write it only through `toml_edit`, never by parsing or assembling TOML text.
Give each config section a `SettingsSection` in `src/config/`, a pane in `src/ui/setup.rs` and a `SetupSection` case in `src/ui/section.rs`, and list it in `ServerConfig`.
Write a key only when it differs from ricochet's default, so a later ricochet default still applies; `auth.default_role` is the exception, because ricochet warns at every sign-in without it.
Show ricochet's own default R repository, `CRAN = https://cran.rpkgs.com`, in the form but never write it, so the person can save it as is.
Sort R repositories by name when saving, because ricochet reads them in name order.

## Proxy

Keep each managed machine's proxy in `[desktop.proxy]` of its `ricochet-config.toml`, a table ricochet ignores, because ricochet has no proxy setting and a remote host can sit behind a different proxy than this computer.
Write nothing for the system choice, so a config without the table follows the machine's own settings.
Give ricochet's service `HTTP_PROXY`, `HTTPS_PROXY` and `NO_PROXY` in both cases, through the launchd job, the systemd unit or the Windows launcher's `--env` arguments, because launchd, systemd and the `Run` entry start it without the proxy variables of a shell, and R, Python and Git read them from the environment.
Keep the proxy out of command lines and world-readable files, because its URL can hold a password: write the launchd job and the systemd unit readable by their owner only, and hand it to a remote install through stdin.
Always put `localhost`, `127.0.0.1` and `::1` in `NO_PROXY`, and reach ricochet's health check without a proxy, because the app and ricochet talk to each other on this machine.
Read the system proxy from `scutil --proxy` on macOS, the `Internet Settings` registry key on Windows, and `HTTPS_PROXY` elsewhere, including a login shell on an SSH host; PAC files aren't read.
Send the app's own downloads through `net::download`, which uses the local machine's choice, and leave the webview on the operating system's proxy, which already handles PAC.

## Authentication

Save managed auth only after the person selects **I agree**, because you share your name and email address with ricochet when you sign in through it.
Hold the port at 6188 under managed auth, because Zitadel accepts only `http://localhost:6188/oauth/callback` as the redirect for the Web app in `AuthenticationSettings::managed_provider`.
Warn whenever the form changes the issuer, because ricochet keys each user by the bare OIDC `sub` and gives everyone who signs in through a new identity provider a new account with the default role.
Read and change people with `ricochet-server user --config <file>`, which the app can run beside the server because ricochet writes SQLite directly, and expect `list` to print `No users found.` instead of JSON when nobody has signed in.

## Sandbox

Run `ricochet-server` under `sandbox-exec` on macOS with the profile in `src/service/launchd.rs`, and keep ricochet's source unchanged: every process the server starts inherits the profile.
Start the job through the `ricochet` launcher, which execs `sandbox-exec`, because macOS names a background item after the executable launchd runs.
Never let the profile write `~/Library/LaunchAgents` or `~/.local/bin`, because content could rewrite the job or the binary and leave the sandbox at the next start.
Pass folders to the profile as `-D` parameters rather than writing them into its text, so a path never needs SBPL escaping.
Deny `file-read-data` under `/Users`, never `file-read*`, because R and Python resolve a path by reading the metadata of every parent folder.
Set `HOME` inside ricochet's folder, and point `JULIAUP_DEPOT_PATH` and `UV_PYTHON_INSTALL_DIR` at the real ones, because the server finds juliaup's Julia installs and uv's Pythons through them.

## Content

Show a server's own web UI in the system webview through gpui-component's `webview` feature, WKWebView on macOS, WebView2 on Windows and WebKitGTK on Linux, so the app and the server share one UI and every page works through a browser session.
Run the app on X11 on Linux, through XWayland under Wayland, and start GTK before GPUI in `linux_webview::prepare`, because wry embeds WebKitGTK only in an X11 window.
Advance GTK's loop from a GPUI timer while the webview exists, because GPUI doesn't run it and WebKitGTK paints nothing without it.
Hand wry an Xlib handle on Linux, because GPUI gives out an XCB one and wry accepts only Xlib; both name the same X window.
Put the app's own controls, the server menu, the managed server's status and its actions, in the title bar beside the window buttons on macOS and Windows, and leave the rest of the window to the web UI.
Open and close a managed machine's settings with ⌘, on macOS and Ctrl+, elsewhere, the **Settings…** item in the app menu and the title bar's settings button, through the `ToggleSettings` action registered on the app, because a key binding inside the window never fires while the webview has focus.
Hide the webview whenever something else must show over the content area, such as the server menu, because the system draws it above the GPUI window.
Hand only `http` and `https` links from the webview to the browser, because content on the server can ask to open any URL.
Keep the webview's cookies and storage in the app's data folder, because WebView2 can't write next to the program.
Open a managed SSH server's tunnel through `Backend::reach_dashboard` before showing its web UI, because the server answers on `localhost` only while the tunnel is up.
Link the AppImage against the system's GTK and WebKitGTK instead of bundling them, because WebKitGTK starts helper processes from fixed paths and needs the system's GLib.

## Uninstall

Move what uninstall removes to the Trash on macOS and the Recycle Bin on Windows, and delete it on a Linux server, which has no Trash; the confirmation says ricochet and all of its folders will be deleted.
Keep the app itself on uninstall; it returns to the setup state so you can install ricochet again.
Leave juliaup's Julia installs and uv's Pythons alone, because the person installed those, not the app.

## renv

Keep `renv_version` in the `justfile` equal to `RENV_VERSION` in ricochet's `utils/vendor-versions.sh`.
Bundle renv in the macOS and Windows builds only, because ricochet's own Linux packages already ship it.
Ricochet installs whichever tarball the app copies, but reinstalls renv only when its own pin changes.

## CI

Run git-sv, commitlint, tests and the Linux and Windows builds in Crow on the Linux agent.
Build the `.dmg` with GitHub Actions in `.github/workflows/mac.yml`, because Crow's Apple Silicon agent sandboxes each step and `hdiutil` can't attach a disk image there.
The repository is private, so every run of `mac.yml` bills macOS minutes; it runs on each pushed `v*` tag and can be rerun for a tag from the Actions tab.
`mac.yml` starts on the tag push, not on the release, and waits for Crow's release before it uploads, because Crow only edits a release that already exists and GitHub fires no `published` event for that.
Build the AppImage on Debian bookworm, the oldest Debian that still gets security updates, because the glibc it links against sets the oldest distribution it runs on.
Start a script in a Debian container with `set -eu`, never `-o pipefail`, because Crow runs it with dash.
Cross-compile Windows with cargo-xwin and the `windows` Cargo profile, because no Windows agent exists and GPUI precompiles its Windows shaders only when it builds on Windows; that profile keeps GPUI's debug assertions on, which makes it compile them at run time.
Keep `linux.yaml` and `windows.yaml` depending on `release`, because `gh release upload` needs the release `release.yaml` creates.
Write Crow metadata in brace form (`${CI_COMMIT_TAG}`) and shell-local variables bare or with `$$`, because Crow expands `${...}` before the shell runs, as ricochet's root `CLAUDE.md` explains.
Make every release step safe to restart: edit the release when it exists, and replace an artifact a previous run uploaded, with `gh release upload --clobber`.
`.gitsv/config.yaml` and `.commitlintrc.json` are copies of ricochet's; change them in both repositories together.

## Style

Write app text in plain, friendly, complete sentences that match the copy already in the app, such as "Enable compression" with "Compress old logs to save disk space.", because clipped label-speak like "The claim that lists each user's groups." reads like a robot.
Leave a setting out of the app when ricochet ignores it on macOS, as it does `resources.default_memory_limit`.
Link to a setting rather than nudging people toward it.
Copy colour tokens from ricochet's `ricochet-ui/style/theme.css` as OKLCH triples, and keep every corner square, because that theme sets `--radius: 0rem`.
Set `ricochet` in lowercase monospace through `prose`, which also renders backticked words in monospace.
Give a scrolling area a fixed pixel height, never `size_full` inside a parent sized by its children, because GPUI resolves the percentage to the content size and the content spills out unclipped.
Stop scroll-wheel propagation on a scrolling area inside the scrolling page, because GPUI scrolls every scroll container under the pointer.
Render long lists such as the log through GPUI's `list` with a `ListState`, because building every row on each render makes the window lag.
Run `just fmt` before every commit, because CI fails on anything `just lint` reports.
Run `just icons` after changing `Support/logo.svg` and commit what it renders, because `just app` and `just appimage` copy those files and need no `rsvg-convert`.

## Hooks

Run `prek run -a` before every commit, because `.crow/lint.yaml` runs the same hooks on every file.
Exclude copied upstream files in both `prek.toml` and `_typos.toml`, because typos walks the repository itself and never sees prek's `exclude`.
Drop the `mac.yml` exclude from the `yaml-file-extension` hook when that file goes, because `check-useless-excludes` fails on an exclude that matches nothing.

## Renovate

Keep the `rust-toolchain` manager disabled in `renovate.json`, because the toolchain follows ricochet's pin by hand.
Keep `allowedVersions` for `libc` equal to the upper bound in `Cargo.toml`, because Renovate widens a version range it is not told to respect.
Leave automerge off for the GPUI crates, because no test renders the UI.
