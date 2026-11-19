# clean

**Mail, without the noise.** A calm, private, native mail client for Omarchy.

Built in Rust with [GPUI Kit](https://gpui-kit.com). Three theme modes — **White**,
**Black**, and **Omarchy** (live desktop palette) — plus a first-run appearance and
account setup.

![clean — inbox, reader and calendar rail](docs/screenshot.png)

## Features

- Three-pane mail layout with an Upcoming calendar rail, modelled on the reference UI.
- White / Black / Omarchy themes with nine accent colours. Blue is the default and
  follows Omarchy's live blue token; in Omarchy mode the app re-skins itself when the
  desktop theme changes.
- First-run setup: appearance → connect account. Reachable later from Settings → Accounts.
- Google (OAuth 2.0 Desktop client), iCloud Mail (app-specific password), and Proton
  (via Proton Mail Bridge) account support.
- Local-first: SQLite cache with FTS5 full-text search, credentials in the GNOME/Secret
  Service keyring, no telemetry.
- IMAP sync with lazy body fetching, mark-as-seen, SMTP send, inline reply/compose.
- Local calendar events with a New Event sheet and one-click "add to calendar" from a
  detected date in a message.
- Custom-drawn window chrome, spring animations and keyboard navigation
  (`J`/`K`, `Enter`, `Esc`, `⌘K`, `⌘N`, `⌘R`, `⌘,`).

## Build

Requirements on Arch / Omarchy:

```bash
sudo pacman -S --needed base-devel clang cmake pkgconf \
  wayland libxkbcommon fontconfig freetype2 \
  vulkan-icd-loader openssl zstd alsa-lib webkit2gtk-4.1
```

Then:

```bash
make run          # cargo build + launch
cargo test        # engine tests
```

The first GPUI build is heavy. `make` uses `CARGO_BUILD_JOBS=8`; adjust for your machine.

### Embedding a Google OAuth client (for distribution)

By default `clean` asks users for their own Google Desktop OAuth client. To ship a
build where users just click **Continue with Google**, bake your own client in at
build time:

```bash
CLEAN_GOOGLE_CLIENT_ID='xxxx.apps.googleusercontent.com' \
CLEAN_GOOGLE_CLIENT_SECRET='GOCSPX-xxxx' \
  CARGO_BUILD_JOBS=8 cargo build --release
```

Both values are compiled into the binary as public Desktop-client identifiers and
stored in the keyring on first connect. The manual fields in Setup remain as an
override. Before releasing widely, publish and verify your OAuth consent screen —
the IMAP scope `https://mail.google.com/` is restricted by Google.

There is also a helper that prompts for the values, saves them locally with mode
`600`, and builds for you:

```bash
./build-google.sh
```


## Install

User-level, no root:

```bash
./install.sh
# install.sh builds --release and installs into ~/.local
```

Arch package:

```bash
make tarball
cp clean-0.1.0.tar.gz packaging/
cd packaging && makepkg -si
```

The app registers as `dev.alvyn.clean` and ships a `.desktop` entry and hicolor icons.

## CLI

The same binary exposes a headless engine, useful for testing without the GUI:

```bash
clean status
clean auth google        # needs your own OAuth Desktop client ID/secret
clean auth apple         # iCloud app-specific password
clean auth proton        # requires Proton Mail Bridge
clean sync
clean search "weekend"
clean folders
clean ui                 # launch the window explicitly
```

## Account notes

| Provider | What you need |
|---|---|
| Google | An OAuth 2.0 **Desktop** client from Google Cloud, with the Gmail API enabled. Add your address as a test user. Client ID/secret are stored in the keyring, not in `config.toml`. |
| Apple | iCloud Mail with an **app-specific password** from appleid.apple.com. This is not "Sign in with Apple". |
| Proton | **Proton Mail Bridge** running locally; copy the Bridge username/password from Mailbox details. Proton does not offer a direct mail API. |

Without an account, `clean` opens with demo mail so the full UI can be explored.

## Data locations

- Config: `~/.config/clean/config.toml`
- Cache + calendar: `~/.local/share/clean/clean.db`
- State: `~/.local/state/clean`
- Secrets: your desktop keyring (Secret Service)

## Design

See [PLAN.md](PLAN.md) for the original blueprint, palettes, motion specs and milestones.
Branding, copy and artwork are original to `clean`; the reference UI was used only for
layout and interaction inspiration.
