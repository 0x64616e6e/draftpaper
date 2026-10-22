#!/bin/sh
# make pixels (and alone): the key plan (issue #47), headless (nothing on any display), on two screens side
# by side: the T14's panel (1920x1200, 6.4 px/mm) and a 27" monitor (2560x1440, 4.3 px/mm), both at scale
# 1.5. Each sheet draws both screens in their arrangement in the bottom margin, itself filled, the active
# one heavier, a north mark left of it with the arm toward the other screen thickened (examples/sheetcheck.rs
# checks them pixel by pixel; the directions from the IPC); the focus moved to the second screen moves
# the heavier outline there; [title] key_plan = false takes the plans away. SHOT=DIR keeps the screenshots.
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
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-keyplan.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run"
config() { printf 'theme = "softglow"\nmargin = 6\n[title]\nkey_plan = %s\n[apps]\nskip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]\n[session]\nrestore = false\napps = false\n' "$1" > "$t/.config/draftpaper/config.toml"; }
config true
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" DRAFTPAPER_MM=6.4,4.3 "$comp" --headless 1920x1200,2560x1440 --scale 1.5 --command true > "$t/log" 2>&1 &
nest=$!
pids=$nest
i=0
until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
sock=$runtime/draftpaper-$wl.sock
run() {
    env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" DRAFTPAPER_BIN="$cli" \
        DRAFTPAPER_ROOT="$repo" DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" "$@"
}
run "$cli" apply > /dev/null
run qs -p "$repo/qml/shell.qml" -n > "$t/qs.log" 2>&1 &
pids="$pids $!"
outputs() { run "$comp" msg outputs | tr -d ' \n'; }
i=0
until [ "$(outputs | grep -o draftpaper-title-block | wc -l)" = 2 ] || [ $i -gt 300 ]; do sleep 0.1; i=$((i + 1)); done
sleep 2
ok "two screens, each with a key plan" "$(outputs | grep -o '"key_plan":{"mark"' | wc -l)" "2"
# the layout as the compositor has it: x,y,width,height of each screen, in its order. Read by name,
# not by where the fields fall in the JSON: the outputs carry whatever the compositor has to say, and
# a field added to them (`word`, say) is not a change to the layout this is measuring.
layout=$(run "$comp" msg outputs | jq -r '.[] | "\(.x),\(.y),\(.width),\(.height)"')
l1=$(echo "$layout" | sed -n 1p)
l2=$(echo "$layout" | sed -n 2p)
ok "side by side" "$l1 $l2" "0,0,1280,800 1280,0,1707,960"
# the north marks: each the arm toward the other screen thickened (the monitor's centre is a little lower,
# 3°: the nearest quarter is level)
ok "the north marks: right on the panel, left on the monitor" "$(outputs | grep -o '"towards":\[[^]]*\]' | tr '\n' ' ')" '"towards":["right"] "towards":["left"] '
shots() {
    rm -f "$t"/Pictures/*.png
    kill -USR1 $nest
    i=0
    until [ "$(ls "$t"/Pictures/*.png 2>/dev/null | wc -l)" = 2 ] || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$t"/Pictures/*-headless.png)" "$t/$1-1.png"
    mv "$(ls "$t"/Pictures/*-headless-2.png)" "$t/$1-2.png"
    [ -n "${SHOT:-}" ] && cp "$t/$1-1.png" "$t/$1-2.png" "$SHOT/"
}
shots first
"$check" "$t/first-1.png" 1.5 6.4 6 keyplan 0 0 "$l1" "$l2" | sed 's/^/  /' || fail=1
"$check" "$t/first-2.png" 1.5 4.3 6 keyplan 1 0 "$l1" "$l2" | sed 's/^/  /' || fail=1
# the focus onto the second screen (focus right, at the first one's edge): it is the active one now
run "$comp" msg focus right > /dev/null
sleep 0.6
shots moved
"$check" "$t/moved-1.png" 1.5 6.4 6 keyplan 0 1 "$l1" "$l2" | sed 's/^/  /' || fail=1
"$check" "$t/moved-2.png" 1.5 4.3 6 keyplan 1 1 "$l1" "$l2" | sed 's/^/  /' || fail=1
config false
sleep 2
ok "[title] key_plan = false: none" "$(outputs | grep -o '"key_plan":null' | wc -l)" "2"
exit $fail
