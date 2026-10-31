<p align="center">
  <img src="apps/cli/assets/branding/termkey-icon.svg" alt="TermKey" width="420">
</p>

# TermKey

Local-only, encrypted vault for private keys, seed phrases, and passwords. Run `termkey` for the full-screen TUI, or use subcommands for direct terminal workflows. **XChaCha20-Poly1305** + **Argon2id**. Zero cloud. Zero trust.

- **Vault path:** platform-specific private local storage (see below)
- **Secret types:** private keys, seed phrases, passwords
- **Networks:** Ethereum, Bitcoin, Solana, or a custom network label
- **Extras:** encrypted backup/import, optional random recovery phrase, strong password generation for password entries, address derivation for supported crypto entries

---

## Security

| | |
|---|---|
| **XChaCha20-Poly1305** | AEAD cipher with a 192-bit nonce for authenticated encryption |
| **Argon2id** | Memory-hard KDF for deriving encryption keys from your master password |
| **Local-only storage** | Vault data stays in private local files; no TermKey cloud sync or account |

---

## Platforms and release status

The existing supported release is **macOS Apple Silicon, macOS 11 or later**.
This branch prepares the following release targets. New platforms are release
candidates until their native CI, security tests, and clean-machine checks pass;
a successful cross-compilation alone does not establish runtime support.

| Platform | Rust target | Release ZIP | Verification required before release |
|---|---|---|---|
| macOS Apple Silicon | `aarch64-apple-darwin` | `termkey-macos-aarch64.zip` | Native tests; signed/notarized download and installer checks |
| macOS Intel | `x86_64-apple-darwin` | `termkey-macos-x64.zip` | Native Intel tests; signed/notarized ZIP; macOS 11 minimum |
| Linux x86_64 | `x86_64-unknown-linux-musl` | `termkey-linux-x64.zip` | Native MUSL tests; static linkage; clean Linux and clipboard checks |
| Linux ARM64 | `aarch64-unknown-linux-musl` | `termkey-linux-arm64.zip` | Native ARM64 MUSL tests; static linkage; clean Linux and clipboard checks |
| Windows x86_64 | `x86_64-pc-windows-msvc` | `termkey-windows-x64.zip` | Native Windows security tests; Windows Terminal/PowerShell; standard-user checks |

Each ZIP includes the native executable, `LICENSE`, dependency notices in
`THIRD-PARTY-LICENSES.txt`, and platform-specific `README.txt` instructions.

The Apple Silicon ZIP name is deliberately unchanged for existing download and
Homebrew users. Other names use `x64`/`arm64`; release tags version the download
URLs. The existing signed/notarized Apple Silicon DMG remains available. New ZIPs
are available only after an authorized, verified release is published.

## Install

### Homebrew — macOS and Linux

```bash
brew install ryanonmars/termkey/termkey
termkey
```

