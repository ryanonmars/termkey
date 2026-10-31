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
contains termkey, LICENSE, and README.txt. Production Mac binaries are Developer
ID signed and notarized; keep Gatekeeper enabled. ZIP extraction may require
chmod 755 termkey before running a manually installed copy.
