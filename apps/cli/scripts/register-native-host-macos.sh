#!/usr/bin/env bash

set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  echo "usage: $0 <extension-id> [native-host-binary-path]" >&2
  exit 1
fi

extension_id=$1
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/../../.." && pwd)
binary_path=${2:-"$repo_root/target/debug/termkey-native-host"}
template_path="$repo_root/apps/cli/native-messaging/com.ryanonmars.termkey.template.json"
manifest_dir="$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"
manifest_path="$manifest_dir/com.ryanonmars.termkey.json"

if [[ ! -x "$binary_path" ]]; then
  echo "native host binary not found or not executable: $binary_path" >&2
  echo "build it first with: cargo build -p termkey --bin termkey-native-host" >&2
  exit 1
fi

mkdir -p "$manifest_dir"

sed \
  -e "s|__PATH__|$binary_path|g" \
  -e "s|__EXTENSION_ID__|$extension_id|g" \
  "$template_path" > "$manifest_path"

chmod 644 "$manifest_path"

echo "Installed Chrome native host manifest:"
echo "  $manifest_path"
echo
echo "Extension ID:"
echo "  $extension_id"
