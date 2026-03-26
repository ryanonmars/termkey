#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
BINARY_PATH="$SCRIPT_DIR/termkey"

pause_if_needed() {
  if [[ -t 0 ]]; then
    printf '\nPress Enter to close this window...'
    read -r _
  fi
}

choose_install_dir() {
  if [[ -n "${TERMKEY_INSTALL_DIR:-}" ]]; then
    printf '%s\n' "$TERMKEY_INSTALL_DIR"
    return
  fi

  if [[ "$(uname -m)" == "arm64" ]]; then
    if [[ -d "/opt/homebrew/bin" ]] || [[ ":$PATH:" == *":/opt/homebrew/bin:"* ]]; then
      printf '%s\n' "/opt/homebrew/bin"
      return
    fi
  fi

  if [[ -d "/usr/local/bin" ]] || [[ ":$PATH:" == *":/usr/local/bin:"* ]]; then
    printf '%s\n' "/usr/local/bin"
    return
  fi

  if [[ -d "$HOME/.local/bin" ]] || [[ ":$PATH:" == *":$HOME/.local/bin:"* ]]; then
    printf '%s\n' "$HOME/.local/bin"
    return
  fi

  printf '%s\n' "$HOME/.local/bin"
}

if [[ ! -x "$BINARY_PATH" ]]; then
  echo "Could not find the termkey binary next to this installer."
  pause_if_needed
  exit 1
fi

INSTALL_DIR=$(choose_install_dir)
DEST_PATH="$INSTALL_DIR/termkey"

echo "Installing TermKey to $DEST_PATH"

if [[ "$INSTALL_DIR" == "$HOME/.local/bin" ]] && [[ ! -d "$INSTALL_DIR" ]]; then
  mkdir -p "$INSTALL_DIR"
fi

if [[ -w "$INSTALL_DIR" ]]; then
  install -m 755 "$BINARY_PATH" "$DEST_PATH"
else
  sudo mkdir -p "$INSTALL_DIR"
  sudo install -m 755 "$BINARY_PATH" "$DEST_PATH"
fi

xattr -d com.apple.quarantine "$DEST_PATH" 2>/dev/null || true

echo
echo "Installed: $DEST_PATH"
echo
echo "Run 'termkey' in a new Terminal window."
echo "If the command is not found, add $INSTALL_DIR to your PATH."

pause_if_needed
