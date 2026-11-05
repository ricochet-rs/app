# Ricochet for Mac

Run ricochet on your Mac and deploy your R, Julia, and Python work to it.

## Install

You need an Apple Silicon Mac with macOS 15 or later, and R, Julia, or Python.

1. Download `ricochet-<version>.dmg` from the [latest release](https://github.com/ricochet-rs/macos/releases/latest).
2. Drag **Ricochet** into **Applications** and open it.
3. If macOS blocks it, open **System Settings** > **Privacy & Security** and select **Open Anyway**.

![The Ricochet menu in the macOS menu bar](docs/screenshots/menu.png)

## Set up sign-in

1. Register an app with your identity provider.
2. In **Sign-in**, choose **OpenID Connect** or **Microsoft Entra ID** and enter the **Issuer URL**, **Client ID**, and **Client secret**.
3. Copy the **Redirect URL** into the app you registered.
4. Select **Install and start**.

The first person to sign in becomes the admin.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/sign-in-dark.png">
  <img alt="The Sign-in pane of the setup window" src="docs/screenshots/sign-in.png">
</picture>

## Share it

Open **Edit configuration** > **Remote access** and pick one:

- **Tailscale Funnel** for your Mac's `ts.net` address
- **Cloudflare Tunnel** for a hostname on your own domain
- **Your own address** for any other tunnel or proxy

Follow the steps on the pane, select **Save and restart**, and add the new **Redirect URL** to your identity provider.

![The Remote access pane set to Cloudflare Tunnel](docs/screenshots/remote-access.png)

## Everyday use

Click the rabbit in your menu bar to open, restart, or update ricochet, or to read its logs.
Ricochet keeps running after you quit the app.

## Your files stay private

Ricochet runs your apps in a sandbox, so they can't read the rest of your home folder or other drives.

## Uninstall

Open **Edit configuration** > **Server** and select **Uninstall ricochet**.
The app moves ricochet and everything you deployed to the Trash, so empty it only when you no longer need them.
