<p align="center">
  <img src="apps/cli/assets/branding/termkey-icon.svg" alt="TermKey" width="420">
</p>

# TermKey

Local-only, encrypted vault for private keys, seed phrases, and passwords. Run `termkey` for the full-screen TUI, or use subcommands for direct terminal workflows. **XChaCha20-Poly1305** + **Argon2id**. Zero cloud. Zero trust.

- **Vault path:** `~/.termkey/`
- **Secret types:** private keys, seed phrases, passwords
- **Networks:** Ethereum, Bitcoin, Solana, or a custom network label
- **Extras:** encrypted backup/import, optional recovery question, strong password generation for password entries, address derivation for supported crypto entries

---

## Security

| | |
|---|---|
| **XChaCha20-Poly1305** | AEAD cipher with a 192-bit nonce for authenticated encryption |
| **Argon2id** | Memory-hard KDF for deriving encryption keys from your master password |
| **Local-only storage** | Vault data lives under `~/.termkey/` with no cloud sync or remote service |

---

## Install

### macOS

**Homebrew** (recommended; updates with `brew upgrade`):

```bash
brew install ryanonmars/termkey/termkey
```

**Installer DMG:** [Apple Silicon (ARM64)](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-macos-aarch64.dmg) · [Intel (x86_64)](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-macos-x86_64.dmg)

**Direct PKG installer:** [Apple Silicon (ARM64)](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-macos-aarch64-installer.pkg) · [Intel (x86_64)](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-macos-x86_64-installer.pkg)

**Direct ZIP download:** [Apple Silicon (ARM64)](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-macos-aarch64.zip) · [Intel (x86_64)](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-macos-x86_64.zip)

The macOS installer adds `TermKey.app` and `Uninstall TermKey.app` to `/Applications` and also installs the `termkey` CLI to `/usr/local/bin`. To remove the installer-based version later, open `Uninstall TermKey.app` from Applications. Your vault in `~/.termkey` is left untouched.

```bash
unzip termkey-macos-*.zip
chmod +x termkey
sudo mv termkey /usr/local/bin/
# or on Apple Silicon with Homebrew:
sudo mv termkey /opt/homebrew/bin/
```

**Gatekeeper:** If macOS blocks the app, go to **System Settings → Privacy & Security**, scroll to the **Security** section, click **Open Anyway**, then confirm with **Open**. If needed, remove the quarantine flag from the extracted binary with `xattr -d com.apple.quarantine ./termkey`.

### Linux

**Homebrew on Linux:** [brew.sh](https://brew.sh) then:

```bash
brew install ryanonmars/termkey/termkey
```

**Direct download:** [Linux x86_64](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-linux-x86_64.zip)

```bash
unzip termkey-linux-x86_64.zip
chmod +x termkey
sudo mv termkey /usr/local/bin/
```

### Windows

Download and run the installer: [TermKey-Setup.exe](https://github.com/ryanonmars/termkey/releases/latest/download/TermKey-Setup.exe)

No admin required. It installs to `%LOCALAPPDATA%\termkey`, adds `termkey` to your user `PATH`, and includes an uninstaller.

PowerShell bootstrap:

```powershell
iwr https://raw.githubusercontent.com/ryanonmars/termkey/master/apps/cli/scripts/install.ps1 | iex
```

Manual ZIP: [Windows x86_64](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-windows-x86_64.zip)

**SmartScreen:** On first launch click "More info" → "Run anyway", or right-click the `.exe`, open **Properties**, and check **Unblock**.

---

## How It Works

Run `termkey` with no subcommand to open the TUI.

When terminal graphics are supported, TermKey shows a short pre-TUI splash icon before entering the fullscreen app. The current best-effort auto-detected targets are Kitty and Ghostty via the Kitty graphics protocol, plus iTerm2 via its OSC 1337 inline-image protocol. iTerm2 may prompt for permission before displaying inline images. `tmux` is disabled by default because redraws can wipe terminal images. Override detection with `TERMKEY_GRAPHICS=kitty` or `TERMKEY_GRAPHICS=iterm2`, disable it with `TERMKEY_GRAPHICS=off`, and adjust the delay with `TERMKEY_SPLASH_MS=<milliseconds>`.

On first launch, TermKey opens a setup wizard where you:

1. Create your master password
2. Create the local vault
3. Optionally set up a recovery question

After setup, you land on the login screen and then the dashboard.

Inside the TUI you can:

- Add private keys, seed phrases, or passwords
- Generate strong passwords while creating password entries
- Search and filter entries in place
- View secrets or copy them to the clipboard
- Edit, rename, and delete entries
- Export an encrypted backup and import it later
- Change your master password
- Open settings and recovery flows

For crypto entries, TermKey supports Ethereum, Bitcoin, Solana, and custom network labels. Public address derivation is available for supported Ethereum, Bitcoin, and Solana private keys and seed phrases.

For password entries, you can also store optional username and URL metadata alongside the secret.
Both the TUI add form and `termkey add` can generate a strong password for you during entry creation.

---

## CLI Commands

The TUI is the default interface, but the command mode is fully available when you want direct operations:

```bash
termkey init
termkey add
termkey list
termkey view <name-or-index>
termkey edit <name-or-index>
termkey rename <old> <new>
termkey delete <name-or-index>
termkey copy <name-or-index>
termkey search <query>
termkey export <directory>
termkey import <path/to/backup.ck>
termkey passwd
termkey recover
termkey config --show
termkey derive <name-or-index>
```

Notes:

- `termkey export <directory>` writes an encrypted `backup.ck` file into that directory.
- `termkey recover` uses your configured recovery question flow.
- `termkey derive` saves a public address for supported Ethereum, Bitcoin, and Solana key or seed entries.

---

## Keyboard Shortcuts

| | |
|---|---|
| **Navigation** | `↑/↓` move, type a number then press `Enter` to jump to that entry, `Esc` back/clear filter, `/` search, `Shift+F` find/filter |
| **Entry** | `Shift+A` add, `Shift+V` view, `Shift+C` copy, `Shift+E` edit, `Shift+D` delete |
| **Vault** | `Shift+X` export, `Shift+I` import, `Shift+P` change password, `Shift+S` settings |
| **Other** | `?` help, `Shift+Q` quit, `Ctrl+C` quit, `Ctrl+Q` quit, `F1` recovery from the login screen |

---

## Links

- [Releases](https://github.com/ryanonmars/termkey/releases)
- [Issues](https://github.com/ryanonmars/termkey/issues)

**License:** MIT · **Rust**
