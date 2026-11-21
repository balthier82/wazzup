# Wazzup

WhatsApp on your GNOME desktop: WhatsApp Web inside a native GTK 4 and libadwaita window, powered by the Chromium Embedded Framework so voice and video calls work.

Wazzup is an independent project and is not affiliated with WhatsApp or Meta.

## Features

- Voice and video calls, screen sharing, voice messages
- Native GNOME notifications that open the right chat, unread badge on the dock and a top bar icon
- Passcode lock with automatic locking on start, when hidden or after idle time
- Chat blur until you point at them, per-device permissions for microphone, camera and screen
- Downloads panel, native file dialogs, spell checking, HTTP and SOCKS5 proxy
- `whatsapp:` and `wa.me` links, `--new-chat`, single instance and separate profiles
- Light, dark or system style, interface in English, Italian, French, German and Spanish

## Install

Download the AppImage for your architecture (`x86_64` or `aarch64`) from the [releases page](https://github.com/balthier82/wazzup/releases), make it executable and run it:

```sh
chmod +x Wazzup-*-x86_64.AppImage
./Wazzup-*-x86_64.AppImage
```

Requirements:

- A distribution with glibc 2.39 or newer (Fedora 40+, Ubuntu 24.04+, Debian 13+)
- XWayland, used by the web engine for the clipboard
- On GNOME, the AppIndicator extension to see the top bar icon

## Command line

```text
wazzup [--background] [--new-chat NUMBER|LINK] [--preferences] [--quit] [LINK...]
```

Set `WAZZUP_PROFILE=name` to run a separate account with its own data.

## Building

```sh
cargo install export-cef-dir --version "156.7.0+156.0.31"
export-cef-dir --force ~/.local/share/cef
./waz-cmd.py dev
```

`waz-cmd.py` gathers the everyday tasks:

| Command | What it does |
| --- | --- |
| `./waz-cmd.py dev [--profile NAME] [-- ARGS]` | build and start the development version |
| `./waz-cmd.py check` | formatting, lints, tests, translations and metadata |
| `./waz-cmd.py appimage` | build an AppImage for this machine into `dist/` |
| `./waz-cmd.py bump patch\|minor\|major\|X.Y.Z [--push]` | raise the version, commit and tag it |
| `./waz-cmd.py release patch\|minor\|major\|X.Y.Z` | same as bump, then push so GitHub builds both architectures and prepares a draft release |
| `./waz-cmd.py rerun` | run the latest GitHub release build again |
| `./waz-cmd.py status` | recent GitHub builds and releases |

Translations live in `po/`; run `tools/extract-strings.py` to refresh the template and list missing messages.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
