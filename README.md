<p align="center">
  <img src="Support/AppIcon.png" width="128" height="128" alt="The ricochet bunny">
</p>

<h1 align="center">ricochet native app</h1>

<p align="center">
  Install and configure ricochet on macOS, Windows, and Linux.
</p>

<p align="center">
  <a href="https://github.com/ricochet-rs/app/releases/latest"><strong>Download</strong></a>
  ·
  <a href="https://docs.ricochet.rs">Docs</a>
  ·
  <a href="https://docs.ricochet.rs/latest/admin/pricing/5-community-edition/">Community Edition</a>
</p>

---

This repository contains a native app to install and configure ricochet for macOS, Windows, and Linux.
The ricochet native app will get you up and running with the Community Edition in minutes.

## Get started

1. Download the latest release for your platform.
2. Open the app and choose where to install ricochet: this machine or one of your SSH hosts.
3. Choose how people sign in, add a license if you have one, and select **Install and start**.

That's it. ricochet runs in the background, and you can come back to the app any time to change settings, read the logs, or update.

| Platform              | Download                                |
| --------------------- | --------------------------------------- |
| macOS (Apple Silicon) | `ricochet-<version>.dmg`                |
| Windows (x86_64)      | `ricochet-<version>-windows-x86_64.zip` |
| Linux (x86_64)        | `ricochet-<version>-x86_64.AppImage`    |

## Install on a remote host

The ricochet native app can use your existing SSH connections to install ricochet on any of your machines.
Every host from your SSH config shows up in the app, and you can add one with `user@host`.

Each SSH host is expected to be a Linux machine you can reach with key-based SSH access.

## See your content

The **Content** pages at the top of the sidebar show what is deployed to a ricochet server, right in the app.

- **Apps** and **Tasks** list the items you can access in a table, as ricochet's web UI does. Each item has an overview with its deployment history, its build, instance and run logs, what is running now, its settings and schedule, and its environment variables.
- **Deployment history** lists the latest deployments of every item, and opens a build log when you select one.
- **Deploy** deploys from a Git repository, or uploads a project folder with a `_ricochet.toml` the way `ricochet deploy` does.
- **Credentials** lists your Git credentials.

Apps and documents still open in your browser.

Switch instances from the menu at the top of the window.
It lists this machine, the SSH hosts you connected to, and any other ricochet server you connected to by its address.
**Connect another instance** connects to a running ricochet by its address, or installs ricochet on a machine instead, and **Manage instances** lets you rename, group and reorder the list.
A server you connected to by address shows only its content, since the app doesn't manage it.
Select **Sign in with your browser** to sign in the same way the ricochet CLI does, or paste an API key.
Keys from the browser sign-in last eight hours.

## Bring your own languages

The app doesn't install R, Julia, or Python.
On macOS and Windows, install them yourself, along with [juliaup](https://github.com/JuliaLang/juliaup) and [uv](https://docs.astral.sh/uv/).
On Linux, juliaup and uv are vendored in ricochet's `.deb` and `.rpm` installers.

## Authentication

We provide a managed authentication provider that is intended only for local development.
By using it, you agree to our [privacy policy](https://ricochet.rs/legal/privacy/).

Ready for something more permanent? Bring your own identity provider instead.
Head to [docs.ricochet.rs](https://docs.ricochet.rs) to learn how to set it up.

## Community Edition

The Community Edition is designed to empower the community while maintaining a clear boundary with enterprise use.
It comes with a few limits:

- Up to 2 developer users
- Up to 5 git deployments
- No private applications
- No scheduled tasks
- Audit logging is disabled

Read more in the [Community Edition docs](https://docs.ricochet.rs/latest/admin/pricing/5-community-edition/).

Have a license? Add it during setup, or later under **Server**.
