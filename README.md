# TermKey

Local-only, encrypted TUI vault for private keys and seed phrases. **XChaCha20-Poly1305** + **Argon2id**. Zero cloud. Zero trust.

- **Vault:** `~/.termkey/` — local storage only, no network, no cloud sync
- **TUI:** Run `termkey`, use keyboard shortcuts (Shift+letter for actions)

---

## Security

| | |
|---|---|
| **XChaCha20-Poly1305** | AEAD cipher, 192-bit nonce — authenticated, tamper-evident |
| **Argon2id** | Memory-hard KDF — resistant to GPU & ASIC attacks |
| **~/.termkey/** | Local-only storage — no network access |

---

## Install

### macOS

**Homebrew** (recommended; updates with `brew upgrade`):

```bash
brew install ryanonmars/termkey/termkey
```

**Direct download:** [Apple Silicon (ARM64)](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-macos-aarch64.zip) · [Intel (x86_64)](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-macos-x86_64.zip)

```bash
unzip termkey-macos-*.zip
chmod +x termkey
sudo mv termkey /usr/local/bin/
```

**Gatekeeper:** If macOS blocks the app, go to **System Settings → Privacy & Security**, scroll to the **Security** section — the blocked app appears there. Click **Open Anyway**, then confirm with **Open**. Optionally, from the folder where the binary is: `xattr -d com.apple.quarantine ./termkey` (if you see "No such xattr", skip that step).

### Linux

**Homebrew (Linuxbrew):** [brew.sh](https://brew.sh) then:

```bash
brew install ryanonmars/termkey/termkey
```

**Direct download:** [Linux x86_64](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-linux-x86_64.zip)

```bash
unzip termkey-linux-x86_64.zip
chmod +x termkey
sudo mv termkey /usr/local/bin/
```

### Windows Install (Recommended)

Download and run the installer: [TermKey-Setup.exe](https://github.com/ryanonmars/termkey/releases/latest/download/TermKey-Setup.exe)

No admin required. It installs to `LOCALAPPDATA\termkey`, adds `termkey` to your user `PATH`, and includes an uninstaller.

PowerShell bootstrap:

```powershell
iwr https://raw.githubusercontent.com/ryanonmars/termkey/main/scripts/install.ps1 | iex
```

Manual ZIP: [Windows x86_64](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-windows-x86_64.zip)

**SmartScreen:** On first launch click "More info" → "Run anyway", or right-click the .exe → Properties → check **Unblock**.

---

## Usage

1. **Launch:** Run `termkey`. First launch runs the setup wizard (create vault + master password); after that you get the login screen.
2. **Dashboard:** Entry list is the home screen. **↑/↓** navigate, **/** search/filter, **Enter** view selected entry.
3. **Add:** **Shift+A** — fill the form; the secret field is hidden and never touches shell history.
4. **View / copy:** **Shift+V** reveal in TUI, **Shift+C** copy to clipboard (auto-clears after 10s).
5. **Edit / delete:** **Shift+E** edit, **Shift+D** delete (confirmation required). **Shift+X** export vault, **Shift+I** import backup.
6. **Help:** **?** shows the full shortcut list. **Shift+Q** quit. **Ctrl+C** or **Ctrl+Q** quit from anywhere.

---

## Keyboard shortcuts

| | |
|---|--|
| **Navigation** | ↑/↓ move, Enter select, Esc back/clear filter, / search, **Shift+F** find/filter |
| **Entry** | **Shift+A** add, **Shift+V** view, **Shift+C** copy, **Shift+E** edit, **Shift+D** delete |
| **Vault** | **Shift+X** export, **Shift+I** import, **Shift+P** change password, **Shift+S** settings |
| **Other** | **?** help, **Shift+Q** quit, **F1** recovery (login screen) |

---

## Links

- [Releases](https://github.com/ryanonmars/termkey/releases)
- [Issues](https://github.com/ryanonmars/termkey/issues)

**License:** MIT · **Rust**
