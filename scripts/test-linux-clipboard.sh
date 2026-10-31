#!/usr/bin/env sh
# Run synthetic tests only against private Xvfb and headless Sway servers.
# No packages are installed here. Invoke in an ephemeral Linux container/CI job
# as an unprivileged user with a precompiled native clipboard_linux test binary.
set -eu

if [ "${TERMKEY_TEST_CLIPBOARD_SANDBOX:-}" != "1" ]; then
  echo 'Set TERMKEY_TEST_CLIPBOARD_SANDBOX=1 in an isolated test container to run this helper.' >&2
  exit 1
fi
if [ "$(id -u)" = "0" ]; then
  echo 'Run clipboard sandbox tests as an unprivileged container user, not root.' >&2
  exit 1
fi
if [ "$#" != "1" ] || [ ! -x "$1" ]; then
  echo 'usage: test-linux-clipboard.sh <native clipboard_linux test executable>' >&2
  exit 1
fi
case "$1" in
  /*) test_binary=$1 ;;
  *) test_binary=$(pwd)/$1 ;;
esac
for command in Xvfb sway; do
  command -v "$command" >/dev/null || { echo "Required sandbox tool is missing: $command" >&2; exit 1; }
done

test_root=$(mktemp -d)
x11_pid=''
sway_pid=''
cleanup() {
  if [ -n "$sway_pid" ]; then kill "$sway_pid" 2>/dev/null || true; wait "$sway_pid" 2>/dev/null || true; fi
  if [ -n "$x11_pid" ]; then kill "$x11_pid" 2>/dev/null || true; wait "$x11_pid" 2>/dev/null || true; fi
  rm -rf -- "$test_root"
}
trap cleanup EXIT HUP INT TERM
mkdir "$test_root/headless" "$test_root/x11" "$test_root/wayland"
chmod 700 "$test_root" "$test_root/headless" "$test_root/x11" "$test_root/wayland"

# No display variables or compositor sockets from a user's session are inherited.
echo 'Verifying safe clipboard failure without a graphical session.'
env -u DISPLAY -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u TERMKEY_TEST_EXTERNAL_WRITER \
  XDG_RUNTIME_DIR="$test_root/headless" TERMKEY_TEST_HEADLESS_CLIPBOARD=1 \
  TERMKEY_TEST_GRAPHICAL_CLIPBOARD=0 \
  "$test_binary" --exact headless_clipboard_error_does_not_expose_value --test-threads=1 --nocapture

# -displayfd asks Xvfb to allocate a free server number rather than attaching to
# any preexisting desktop. TCP is disabled; this server lives in the sandbox.
Xvfb -screen 0 1280x720x24 -nolisten tcp -displayfd 3 \
  3>"$test_root/display-number" >"$test_root/xvfb.log" 2>&1 &
x11_pid=$!
tries=0
while [ ! -s "$test_root/display-number" ]; do
  if ! kill -0 "$x11_pid" 2>/dev/null || [ "$tries" -ge 100 ]; then
    cat "$test_root/xvfb.log" >&2; echo 'Private Xvfb server failed to start.' >&2; exit 1
  fi
  tries=$((tries + 1)); sleep 0.1
done
read -r display_number < "$test_root/display-number"
case "$display_number" in ''|*[!0-9]*) echo 'Invalid private Xvfb display number.' >&2; exit 1 ;; esac
echo 'Verifying native X11 clipboard retention, timed clearing, and replacement ownership.'
env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u TERMKEY_TEST_EXTERNAL_WRITER \
  DISPLAY=":$display_number" XDG_RUNTIME_DIR="$test_root/x11" \
  TERMKEY_TEST_GRAPHICAL_CLIPBOARD=1 TERMKEY_TEST_HEADLESS_CLIPBOARD=0 \
  "$test_binary" --test-threads=1 --nocapture

cat > "$test_root/sway.conf" <<'CONFIG'
xwayland disable
seat seat0 fallback true
output * mode 1280x720
CONFIG
env -u DISPLAY -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u TERMKEY_TEST_EXTERNAL_WRITER \
  XDG_RUNTIME_DIR="$test_root/wayland" WLR_BACKENDS=headless WLR_RENDERER=pixman \
  WLR_HEADLESS_OUTPUTS=1 WLR_LIBINPUT_NO_DEVICES=1 \
  sway --unsupported-gpu --config "$test_root/sway.conf" \
  >"$test_root/sway.log" 2>&1 &
sway_pid=$!
tries=0
wayland_socket=''
while [ -z "$wayland_socket" ]; do
  for candidate in "$test_root/wayland"/wayland-*; do
    if [ -S "$candidate" ]; then wayland_socket=${candidate##*/}; break; fi
  done
  if [ -n "$wayland_socket" ]; then break; fi
  if ! kill -0 "$sway_pid" 2>/dev/null || [ "$tries" -ge 100 ]; then
    cat "$test_root/sway.log" >&2; echo 'Private headless Sway server failed to start.' >&2; exit 1
  fi
  tries=$((tries + 1)); sleep 0.1
done
echo 'Verifying native Wayland data-control clipboard retention, timed clearing, and replacement ownership.'
env -u DISPLAY -u WAYLAND_SOCKET -u TERMKEY_TEST_EXTERNAL_WRITER \
  WAYLAND_DISPLAY="$wayland_socket" XDG_RUNTIME_DIR="$test_root/wayland" \
  TERMKEY_TEST_GRAPHICAL_CLIPBOARD=1 TERMKEY_TEST_HEADLESS_CLIPBOARD=0 \
  "$test_binary" --test-threads=1 --nocapture

echo 'Isolated native Linux clipboard checks passed (X11, Sway/wlroots Wayland, and headless).'