The prepared formula selects the same native ZIP offered for manual downloads.
The live tap changes only when a verified release is authorized and published.
Homebrew has its own OS/architecture support policy; this does not define the
minimum OS of a standalone TermKey executable. In particular, Intel macOS and
older macOS versions have limited upstream Homebrew support. Use the matching
ZIP when Homebrew is unavailable. See [Homebrew support tiers](https://docs.brew.sh/Support-Tiers).

### Existing Apple Silicon installer

[Download the signed/notarized DMG](https://github.com/ryanonmars/termkey/releases/latest/download/termkey-macos-aarch64.dmg).
Open it and run the included installer. It installs the app launcher, command-line
tool, and companion uninstaller. This installer remains Apple Silicon only.

### Manual ZIP — macOS and Linux

Download the matching ZIP and `SHA256SUMS` from the **same** [GitHub Release](https://github.com/ryanonmars/termkey/releases).
Check the ZIP's SHA-256 against its line in the manifest before extraction:

```bash
# macOS example; use termkey-macos-x64.zip on Intel
shasum -a 256 termkey-macos-aarch64.zip

# Linux example; use termkey-linux-arm64.zip on ARM64
sha256sum termkey-linux-x64.zip
```

From the download folder, extract your selected ZIP, then install its executable:

```bash
# Substitute the matching ZIP filename from the table above
unzip termkey-linux-x64.zip
mkdir -p "$HOME/.local/bin"
chmod 755 termkey
cp termkey "$HOME/.local/bin/termkey"
export PATH="$HOME/.local/bin:$PATH"
termkey --help
termkey
```

Add the `export PATH=...` line to your shell startup file, such as `~/.zshrc` or
`~/.bashrc`, for future terminals. ZIP extractors may lose executable permissions;
`chmod 755` restores them. No Rust installation is needed. Production macOS ZIPs
must be signed and notarized; keep Gatekeeper enabled and verify the release's
origin if macOS warns about a download.

### Portable ZIP — Windows x86_64

Use Windows Terminal with PowerShell. Download `termkey-windows-x64.zip` and
`SHA256SUMS` from the same release. Compare the displayed SHA-256 to the manifest:

```powershell
Get-FileHash .\termkey-windows-x64.zip -Algorithm SHA256
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\TermKey'
Expand-Archive .\termkey-windows-x64.zip -DestinationPath $installDir
& (Join-Path $installDir 'termkey.exe') --help
& (Join-Path $installDir 'termkey.exe')
```

Add `$installDir` to **your user PATH** through Windows Settings, or run:

```powershell
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($userPath -split ';') -notcontains $installDir) {
    [Environment]::SetEnvironmentVariable('Path', "$installDir;$userPath", 'User')
}
$env:Path = "$installDir;$env:Path"
termkey --help
```

Administrator rights, MSI, WinGet, Rust, and a separate Visual C++ runtime are not
required by the portable distribution design. Release verification must confirm
this on a clean standard-user machine. New downloads may trigger Windows
SmartScreen/reputation warnings, even when signed. Check the source and checksum;
do not disable Windows security features. Windows client-version and desktop
verification are required before claiming release support.

## Vault locations and permissions

| Platform or override | Vault data | Configuration/recovery settings |
|---|---|---|
| macOS | `~/.termkey/vault.ck` | `~/.termkey/config.json` |
| Windows | `%USERPROFILE%\.termkey\vault.ck` | `%USERPROFILE%\.termkey\config.json` |
| New Linux installation | `$XDG_DATA_HOME/termkey/vault.ck`, default `~/.local/share/termkey/vault.ck` | `$XDG_CONFIG_HOME/termkey/config.json`, default `~/.config/termkey/config.json` |
| Existing Linux `~/.termkey` installation | Existing files stay in place | Existing co-located configuration stays in place |
| Absolute `TERMKEY_VAULT_DIR` | `<directory>/vault.ck` | `<directory>/config.json` |

Use a private local directory, separate from the application installation folder.
TermKey enforces owner-restricted Unix permissions or Windows current-user ACLs
for sensitive files and reports protection failures. Filesystems that cannot
provide the required protection are not a safe vault/export destination. Copying
files manually may change their permissions; reapply private destination access.
Do not share a writable vault directory between accounts or place it in a
synchronized/cloud folder if you require local-only storage.

## Updating, reset, and uninstall

`termkey update` uses Homebrew for a detected Homebrew installation; portable
installations receive the matching release download instructions. With Homebrew:

```bash
brew upgrade termkey
```

For a ZIP upgrade, close TermKey, verify the new matching archive, and replace
only the installed executable. Preserve your vault and configuration. On Windows,
close all running copies before replacing `termkey.exe`.

`termkey reset` permanently removes the active vault and settings after
confirmation, keeps the application installed, and preserves exported backups.
Settings also offers **Reset vault and settings**. Type `DELETE` to confirm.

`termkey uninstall` preserves vault data, settings, and backups. Homebrew uses its
package manager; the macOS installer uses its companion uninstaller. Portable
Windows removal prints a PowerShell `Remove-Item -LiteralPath` command to run
after closing all copies; the executable remains installed until you run that
command. Follow the platform-specific instructions printed by TermKey. Before and after removal,
TermKey identifies retained vault data and provides a cleanup command that still
works without the application. Delete unwanted exported backups separately.

## Moving an encrypted vault between operating systems

The encryption and file formats are unchanged and contain no Keychain, DPAPI,
or hardware binding. Cross-platform fixture exchange is a required release gate.

The simplest transfer is an encrypted backup:

1. Export with `termkey export <directory>` and choose a backup password.
2. Move the encrypted `.termkey` file to the destination.
3. Create/unlock the destination vault and run `termkey import <backup-file>`.
4. Enter the **backup password**, not necessarily either vault's master password.
5. Configure recovery for the destination vault. Secondary passwords on imported
   protected entries remain required.

Exports contain entries, not local configuration or the source recovery setup.
Do not rename an exported backup to `vault.ck`: backups and raw vaults have
different magic values.

For a raw-vault transfer, close every TermKey instance before copying `vault.ck`.
Preserve its matching recovery configuration if you need the same recovery phrase.
Do not blindly reuse an old absolute `vault_path` from another OS: update the
local configuration/override to the destination path. Restore private file access
before opening the copy. Existing legacy decoding and migration remain supported;
no conversion or encryption change is required for another platform.

## Security and terminal limitations

Master passwords and secrets are not sent in update requests. GitHub update
checks can occur at startup; vault operations otherwise require no online account.
Zeroization reduces exposure, but vault allocations are not currently locked in
RAM: paging, hibernation, screenshots, and privileged tools can retain sensitive
data. Unix startup disables ordinary core dumps with `RLIMIT_CORE`; Windows asks
Windows Error Reporting to exclude heap data and disable crash/hang snapshots.
TermKey reports failures to apply these controls. Administrator dump policies and
external debuggers can bypass them; they do not guarantee that no dump is possible.

Private temporary files are flushed before replacement. Parent-directory syncing
is best effort on Unix and is not performed on Windows. Atomic replacement avoids
partial file writes but cannot guarantee durability through power loss on every
filesystem. Keep independent encrypted backups.

Before clearing a secret, TermKey checks its copy token and the current clipboard
text. External applications can race that check or copy identical text, so clearing
is best effort. It cannot erase Windows clipboard history/cloud clipboard,
third-party clipboard managers, or an external application's retained copy.
Linux requires an available X11 or supported Wayland clipboard protocol; some
compositors and headless/SSH sessions provide none. An unavailable clipboard
returns an error without printing the secret; other vault operations remain usable.
Optional Kitty/iTerm graphics fall back to text. Set `TERMKEY_GRAPHICS=off` or
`TERMKEY_SPLASH_MS=0` to disable the splash graphics/delay. Use an interactive
terminal for hidden password prompts and the TUI; do not put passwords in shell
arguments or command history.

---

## How It Works

Run `termkey` with no subcommand to open the TUI.

On first launch, TermKey opens a setup wizard where you:

1. Create your master password
2. Create the local vault
3. Optionally set up a random 24-word recovery phrase

After setup, you land on the login screen and then the dashboard.

The recovery phrase is shown once. Store it offline and never share it. Vaults
upgraded from the legacy format no longer use security-question recovery; after
the first successful upgrade, follow the unlock notice and configure a new
recovery phrase in Settings.

Changing the master password or recovering the vault rotates its encryption key
and disables the previous recovery phrase. Set up a new recovery phrase in Settings
after either operation.

Inside the TUI you can:

- Add private keys, seed phrases, or passwords
- Generate strong passwords while creating password entries
- Search and filter entries in place
- View secrets or copy them to the clipboard
- Edit, rename, and delete entries
- Export an encrypted backup and import it later
- Change your master password
- Open settings and recovery flows
- Reset the vault from Settings → Reset vault and settings (type `DELETE` to confirm)

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
termkey import <path/to/backup.termkey>
termkey passwd
termkey recover
termkey config --show
termkey update
termkey derive <name-or-index>
termkey reset
termkey uninstall
```

Notes:

- `termkey export <directory>` prompts for an editable backup name (default: `backup`) and writes an encrypted `<name>.termkey` file into that directory.
- Imports remain compatible with existing `.ck` backup files.
- `termkey recover` uses your configured 24-word recovery phrase.
- `termkey reset` deletes vault data and settings after confirmation, preserves encrypted backups, and keeps TermKey installed. Run `termkey` afterward to set up a new vault.
- `termkey uninstall` removes TermKey while preserving vault data and settings. Homebrew installs use `brew uninstall termkey`; macOS installer installs launch the companion uninstaller. The confirmation and completion messages show the retained vault location and a standalone cleanup command that works after TermKey is gone. To delete vault data first, run `termkey reset` before uninstalling.
- `termkey update` checks the latest GitHub release. On Homebrew installs it runs the Homebrew update flow; on installer/manual/source installs it prints the correct release download page.
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
