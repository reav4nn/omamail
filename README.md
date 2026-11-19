<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="share/logo-white.svg">
  <img src="share/logo-black.svg" width="104" alt="Omamail logo">
</picture>

# Omamail

**A light, keyboard-driven Gmail client for the Omarchy shell.**

[![Version](https://img.shields.io/badge/version-1.2.10-blue)](manifest.json)
[![Platform](https://img.shields.io/badge/platform-Omarchy-1793d1)](https://omarchy.org)
[![Backend](https://img.shields.io/badge/backend-Rust-b7410e)](backend)
[![License](https://img.shields.io/badge/license-MIT-lightgrey)](LICENSE)

![Omamail in Tokyo Night, Catppuccin Latte and Gruvbox](preview.png)

<sub>Sample mail, from the demo mailbox.</sub>

</div>

Omamail puts your Gmail inbox one key away on the Omarchy desktop. It is a
mail panel built from the shell's own components, so it looks like part of
your desktop in every theme, with desktop notifications for new mail and
verification codes you copy with one click. The Rust backend is about 3 MB
and uses about 7 MB of memory.

## Contents

- [Highlights](#highlights)
- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
- [Updates](#updates)
- [Privacy and safety](#privacy-and-safety)
- [Uninstall](#uninstall)
- [Development](#development)
- [License](#license)

## Highlights

| Verification codes | Compose and reply | Gmail search |
|:---:|:---:|:---:|
| ![A sign-in code mail in Rose Pine](docs/images/code.png) | ![Replying to a mail in Gruvbox](docs/images/compose.png) | ![Search results in Kanagawa](docs/images/search.png) |
| A new mail with a one-time code shows the code in its notification; a click copies it. | `c` to compose, `r`, `a` and `f` to reply, reply all and forward; `Ctrl+Enter` sends. | `/` searches with Gmail's own syntax, such as `from:x has:attachment`. |

- **Part of the shell.** Built from `omarchy-shell`'s own components and theme
  tokens, so colours, font and rounding change with the theme.
- **Notifications.** A user service watches each saved inbox independently with IMAP IDLE and
  posts notifications labelled with the receiving account. Clicking a message selects that account and opens the message.
  After a suspend or a Wi-Fi or VPN change it reconnects within seconds.
  Gmail itself usually announces new mail over IMAP 10–50 seconds after it
  arrives, for any client.
- **Verification codes.** When a new mail carries a one-time code, the
  notification shows it and a click copies only that account’s code. Clicks for removed accounts or an unknown mailbox generation do nothing. The copy is marked
  sensitive and stays out of clipboard history.
- **Keyboard first.** Move with `j`/`k`, search with `/`, compose with `c`.
  See [Usage](#usage).
- **Opens on the last list.** The newest 200 headers of each folder you
  open are kept in `~/.local/state/omamail/accounts/<id>/list.json` (readable only by
  you), so the list is there when the window opens and settles when Gmail
  replies. Starring or moving a saved row waits until that reply confirms
  the mailbox. Removing an account deletes its file.
- **Light.** A ~3 MB Rust backend using about 7 MB of RAM. Closing the
  window ends it; only the notification service keeps running.
- **Careful with mail.** HTML is sanitized, remote images load only when
  you ask, and attachments never overwrite a file or run as a program. See
  [Privacy and safety](#privacy-and-safety).

## Requirements

| | |
|---|---|
| **Desktop** | [Omarchy](https://omarchy.org) with `omarchy-shell`. The panel is built from the shell's components, so other desktops need a port. |
| **Account** | A Gmail account with 2-Step Verification and an [app password](https://myaccount.google.com/apppasswords). |
| **Build** | Rust and Cargo, available through Omarchy’s development tools. |
| **Attachments** | zenity, for the attachment file picker. |

Install these prerequisites before running `install.sh`.

## Installation

```bash
omarchy plugin add https://github.com/reav4nn/omamail.git --enable
~/.config/omarchy/plugins/reavann.omamail/install.sh
```

`omarchy plugin add` clones the plugin; `install.sh` builds the backend into
`~/.local/lib/omamail/` and installs the launcher entry, the icon and the
notification service.

### Sign in

Omamail signs in with a Gmail **app password**. Turn on 2-Step
Verification, then create one at <https://myaccount.google.com/apppasswords>.
The password is kept in the desktop keyring, never in a file.

Click the address in the sidebar to **add**, **switch**, **sign in again** or
**remove** an account. The last selection is restored when Omamail opens.
Choose *Saved accounts* on the sign-in screen if one password is missing or
rejected. Adding an address again (regardless of case) updates that account.
An existing single-account installation keeps its server settings and keyring entry.

Every composer shows **From**. Switching starts at the new inbox, with no
previous search, open message or undo. If there is a draft, Omamail asks
before discarding it. During a send, account changes wait. Removing the last
account returns to sign-in; it does not delete messages from Gmail.

## Usage

Open Omamail from the app launcher, or bind a key in
`~/.config/hypr/bindings.lua` to:

```bash
omarchy-shell shell toggle reavann.omamail '{}'
```

| Key | Action |
|---|---|
| `j` / `k`, arrows | Next / previous message (opens it) |
| `J` / `K` | Next / previous folder |
| `Enter` | Open the selected message |
| `Space` / `Shift+Space` | Scroll the message |
| `/` | Search (Gmail syntax: `from:x has:attachment`) |
| `c` | Compose |
| `r` / `a` / `f` | Reply / reply all / forward |
| `e` | Archive |
| `#`, `Delete` | Move to Trash |
| `s` | Star |
| `u` | Toggle read |
| `i` | Load remote images |
| `z` | Undo the last archive or move (while the status bar offers it) |
| `R` | Refresh |
| `?` | About: version, author, source and license |
| `Esc` | Clear search, then close |
| `Ctrl+Enter` | Send (composer) |

## Updates

Once a day a user timer checks for new commits and posts a notification
listing them. Click it, or run:

```bash
~/.config/omarchy/plugins/reavann.omamail/update.sh
```

This runs `omarchy plugin update`, which shows the diff before anything is
installed. Read changes to `backend/` and `install.sh` before accepting:
`install.sh` builds that code. Only then does it rebuild and restart the
shell. The daily check fetches into `~/.cache/omamail/update.git` and never
touches the plugin folder. To turn it off for good:

```bash
systemctl --user disable --now omamail-update-check.timer
```

## Privacy and safety

Everything runs as your own user, and nothing in your existing configuration
changes besides enabling the plugin.

| | |
|---|---|
| **Network** | `imap.gmail.com:993` and `smtp.gmail.com:465`, TLS only. A daily `git fetch` of this repository for the update check. |
| **Secrets** | The app password lives only in the keyring (`secret-tool`): never in argv, a file or a log. Notifications go straight to D-Bus and codes are piped to `wl-copy`, so senders, subjects and codes never appear in process arguments. |
| **Files** | Backend in `~/.local/lib/omamail/`, account settings in `~/.config/omamail/`, image, build and update caches in `~/.cache/omamail/`, service state and saved headers in `~/.local/state/omamail/`, attachments in `$XDG_DOWNLOAD_DIR` (or `~/Downloads`). |
| **Desktop** | Launcher entry and icon in `~/.local/share/`; `omamail-watch.service`, `omamail-update-check.service` and `.timer` in `~/.config/systemd/user/`. |

How mail is handled:

- **HTML** is sanitized (ammonia) to the subset Qt renders: no scripts,
  forms, iframes, inline styles or event handlers. Subjects and names are
  always plain text.
- **Links** open only for `http(s)` and `mailto`; the target shows in the
  status bar on hover.
- **Remote images** stay blocked until you press *Load images*, so opening a
  mail does not ping trackers. They are then fetched from public addresses
  only (no LAN, localhost or Tailscale), at most 40 images of 5 MB each. As
  in any mail client, the sender's server sees your IP, and `http://`
  images travel unencrypted.
- **Attachments** are saved under a sanitized name and never overwrite a
  file. *Save and open* opens only common document, image and media types,
  checked by extension and by content: a JAR named `invoice.pdf` is saved,
  not run.
- **Verification codes** stay out of clipboard history, but do appear in
  Omarchy's notification history.
- **Removing an account** deletes its password, settings, cached headers and
  images, watcher position and saved codes. Downloads stay in your Downloads
  folder. Pre-upgrade files without verifiable ownership (including the old
  shared image cache) are never reused; uninstall removes those files.

## Uninstall

```bash
systemctl --user disable --now omamail-watch.service omamail-update-check.timer
omarchy plugin remove reavann.omamail
rm -rf ~/.local/lib/omamail ~/.config/omamail ~/.cache/omamail ~/.local/state/omamail
rm ~/.config/systemd/user/omamail-watch.service ~/.config/systemd/user/omamail-update-check.{service,timer} \
   ~/.local/share/applications/omamail.desktop \
   ~/.local/share/icons/hicolor/scalable/apps/omamail.svg
secret-tool clear application omamail
```

## Development

```
Panel.qml, Backend.qml, Format.js, components/   QML panel (Panel.qml is the entry point)
backend/                                         Rust: IMAP/SMTP, MIME, sanitizer, JSON-lines server, watcher
share/                                           launcher entry, icon, logo, systemd units
install.sh, update.sh                            build and install; update
tools/make-preview.sh                            regenerate preview.png
docs/images/                                     README screenshots, from the demo mailbox
```

From a checkout elsewhere, `./install.sh` builds the backend and copies the
plugin into `~/.config/omarchy/plugins/reavann.omamail`. Run it after each
change, and restart the shell for QML changes (it caches compiled QML):

```bash
cd backend && cargo test --locked && cd ..
./install.sh
omarchy restart shell
```

For the offline multi-account checks, run these from the repository root:

```bash
(cd backend && cargo test --locked && cargo test --locked --features demo && cargo build --locked)
node --test tests/account-panel.mjs
python3 tests/loopback-imap.py backend/target/debug/omamail
```

The last check uses Python, OpenSSL and a temporary loopback TLS server.
The QML controller tests do not render the desktop. On Omarchy, the `demo`
build starts with two synthetic accounts; use `demo` as the app password to
exercise adding another. It never signs in to Gmail or uses your keyring.

The backend speaks one JSON object per line on stdio. Start with
`{"id":1,"cmd":"status"}`, then include the returned `session` value on every
account or mailbox command. Obtain the new token after a successful change;
commands from an earlier session or backend process are rejected. See
`backend/src/server.rs`. The rules every change follows are in
[AGENTS.md](AGENTS.md); how to report a bug or send a change is in
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE). Omamail is not affiliated with, made or endorsed by Google.
