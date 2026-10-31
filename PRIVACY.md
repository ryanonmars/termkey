# TermKey Privacy Policy

**Effective date: 2026-10-30**

TermKey is a local encrypted vault with terminal and command-line interfaces.

## Local vault and retention

Your vault is stored locally under `~/.termkey/`, or a location you configure. Saved entries remain in your vault until you delete them or remove the vault. Encrypted backups remain wherever you export them.

`termkey reset` removes local vault data and settings while preserving encrypted backups and keeping TermKey installed. `termkey uninstall` removes the application while preserving vault data, settings, and encrypted backups. Installer installations use their companion uninstaller.

## Network use and disclosures

TermKey checks GitHub's latest-release API when it performs an update check, including during normal application startup and `termkey update`. That request identifies the application version in its User-Agent and asks GitHub for release metadata. It does not include vault secrets. Opening release or download links also connects you to GitHub. GitHub and your network provider may receive ordinary connection information such as your IP address under their own policies.

When you choose to open a URL stored with an entry, TermKey opens that URL in your default browser. The destination site receives ordinary connection information under its own policies.

TermKey does not sell personal information or use analytics or advertising SDKs. Vault secrets, master passwords, secondary passwords, and generated passwords are not transmitted to the developer or to third parties.

## Contact

For privacy questions or requests, open an issue at [GitHub Issues](https://github.com/ryanonmars/termkey/issues).
