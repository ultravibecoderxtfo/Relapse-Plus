# Relapse +

A native **GTK4 + GtkSourceView** interface for [Relapse](https://relapse.im)
profiles, written in Rust. Tabs, Luau syntax highlighting and completion,
themes, a script sidebar, an output console and a Client Manager for launching
several accounts side by side.

> **Relapse + is only a UI.** It does not inject into Roblox and it does not
> execute Lua. `Execute` sends a protocol frame to a Relapse backend over its
> local Unix socket. You need the official Relapse launcher installed.

---

## Install with one command

```bash
curl -fsSL https://raw.githubusercontent.com/ultravibecoderxtfo/Relapse-Plus/main/install.sh | bash
```

That downloads the source into `~/.local/share/relapse-plus`, installs the GTK 4
and GtkSourceView 5 development packages plus Rust if they are missing, builds
the native binary as a release build, adds a **Relapse +** entry to your
application menu, and tells you how to start it.

Supported package managers: `pacman`, `apt`, `dnf`, `zypper`. Other distros:
install Git, curl, a C toolchain, GTK 4 and GtkSourceView 5 development
packages, then run the installer again.

Start it afterwards from the app menu or with:

```bash
~/.local/share/relapse-plus/run.sh
```

## Manual install

```bash
git clone https://github.com/ultravibecoderxtfo/Relapse-Plus.git
cd Relapse-Plus
./run.sh                 # build + run
./packaging/install-app.sh   # optional: add to the app menu
```

Arch / CachyOS package names: `gtk4 gtksourceview5 base-devel`. Debian / Ubuntu:
`libgtk-4-dev libgtksourceview-5-dev build-essential pkg-config`.

## Using it

1. Open **Settings → Launcher** and point it at the Relapse launcher binary
   (`relapse-launcher`). The UI has no bundled launcher and does not download
   one.
2. Open the **Client Manager** and press **Paste cookie**. Paste a
   `.ROBLOSECURITY` cookie, name the profile, and save. The account is checked
   against Roblox and the avatar, user ID and display name are stored locally.
3. Press **Launch** on a profile. The launcher starts the profile in the
   background and the UI attaches to its socket automatically. Attach several
   profiles and `Execute` runs in every attached, ready instance.
4. Scripts live in the `scripts/` folder next to the app and appear in the right
   sidebar. Click one to open it, then run it with `Ctrl+Enter`.

### Controls

| Action | Shortcut |
| --- | --- |
| Execute current tab | `Ctrl+Enter` |
| Execute a file from disk | `Ctrl+Shift+Enter` |
| Open file | `Ctrl+O` |
| Save file | `Ctrl+S` |
| New tab | `Ctrl+N` |
| Close tab | `Ctrl+W` |
| Toggle output drawer | `` Ctrl+` `` |
| Settings and protocol trace | `Ctrl+,` |

## Features

- GtkSourceView editor with the Luau language definition, line numbers,
  current-line highlight and a bundled Relapse style scheme.
- Luau-aware autocomplete plus bracket and quote completion.
- Five built-in themes (Graphite, Midnight, Frost, Forest, Ember) and a live
  theme editor for colours, fonts, type size and header/background images.
- Output console with **Roblox** and **UI** filters; script output can be
  redirected into the console instead of Roblox's own console.
- Client Manager with profiles, pasted-cookie logins and automatic account
  checks (user ID, username, avatar).
- Multi-instance launching: attach any number of profiles and execute in all of
  them at once.
- Everything is local: profiles, cookies and scripts stay on your machine.

## Privacy

This repository contains the UI only. No profiles, cookies, account data,
avatars, logs or personal paths are included or sent anywhere. Pasted cookies
are written with `0600` permissions under
`~/.local/share/relapse/profiles/<profile>/` and are used only for the local
Roblox account check and the Relapse launcher.

## Layout

```
src/                 Rust source: UI, protocol client, completion, settings, themes
assets/              Luau language definition, GtkSourceView schemes, app icons
styles/              GtkSourceView syntax scheme
packaging/           desktop entry and user-level installer
style.css            window chrome, colours and spacing
install.sh           one-command installer
PROTOCOL.md          socket wire format
```

## License

MIT — see [LICENSE](LICENSE).
