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
2. Open the app and select **Get started** to install ricochet on this machine.
3. Choose how people sign in, add a license if you have one, and select **Install and start**.

That's it. ricochet runs in the background, and you can come back to the app any time to change settings, read the logs, or update.

| Platform              | Download                                |
| --------------------- | --------------------------------------- |
| macOS (Apple Silicon) | `ricochet-<version>.dmg`                |
| Windows (x86_64)      | `ricochet-<version>-windows-x86_64.zip` |
| Linux (x86_64)        | `ricochet-<version>-x86_64.AppImage`    |

## Use ricochet on a server

The app installs ricochet on this machine only.
To run ricochet on a server, install it there by following the [installation guide](https://docs.ricochet.rs/latest/admin/installation/1-host/), then select **Connect to a server** in the app and enter its address.

## See your content

The app shows a ricochet server's own web UI inside its window, so everything you can do in the browser works in the app too.
You sign in there the same way you do in the browser.
Apps and documents open in your browser.

On Linux the app needs GTK 3 and WebKitGTK 4.1 from your distribution, such as `libwebkit2gtk-4.1-0` on Debian and Ubuntu or `webkit2gtk4.1` on Fedora.
It runs on X11, through XWayland on a Wayland desktop.

Switch instances from the menu in the title bar.
It lists this machine and any other ricochet server you connected to by its address.
**Connect another instance** connects to a running ricochet by its address, or installs ricochet on this machine instead, and **Manage instances** lets you rename, group and reorder the list.
A server you connected to by address shows only its web UI, since the app doesn't manage it.
For ricochet on this machine, press ⌘, on macOS or Ctrl+, on Windows and Linux, choose **Settings…** in the app menu, or select the settings button in the title bar to change its configuration, and select **Done** to return to its web UI.

## Behind a proxy

The **Network proxy** page, under **Server** in this machine's settings, sets the HTTP proxy ricochet uses there to install packages and deploy from Git.
By default it uses the machine's own settings: System Settings on macOS, Windows Settings on Windows, and `HTTPS_PROXY` on Linux.
Choose **Use this proxy** to enter one yourself, with the hosts that should bypass it.
The app itself has a **Network proxy** setting on its **App settings** page, which ⌘, or Ctrl+, opens for a server you added by address, and **App settings** opens from the sidebar of this machine's settings.
By default the app follows your operating system's proxy settings.
Enter a proxy there when the app needs one to reach a ricochet server or to download ricochet.

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
