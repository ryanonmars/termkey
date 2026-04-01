TermKey for macOS
================

This disk image contains:

- Install TermKey.pkg: installer package for the TermKey app, CLI, and uninstaller

Recommended install
-------------------

1. Open Install TermKey.pkg.
2. Follow the macOS installer steps.
3. The installer places TermKey.app and Uninstall TermKey.app in /Applications.
4. It also places termkey at /usr/local/bin/termkey.
5. Open a new Terminal window and run termkey.

Uninstall
---------

To remove the installer-based app later, open Uninstall TermKey.app from /Applications.
It removes the app bundle, the CLI binaries, the Chrome integration files installed by
TermKey, and the installer receipt. Your ~/.termkey vault data is left untouched.

Manual install
--------------

You can also skip the installer and use the ZIP asset if you prefer a manual install.

The ZIP release is still available for users who want to place the binary manually.

Gatekeeper
----------

TermKey is currently distributed as an unsigned installer. If macOS blocks it, use Open Anyway
in System Settings > Privacy & Security.
