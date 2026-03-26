TermKey for macOS
================

This disk image contains:

- termkey: the CLI binary
- install.command: guided installer for common PATH locations

Recommended install
-------------------

1. Right-click install.command and choose Open.
2. Confirm the macOS security prompts if they appear.
3. The installer will copy termkey into:
   - /opt/homebrew/bin on Apple Silicon when available
   - /usr/local/bin on Intel when available
   - ~/.local/bin as a fallback

Manual install
--------------

You can also copy termkey into any directory on your PATH and make sure it is executable.

Examples:

  chmod +x termkey
  sudo cp termkey /usr/local/bin/termkey

or on Apple Silicon systems using Homebrew:

  chmod +x termkey
  sudo cp termkey /opt/homebrew/bin/termkey

Gatekeeper
----------

TermKey is currently distributed as an unsigned binary. If macOS blocks it, use Open Anyway
in System Settings > Privacy & Security, or remove the quarantine flag manually:

  xattr -d com.apple.quarantine /path/to/termkey
