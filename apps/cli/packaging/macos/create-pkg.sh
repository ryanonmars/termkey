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
termkey_app_bundle_dir="$payload_root/Applications/TermKey.app"
uninstall_app_bundle_dir="$payload_root/Applications/Uninstall TermKey.app"
volume_icon="$script_dir/termkey.icns"
plist_template="$script_dir/Info.plist.template"
uninstaller_plist_template="$script_dir/UninstallerInfo.plist.template"
launcher_source="$script_dir/Launcher.swift"
uninstaller_source="$script_dir/Uninstaller.swift"
swift_cache_dir="$staging_dir/swift-cache"

create_app_bundle() {
  local app_bundle_dir=$1
  local executable_name=$2
  local source_path=$3
  local plist_template_path=$4
  local bundle_cli_binary=$5

  local app_contents_dir="$app_bundle_dir/Contents"
  local app_macos_dir="$app_contents_dir/MacOS"
  local app_resources_dir="$app_contents_dir/Resources"
  local app_info_plist="$app_contents_dir/Info.plist"
  local app_pkg_info="$app_contents_dir/PkgInfo"
  local app_executable="$app_macos_dir/$executable_name"

  mkdir -p "$app_macos_dir" "$app_resources_dir"

  CLANG_MODULE_CACHE_PATH="$swift_cache_dir" \
  SWIFT_MODULE_CACHE_PATH="$swift_cache_dir" \
  swiftc -O -o "$app_executable" "$source_path"
  chmod 755 "$app_executable"

  if [[ "$bundle_cli_binary" == "yes" ]]; then
    local app_binary_dir="$app_resources_dir/bin"
    mkdir -p "$app_binary_dir"
    ditto --noextattr --noqtn "$binary_path" "$app_binary_dir/termkey"
    chmod 755 "$app_binary_dir/termkey"
  fi

  sed "s/__VERSION__/$version/g" "$plist_template_path" > "$app_info_plist"
  printf 'APPL????' > "$app_pkg_info"

  if [[ -f "$volume_icon" ]]; then
    ditto --noextattr --noqtn "$volume_icon" "$app_resources_dir/termkey.icns"
  fi
}

mkdir -p "$cli_install_dir" "$output_dir"
mkdir -p "$swift_cache_dir"

ditto --noextattr --noqtn "$binary_path" "$cli_install_dir/termkey"
chmod 755 "$cli_install_dir/termkey"
create_app_bundle "$termkey_app_bundle_dir" "TermKey" "$launcher_source" "$plist_template" yes
create_app_bundle "$uninstall_app_bundle_dir" "UninstallTermKey" "$uninstaller_source" "$uninstaller_plist_template" no

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
