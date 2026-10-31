TermKey for macOS
================

This disk image contains Install TermKey.pkg, the signed and notarized installer for
TermKey, its command-line tool, and its uninstaller.

Requirements
------------

This installer supports Apple Silicon Macs running macOS 11 or newer.
Intel Mac, Linux, and Windows portable distributions use separate native ZIPs;
this disk image does not install those versions.

Install
-------

1. Open Install TermKey.pkg.
2. Follow the macOS installer steps.
3. The installer places TermKey.app and Uninstall TermKey.app in /Applications.
4. It also places termkey at /usr/local/bin/termkey.
5. Open a new Terminal window and run termkey normally.

Uninstall
---------

To remove TermKey later, open Uninstall TermKey.app from /Applications. It removes
the app bundle, CLI binary, and installer receipt. Your ~/.termkey vault data is left untouched.

The uninstaller clearly identifies the retained vault folder and provides a
copyable command to permanently remove its vault data and settings afterward.
Exported backups are preserved; delete unwanted backup files separately.
You can also run termkey reset before uninstalling.

Portable ZIPs and integrity
--------------------------

The Apple Silicon portable ZIP remains named termkey-macos-aarch64.zip for
compatibility. Intel uses termkey-macos-x64.zip. Download SHA256SUMS from the
same release and compare the archive's SHA-256 before extracting. Each ZIP
contains termkey, LICENSE, THIRD-PARTY-LICENSES.txt, and README.txt. Production Mac binaries are Developer
ID signed and notarized; keep Gatekeeper enabled. ZIP extraction may require
chmod 755 termkey before running a manually installed copy.
Launch the ZIP executable from Terminal, not by double-clicking it in Finder.
ZIPs and standalone tools cannot carry stapled notarization tickets; first
launch may require internet access to retrieve Apple's approval. Keep Gatekeeper
enabled and do not remove quarantine to bypass a failed check. This production
DMG and its PKG carry stapled tickets.

Upgrades and vaults
------------------

Close TermKey before upgrading. The installer replaces application files only;
it does not reset ~/.termkey or migrate the encrypted vault. Homebrew users should
use brew upgrade termkey. Manual ZIP users replace only the executable and retain
the included license/notices. Use command -v termkey to identify which installed
copy your shell launches when multiple installation methods coexist.

Move encrypted backups with termkey export and termkey import. Use a fresh private
destination vault or preserve an independent backup of an existing destination
before importing. Never overwrite an existing vault.ck during manual migration.
