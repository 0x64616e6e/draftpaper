#!/bin/sh
# make pixels (and alone): the parked windows' cells hatched (issue #46), headless (nothing on any
# display), on the T14's panel (1920x1200 at scale 1.5, 6.4 px/mm). A terminal parked: its cell hatched
# at 45° on whole pixels, clear where the shell letters it (the zone letter, the title), the other cells
# not; brought back, the cell clear again; a theme with hatch_spacing = 0, none.
# examples/sheetcheck.rs checks each screenshot. SHOT=FILE keeps the screenshot of the hatched cell.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
command -v foot > /dev/null || { echo "  (no foot: the hatching is not checked)"; exit 0; }
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-hatch.XXXXXX)
mkdir -p "$t/.config/draftpaper/themes" "$t/Pictures" "$t/run"
printf 'theme = "softglow"\nmargin = 6\n[apps]\nskip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]\n[session]\nrestore = false\napps = false\n' > "$t/.config/draftpaper/config.toml"
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" DRAFTPAPER_MM=6.4 "$comp" --headless 1920x1200 --scale 1.5 --command true > "$t/log" 2>&1 &
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
i=0
until run "$comp" msg outputs 2>/dev/null | grep -q draftpaper-title-block || [ $i -gt 300 ]; do sleep 0.1; i=$((i + 1)); done
act() { run "$comp" msg "$@" > /dev/null; sleep 0.6; }
shot() {
    rm -f "$t"/Pictures/*.png
    kill -USR1 $nest
    i=0
    until ls "$t"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$t"/Pictures/*.png | head -n 1)" "$1"
}

run foot -T "drafting log" sh > /dev/null 2>&1 &
pids="$pids $!"
i=0
until run "$comp" msg windows | grep -q '"focused": true' || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
act park
ok "parked in the first cell" "$(run "$comp" msg windows | tr -d ' \n' | grep -o '"cell":0')" '"cell":0'
sleep 1
shot "$t/parked.png"
[ -n "${SHOT:-}" ] && cp "$t/parked.png" "$SHOT"
"$check" "$t/parked.png" 1.5 6.4 6 hatch 0 1.5 | sed 's/^/  /' || fail=1
"$check" "$t/parked.png" 1.5 6.4 6 hatch 1 none | sed 's/^/  /' || fail=1
act park
shot "$t/back.png"
"$check" "$t/back.png" 1.5 6.4 6 hatch 0 none | sed 's/^/  /' || fail=1
# a theme without hatching
sed 's/^pattern_strength.*/&\nhatch_spacing    = 0/' "$repo/themes/softglow.toml" | sed 's/^label = .*/label = "Plain"/' > "$t/.config/draftpaper/themes/plain.toml"
run "$cli" theme plain > /dev/null
act park
sleep 1
shot "$t/plain.png"
"$check" "$t/plain.png" 1.5 6.4 6 hatch 0 none | sed 's/^/  /' || fail=1
exit $fail
