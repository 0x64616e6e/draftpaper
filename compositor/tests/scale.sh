#!/bin/sh
# make pixels (and alone): the scale bar in the bottom margin (title.rs's scale_bar, issue #44), headless
# (nothing on any display), on the T14's panel (1920x1200 at scale 1.5, 6.4 px/mm): drawn by the
# compositor true to size, each centimetre's number lettered by the shell over its rule; gone when config.toml
# says [title] scale = false; and on a display that reports no size (no DRAFTPAPER_MM: the 160 PPI guess),
# no bar but NTS. examples/sheetcheck.rs checks each screenshot against the geometry. SHOT=FILE keeps the
# first screenshot.
set -u
# `draftpaper apply` sets the desktop's colour scheme and GTK theme with gsettings, and gsettings
# reaches the dconf service over the session bus -- past HOME, past XDG_CONFIG_HOME, into the
# settings of whoever is logged in. A test run would leave the machine wearing the theme this
# test picked, and every application that follows the system scheme would flap. In memory, then.
export GSETTINGS_BACKEND=memory
# and it restarts draftpaper-idle.service with systemctl, which reaches the session's own manager
# the same way: the live service would restart and its ExecStartPre would apply the logged-in
# user's theme over this test's. A stub on PATH keeps the run to itself (tests/stub/systemctl).
PATH="$(cd "$(dirname "$0")" && pwd)/stub:$PATH"
export PATH
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-scale.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run"
config() { printf 'theme = "softglow"\nmargin = 6\n[title]\nscale = %s\n[apps]\nskip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]\n[session]\nrestore = false\napps = false\n' "$1" > "$t/.config/draftpaper/config.toml"; }
config true
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

# a compositor ($@: its environment's DRAFTPAPER_MM, or nothing) with the shell, up and lettered
start() {
    : > "$t/log"
    env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" "$@" "$comp" --headless 1920x1200 --scale 1.5 --command true > "$t/log" 2>&1 &
    nest=$!
    pids="$pids $nest"
    i=0
    until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
    wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
    sock=$runtime/draftpaper-$wl.sock
    run "$cli" apply > /dev/null
    run qs -p "$repo/qml/shell.qml" -n > "$t/qs.log" 2>&1 &
    pids="$pids $!"
    i=0
    until DRAFTPAPER_SOCKET=$sock "$comp" msg outputs 2>/dev/null | grep -q draftpaper-title-block || [ $i -gt 300 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 2
}
run() {
    env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="${wl:-}" DRAFTPAPER_SOCKET="${sock:-}" DRAFTPAPER_BIN="$cli" \
        DRAFTPAPER_ROOT="$repo" DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" "$@"
}
shot() {
    rm -f "$t"/Pictures/*.png
    kill -USR1 "$nest"
    i=0
    until ls "$t"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$t"/Pictures/*.png | head -n 1)" "$1"
}
# the outputs as the shell gets them
bar() { DRAFTPAPER_SOCKET=$sock "$comp" msg outputs | tr -d ' \n'; }

start DRAFTPAPER_MM=6.4
ok "the compositor tells the shell where the bar is" "$(bar | grep -o '"measured":true')" '"measured":true'
shot "$t/bar.png"
[ -n "${SHOT:-}" ] && cp "$t/bar.png" "$SHOT"
"$check" "$t/bar.png" 1.5 6.4 6 scale bar | sed 's/^/  /' || fail=1
# switched off: config.toml is read again when it changes
config false
sleep 2
ok "switched off: nothing for the shell" "$(bar | grep -o '"scale_bar":null')" '"scale_bar":null'
shot "$t/off.png"
"$check" "$t/off.png" 1.5 6.4 6 scale none | sed 's/^/  /' || fail=1
kill $pids 2>/dev/null
pids=

# a display that reports no size: the guess, and nothing to scale
config true
start
ok "no size: not measured" "$(bar | grep -o '"measured":false')" '"measured":false'
shot "$t/nts.png"
"$check" "$t/nts.png" 1.5 6.2 6 scale nts | sed 's/^/  /' || fail=1
exit $fail
