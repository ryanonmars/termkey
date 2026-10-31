# TermKey Privacy Policy

**Effective date: 2026-10-31**

TermKey is a local encrypted vault with terminal and command-line interfaces.

## Local vault and retention

Your vault is stored locally under `~/.termkey/` on macOS, `%USERPROFILE%\.termkey` on Windows, or the XDG data directory on new Linux installations (`~/.local/share/termkey` by default). Linux configuration/recovery settings use the XDG configuration directory (`~/.config/termkey` by default); existing Linux `~/.termkey` installations remain in place. An absolute `TERMKEY_VAULT_DIR` override co-locates vault and configuration in the chosen directory. Saved entries remain in your vault until you delete them or remove the vault. Encrypted backups remain wherever you export them.

`termkey reset` removes local vault data and settings while preserving encrypted backups and keeping TermKey installed. `termkey uninstall` removes the application while preserving vault data, settings, and encrypted backups. macOS installer installations use their companion uninstaller. Portable Windows installations provide close-and-remove instructions. Uninstall identifies retained data and supplies a platform-specific cleanup command.

## Network use and disclosures

TermKey checks GitHub's latest-release API when it performs an update check, including during normal application startup and `termkey update`. That request identifies the application version in its User-Agent and asks GitHub for release metadata. It does not include vault secrets. Opening release or download links also connects you to GitHub. GitHub and your network provider may receive ordinary connection information such as your IP address under their own policies.

When you choose to open a URL stored with an entry, TermKey opens that URL in your default browser. The destination site receives ordinary connection information under its own policies.

TermKey does not sell personal information or use analytics or advertising SDKs. Vault secrets, master passwords, secondary passwords, and generated passwords are not transmitted to the developer or to third parties.

## Clipboard and operating-system services

Clipboard operations use the local operating system. Timed clearing is best effort
and does not erase copies retained by clipboard managers, Windows clipboard
history/cloud clipboard, or other applications. Any OS-level clipboard
synchronization is controlled by the operating system, not a TermKey service.
Linux clipboard access depends on an available X11 or supported Wayland session;
headless environments may have no clipboard. The application does not send a
secret to stdout or a network service as a clipboard fallback.

## Contact

For privacy questions or requests, open an issue at [GitHub Issues](https://github.com/ryanonmars/termkey/issues).
