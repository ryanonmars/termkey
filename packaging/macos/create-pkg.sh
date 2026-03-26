#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 <binary-path> <version> <package-name> <output-dir>" >&2
  exit 1
fi

binary_path=$1
version=$2
package_name=$3
output_dir=$4

if [[ ! -f "$binary_path" ]]; then
  echo "binary not found: $binary_path" >&2
  exit 1
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
staging_dir=$(mktemp -d)
trap 'rm -rf "$staging_dir"' EXIT

payload_root="$staging_dir/root"
install_dir="$payload_root/usr/local/bin"

mkdir -p "$install_dir" "$output_dir"

ditto --noextattr --noqtn "$binary_path" "$install_dir/termkey"
chmod 755 "$install_dir/termkey"
xattr -cr "$payload_root" 2>/dev/null || true

COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 pkgbuild \
  --root "$payload_root" \
  --identifier "com.ryanonmars.termkey" \
  --version "$version" \
  --install-location "/" \
  --scripts "$script_dir/scripts" \
  "$output_dir/${package_name}.pkg" \
  >/dev/null
