# Ricochet for Mac

An app that installs ricochet on an Apple Silicon Mac and keeps it running while you are logged in.
It sits in the Dock and the menu bar, and ricochet keeps running after you quit it.
People sign in through your OpenID Connect provider or Microsoft Entra ID.

## Build

```sh
just dmg 0.1.0
```

This writes `dist/ricochet-0.1.0.dmg`.
You need macOS 15 or later, the Xcode Command Line Tools, and `just`.

## Release

```sh
git sv next-version
git tag v0.2.0
git push origin v0.2.0
```

`git sv next-version` prints the next version from the commits since the last tag.
Pushing the tag publishes a GitHub release with notes from those commits and attaches `ricochet-0.2.0.dmg`.

## What the app installs

| Path | Contents |
| --- | --- |
| `~/.local/bin/ricochet-server` | The latest ricochet release |
| `~/.local/share/ricochet/` | Configuration, database, content, logs, and encryption keys |
| `~/Library/LaunchAgents/rs.ricochet.server.plist` | Starts ricochet when you log in |
| `~/Library/Application Support/Ricochet/` | Your remote access choice and the Cloudflare tunnel token |
| `~/.local/bin/cloudflared` and `~/Library/LaunchAgents/rs.ricochet.tunnel.plist` | Only when you use Cloudflare Tunnel |

## Settings

Change settings from the menu bar with **Edit configuration**.
Settings you add to `ricochet-config.toml` yourself, such as an `[smtp]` section, survive a save, but comments and formatting do not.

## Remote access

Choose how people outside your Mac reach ricochet under **Edit configuration** > **Remote access**:

- **Tailscale Funnel** publishes ricochet at your Mac's `ts.net` address. You need the Tailscale app, HTTPS certificates turned on for your tailnet, and Funnel allowed for your Mac. The pane tells you which of these is missing. Funnel doesn't work on a tailnet you run on Headscale.
- **Cloudflare Tunnel** publishes ricochet at a hostname on your own domain. Create the tunnel in the Cloudflare dashboard, paste its token, and point its public hostname at the service URL the pane shows. Ricochet installs and runs `cloudflared` for you.
- **Your own address** is for any other tunnel or proxy you run.

Each choice changes the redirect URL, so add the new one to your identity provider's app.
The menu bar shows whether ricochet answers at its public address.

## Sandbox

Ricochet and everything it runs can write only to `~/.local/share/ricochet` and temporary folders.
They can read system folders and the R, Python, Julia, and Quarto installs they run, but not the rest of your home folder or other drives.
Their `~` is `~/.local/share/ricochet/home`.

## First launch

The app is not notarized, so macOS blocks it the first time you open it.
Open **System Settings**, select **Privacy & Security**, and select **Open Anyway**.

## Uninstall

Open **Edit configuration**, select **Server**, and select **Uninstall ricochet**.
The app stops ricochet and moves it, with its content, database, caches and encryption keys, to the Trash, so you can restore them until you empty it.
The app stays: **Install ricochet** in the same place downloads ricochet again, and **Save and start** sets it up once you fill in **Sign-in**.

To uninstall by hand instead:

```sh
launchctl bootout gui/$(id -u)/rs.ricochet.server
launchctl bootout gui/$(id -u)/rs.ricochet.tunnel
rm -f ~/Library/LaunchAgents/rs.ricochet.server.plist ~/Library/LaunchAgents/rs.ricochet.tunnel.plist
rm -f ~/.local/bin/ricochet-server ~/.local/bin/cloudflared
rm -rf ~/Library/Application\ Support/Ricochet
```

If you used Tailscale Funnel, turn it off with `tailscale funnel --https=443 off`.
Then delete Ricochet from Applications.
Your data stays in `~/.local/share/ricochet`, including the keys that decrypt your environment variables, so delete that folder only when you no longer need anything you deployed.
