#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 <package-path> <asset-name> <volume-name> <output-dir>" >&2
  exit 1
fi

package_path=$1
asset_name=$2
volume_name=$3
output_dir=$4

if [[ ! -f "$package_path" ]]; then
  echo "package not found: $package_path" >&2
  exit 1
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
staging_dir=$(mktemp -d)
trap 'rm -rf "$staging_dir"' EXIT

mkdir -p "$output_dir"

cp "$package_path" "$staging_dir/Install TermKey.pkg"
cp "$script_dir/README.txt" "$staging_dir/README.txt"

chmod 644 "$staging_dir/Install TermKey.pkg"
chmod 644 "$staging_dir/README.txt"
xattr -cr "$staging_dir" 2>/dev/null || true

hdiutil create \
  -volname "$volume_name" \
  -srcfolder "$staging_dir" \
  -ov \
  -format UDZO \
  "$output_dir/${asset_name}.dmg" \
  >/dev/null
