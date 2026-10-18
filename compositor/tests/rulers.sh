#!/bin/sh
# make pixels (and alone): the rulers (issue #48, qml/Rulers.qml), headless (nothing on any display), on
# the T14's panel (1920x1200 at scale 1.5, 6.4 px/mm). Brought out with their zero at 150, 90 mm, level:
# the shell's surface up, a tick at every true millimetre on the horizontal one's edge
# (examples/sheetcheck.rs). The pointer, moved a third of a pixel at a time (pointer_by), keeps to whole
# millimetres from the zero, and moves (small moves add up); the same with the rulers turned to 30° and
# 120° (along each ruler); put away, it moves freely again. The panel by the zero hides (its tab stays)
# and shows. SHOT=FILE keeps the screenshot of the rulers.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-rulers.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run" "$t/bin"
# idle's service is the user's: left alone when config set applies
printf '#!/bin/sh\nexit 0\n' > "$t/bin/systemctl"
chmod +x "$t/bin/systemctl"
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
    env -u DISPLAY PATH="$t/bin:$PATH" HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" DRAFTPAPER_BIN="$cli" \
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
ok "out: the shell's rulers" "$(ipc state)" '{"crossing":[150,90],"zeros":[0,0],"angles":[0,90],"locked":false,"snap":true}'
sleep 1
rm -f "$t"/Pictures/*.png
kill -USR1 $nest
i=0
until ls "$t"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
sleep 0.3
mv "$(ls "$t"/Pictures/*.png | head -n 1)" "$t/rulers.png"
[ -n "${SHOT:-}" ] && cp "$t/rulers.png" "$SHOT"
"$check" "$t/rulers.png" 1.5 6.4 6 rulers 150 90 | sed 's/^/  /' || fail=1
# the panel by the zero (right of it and below, 50 mm wide): dark where it is; hidden (p), the rulers and
# their angles show there again, and its tab stays by the zero
shot() {
    rm -f "$t"/Pictures/*.png
    kill -USR1 $nest
    i=0
    until ls "$t"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$t"/Pictures/*.png | head -n 1)" "$1"
}
# its middle: 13 mm right of the zero and 25 more; 13 mm below it and 6 more (physical pixels)
px=$(awk 'BEGIN { printf "%d", 999 + 38 * 6.4 }'); py=$(awk 'BEGIN { printf "%d", 615 + 19 * 6.4 }')
ok "the panel there" "$([ "$("$check" "$t/rulers.png" 1.5 6.4 6 lum $px $py)" -lt 90 ] && echo dark)" "dark"
ipc panel false
sleep 0.5
shot "$t/hidden.png"
ok "hidden: not there" "$([ "$("$check" "$t/hidden.png" 1.5 6.4 6 lum $px $py)" -gt 120 ] && echo light)" "light"
# (a corner of it, clear of its sign)
tx=$(awk 'BEGIN { printf "%d", 999 + 13.6 * 6.4 }'); ty=$(awk 'BEGIN { printf "%d", 615 + 18.4 * 6.4 }')
ok "and its tab by the zero" "$([ "$("$check" "$t/hidden.png" 1.5 6.4 6 lum $tx $ty)" -lt 90 ] && echo dark)" "dark"
ipc panel true
# each ruler's own zero, off the crossing: the horizontal one's 20 mm right of it, a red line across the ruler
# there (a row clear of the ticks and numbers, 3 mm under the middle)
zx=$(awk 'BEGIN { printf "%d", 999 + 20 * 6.4 }'); zr=$(awk 'BEGIN { printf "%d", 615 + 3 * 6.4 }')
darkest() { for x in $((zx - 2)) $((zx - 1)) $zx $((zx + 1)) $((zx + 2)); do "$check" "$1" 1.5 6.4 6 lum $x $zr; done | sort -n | head -n 1; }
ok "before: no line 20 mm on" "$([ "$(darkest "$t/rulers.png")" -gt 120 ] && echo clear)" "clear"
ipc zeros 20 -10
sleep 0.5
ok "the zeros set" "$(ipc state | grep -o '"zeros":\[[^]]*\]')" '"zeros":[20,-10]'
shot "$t/zeros.png"
ok "the horizontal ruler's zero line 20 mm on" "$([ "$(darkest "$t/zeros.png")" -lt 110 ] && echo drawn)" "drawn"
"$check" "$t/zeros.png" 1.5 6.4 6 rulers 150 90 20 | sed 's/^/  /' || fail=1
ipc zeros 0 0

# the pointer, a third of a pixel at a time: each place whole millimetres from the zero along each ruler
# (H, V degrees), and somewhere else by the end. The zero: inside the frame's line (26 + 2/1.5 logical
# pixels in) and 150, 90 mm on, at 6.4/1.5 logical pixels a millimetre
walk() {
    h=$1 v=$2 s=${3:-1}
    run "$comp" msg pointer 400 300 > /dev/null
    first=$(pointer)
    bad=0
    for n in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18; do
        run "$comp" msg pointer_by 0.33 0.21 > /dev/null
        pointer | awk -v h="$h" -v v="$v" -v s="$s" '{
            mm = 6.4 / 1.5; zx = (39 + int(150 * 6.4 + 0.5)) / 1.5; zy = (39 + int(90 * 6.4 + 0.5)) / 1.5
            pi = atan2(0, -1); ux = cos(h * pi / 180); uy = -sin(h * pi / 180); wx = cos(v * pi / 180); wy = -sin(v * pi / 180)
            det = ux * wy - uy * wx; dx = $1 - zx; dy = $2 - zy
            a = (dx * wy - dy * wx) / det / mm / s; b = (ux * dy - uy * dx) / det / mm / s
            if (a - int(a < 0 ? a - 0.5 : a + 0.5) > 0.01 || int(a < 0 ? a - 0.5 : a + 0.5) - a > 0.01 || b - int(b < 0 ? b - 0.5 : b + 0.5) > 0.01 || int(b < 0 ? b - 0.5 : b + 0.5) - b > 0.01) exit 1
        }' || bad=$((bad + 1))
    done
    echo "$bad $( [ "$(pointer)" = "$first" ] && echo still || echo moved )"
}
ok "level: on whole millimetres every step, and moving" "$(walk 0 90)" "0 moved"
ipc set 150 90 30 120
sleep 0.5
ok "turned to 30° and 120°: on whole millimetres along each, and moving" "$(walk 30 120)" "0 moved"
# config.toml's [rulers] step = 2 (the settings' Rulers page): the shell tells the compositor, the pointer on
# even millimetres
ipc set 150 90 0 90
run "$cli" config set rulers.step=2 > /dev/null
sleep 1
ok "[rulers] step = 2: on even millimetres every step, and moving" "$(walk 0 90 2)" "0 moved"
# put away: free again
ipc toggle
sleep 0.5
run "$comp" msg pointer 400 300 > /dev/null
run "$comp" msg pointer_by 0.33 0.21 > /dev/null
ok "put away: the pointer free" "$(pointer)" "400.33 300.21"
ok "and the shell's rulers gone" "$(run "$comp" msg outputs | grep -c '"draftpaper-rulers"')" "0"
exit $fail
