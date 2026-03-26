#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 <binary-path> <asset-name> <volume-name> <output-dir>" >&2
  exit 1
fi

binary_path=$1
asset_name=$2
volume_name=$3
output_dir=$4

if [[ ! -f "$binary_path" ]]; then
  echo "binary not found: $binary_path" >&2
  exit 1
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
staging_dir=$(mktemp -d)
trap 'rm -rf "$staging_dir"' EXIT

mkdir -p "$output_dir"

cp "$binary_path" "$staging_dir/termkey"
cp "$script_dir/install.command" "$staging_dir/install.command"
cp "$script_dir/README.txt" "$staging_dir/README.txt"

chmod 755 "$staging_dir/termkey" "$staging_dir/install.command"
chmod 644 "$staging_dir/README.txt"

hdiutil create \
  -volname "$volume_name" \
  -srcfolder "$staging_dir" \
  -ov \
  -format UDZO \
  "$output_dir/${asset_name}.dmg" \
  >/dev/null
