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
cli_install_dir="$payload_root/usr/local/bin"
app_bundle_dir="$payload_root/Applications/TermKey.app"
app_contents_dir="$app_bundle_dir/Contents"
app_macos_dir="$app_contents_dir/MacOS"
app_binary_dir="$app_bundle_dir/Contents/Resources/bin"
app_resources_dir="$app_contents_dir/Resources"
app_info_plist="$app_contents_dir/Info.plist"
app_launcher="$app_macos_dir/TermKey"
app_pkg_info="$app_contents_dir/PkgInfo"
volume_icon="$script_dir/termkey.icns"
plist_template="$script_dir/Info.plist.template"
launcher_source="$script_dir/Launcher.swift"
swift_cache_dir="$staging_dir/swift-cache"

mkdir -p "$cli_install_dir" "$app_macos_dir" "$app_binary_dir" "$output_dir"
mkdir -p "$swift_cache_dir"

ditto --noextattr --noqtn "$binary_path" "$cli_install_dir/termkey"
CLANG_MODULE_CACHE_PATH="$swift_cache_dir" \
SWIFT_MODULE_CACHE_PATH="$swift_cache_dir" \
swiftc -O -o "$app_launcher" "$launcher_source"
ditto --noextattr --noqtn "$binary_path" "$app_binary_dir/termkey"
chmod 755 "$cli_install_dir/termkey" "$app_launcher" "$app_binary_dir/termkey"

sed "s/__VERSION__/$version/g" "$plist_template" > "$app_info_plist"
printf 'APPL????' > "$app_pkg_info"

if [[ -f "$volume_icon" ]]; then
  ditto --noextattr --noqtn "$volume_icon" "$app_resources_dir/termkey.icns"
fi

xattr -cr "$payload_root" 2>/dev/null || true
find "$payload_root" -name '._*' -delete 2>/dev/null || true
dot_clean -m "$payload_root" 2>/dev/null || true

COPYFILE_DISABLE=1 COPY_EXTENDED_ATTRIBUTES_DISABLE=1 pkgbuild \
  --root "$payload_root" \
  --identifier "com.ryanonmars.termkey" \
  --version "$version" \
  --install-location "/" \
  --scripts "$script_dir/scripts" \
  "$output_dir/${package_name}.pkg" \
  >/dev/null
