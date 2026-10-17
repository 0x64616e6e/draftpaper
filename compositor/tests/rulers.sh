#!/bin/sh
# make pixels (and alone): the rulers (issue #48, qml/Rulers.qml), headless (nothing on any display), on
# the T14's panel (1920x1200 at scale 1.5, 6.4 px/mm). Brought out with their zero at 150, 90 mm, level:
# the shell's surface up, a tick at every true millimetre on the horizontal one's edge
# (examples/sheetcheck.rs). The pointer, moved a third of a pixel at a time (pointer_by), keeps to whole
# millimetres from the zero, and moves (small moves add up); the same with the rulers turned to 30° and
# 120° (along each ruler); put away, it moves freely again. SHOT=FILE keeps the screenshot of the rulers.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-rulers.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run"
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
ipc() { run qs ipc -p "$repo/qml/shell.qml" call rulers "$@" 2> /dev/null; }
pointer() { run "$comp" msg pointer | tr -d ' \n{}"' | sed 's/x://; s/,y:/ /'; }

ipc set 150 90 0 90
i=0
until run "$comp" msg outputs | grep -q '"draftpaper-rulers"' || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
ok "out: the shell's rulers" "$(ipc state)" '{"zero":[150,90],"angles":[0,90],"locked":false,"snap":true}'
sleep 1
rm -f "$t"/Pictures/*.png
kill -USR1 $nest
i=0
until ls "$t"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
sleep 0.3
mv "$(ls "$t"/Pictures/*.png | head -n 1)" "$t/rulers.png"
[ -n "${SHOT:-}" ] && cp "$t/rulers.png" "$SHOT"
"$check" "$t/rulers.png" 1.5 6.4 6 rulers 150 90 | sed 's/^/  /' || fail=1

# the pointer, a third of a pixel at a time: each place whole millimetres from the zero along each ruler
# (H, V degrees), and somewhere else by the end. The zero: inside the frame's line (26 + 2/1.5 logical
# pixels in) and 150, 90 mm on, at 6.4/1.5 logical pixels a millimetre
walk() {
    h=$1 v=$2
    run "$comp" msg pointer 400 300 > /dev/null
    first=$(pointer)
    bad=0
    for n in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18; do
        run "$comp" msg pointer_by 0.33 0.21 > /dev/null
        pointer | awk -v h="$h" -v v="$v" '{
            mm = 6.4 / 1.5; zx = 26 + 2 / 1.5 + 150 * mm; zy = 26 + 2 / 1.5 + 90 * mm
            pi = atan2(0, -1); ux = cos(h * pi / 180); uy = -sin(h * pi / 180); wx = cos(v * pi / 180); wy = -sin(v * pi / 180)
            det = ux * wy - uy * wx; dx = $1 - zx; dy = $2 - zy
            a = (dx * wy - dy * wx) / det / mm; b = (ux * dy - uy * dx) / det / mm
            if (a - int(a < 0 ? a - 0.5 : a + 0.5) > 0.01 || int(a < 0 ? a - 0.5 : a + 0.5) - a > 0.01 || b - int(b < 0 ? b - 0.5 : b + 0.5) > 0.01 || int(b < 0 ? b - 0.5 : b + 0.5) - b > 0.01) exit 1
        }' || bad=$((bad + 1))
    done
    echo "$bad $( [ "$(pointer)" = "$first" ] && echo still || echo moved )"
}
ok "level: on whole millimetres every step, and moving" "$(walk 0 90)" "0 moved"
ipc set 150 90 30 120
sleep 0.5
ok "turned to 30° and 120°: on whole millimetres along each, and moving" "$(walk 30 120)" "0 moved"
# put away: free again
ipc toggle
sleep 0.5
run "$comp" msg pointer 400 300 > /dev/null
run "$comp" msg pointer_by 0.33 0.21 > /dev/null
ok "put away: the pointer free" "$(pointer)" "400.33 300.21"
ok "and the shell's rulers gone" "$(run "$comp" msg outputs | grep -c '"draftpaper-rulers"')" "0"
exit $fail
