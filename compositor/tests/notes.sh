#!/bin/sh
# make pixels (and alone): the notes on the sheet (issue #49: qml/MachineSchedule.qml, qml/LoadGraph.qml),
# headless (nothing on any display), on the T14's panel (1920x1200 at 1.5, 6.4 px/mm). On the empty
# workspace both are up (their layer surfaces), the schedule's outline 7 mm in from the frame's corner on
# whole pixels (ink there, paper a millimetre inside), the figure below it; a window opened takes both
# away, closed brings them back; [sheet] machine = false and load = false take them away.
# SHOT=FILE keeps a screenshot of the empty workspace.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-notes.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run" "$t/bin"
cfg=$t/.config/draftpaper/config.toml
cat > "$cfg" <<'TOML'
theme = "softglow"
margin = 6
[apps]
skip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]
[session]
restore = false
apps = false
TOML
cp "$cfg" "$t/before.toml"
# idle's service is the user's: left alone
printf '#!/bin/sh\nexit 0\n' > "$t/bin/systemctl"
chmod +x "$t/bin/systemctl"
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
layers() { run "$comp" msg outputs | jq -r '.[0].layers[]' | grep -c "draftpaper-machine\|draftpaper-load"; }
waitcfg() { i=0; until grep -q "$1" "$cfg" || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done; grep -c "$1" "$cfg"; }
waitlayers() { i=0; until [ "$(layers)" = "$1" ] || [ $i -gt 80 ]; do sleep 0.1; i=$((i + 1)); done; layers; }
shot() {
    rm -f "$t"/Pictures/*.png
    kill -USR1 $nest
    i=0
    until ls "$t"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$t"/Pictures/*.png | head -n 1)" "$1"
}
# the first sample of the load comes 5 s in
ok "an empty workspace: the schedule and the load up" "$(waitlayers 2)" "2"
sleep 1
shot "$t/notes.png"
[ -n "${SHOT:-}" ] && { sleep "${SHOT_AFTER:-0}"; shot "$t/notes.png"; cp "$t/notes.png" "$SHOT"; }
# the frame's corner: the margin (6 mm) and its 2-pixel line; the schedule's outline 7 mm further in, a
# column of ink; a millimetre inside it, the paper
fx=$(awk 'BEGIN { printf "%d", 6 * 6.4 + 2 }'); edge=$(awk -v f="$fx" 'BEGIN { printf "%d", f + 7 * 6.4 }')
lum() { "$check" "$t/notes.png" 1.5 6.4 6 lum "$1" "$2"; }
darkest() { for x in $(($1 - 3)) $(($1 - 2)) $(($1 - 1)) $1 $(($1 + 1)) $(($1 + 2)) $(($1 + 3)); do lum $x "$2"; done | sort -n | head -n 1; }
row=$(awk -v f="$fx" 'BEGIN { printf "%d", f + 20 * 6.4 }')
ok "the schedule's outline, 7 mm in" "$([ "$(darkest "$edge" "$row")" -lt 110 ] && echo ink)" "ink"
ok "the paper a millimetre inside" "$([ "$(lum $((edge + 7)) "$row")" -gt 130 ] && echo paper)" "paper"
# exact to the millimetre: the outline's left edge where frame + 7 mm falls (its nearest physical pixel),
# the paper two pixels outside it; in zone B2, where B2's corner rounded up to a millimetre, + 7 mm, falls
o() { run "$comp" msg outputs | jq -r ".[0] | $1"; }
mmL=$(o .px_per_mm); fxL=$(o .frame.x); fyL=$(o .frame.y)
colAt() { awk -v f="$fxL" -v m="$mmL" -v k="$1" 'BEGIN { v = (f + k * m) * 1.5; printf "%d", int(v + 0.5) }'; }
rowAt() { awk -v f="$fyL" -v m="$mmL" -v k="$1" 'BEGIN { v = (f + k * m) * 1.5; printf "%d", int(v + 0.5) }'; }
edgeAt() { lum "$1" "$2"; }
exact() {   # $1 mm across, $2 mm down: the 2-pixel outline on that pixel and the next, the paper on the one before
    x=$(colAt "$1"); y=$(rowAt "$(($2 + 20))")
    [ "$(lum "$x" "$y")" -lt 110 ] && [ "$(lum $((x + 1)) "$y")" -lt 110 ] && [ "$(lum $((x - 1)) "$y")" -gt 130 ] && echo exact || echo "off: $(lum $((x - 1)) "$y") $(lum "$x" "$y") $(lum $((x + 1)) "$y") at $x"
}
ok "the outline on its millimetre: 7 mm from the frame's corner" "$(exact 7 7)" "exact"
# the load's heavy lines on the sheet's own 10 mm: 50 mm from the frame's corner (a column of the dots too),
# halfway up the graph (the bottom left, 72 mm tall, 7 mm off the frame's bottom): darker than the 49, 51 mm lines
fh=$(awk -v h="$(o .frame.height)" -v m="$mmL" 'BEGIN { print int(h / m + 1e-6) }')
x50=$(colAt 50); ymid=$(rowAt $((fh - 7 - 36)))
l50=$(lum "$x50" "$ymid"); l49=$(lum "$(colAt 49)" "$ymid"); l51=$(lum "$(colAt 51)" "$ymid")
ok "the load's heavy line on the sheet's 50 mm, the 49 and 51 mm lines light" "$([ "$l50" -lt $((l49 - 6)) ] && [ "$l50" -lt $((l51 - 6)) ] && echo on || echo "off: $l49 $l50 $l51")" "on"
zx=$(awk -v w="$(o .zones.width)" -v m="$mmL" 'BEGIN { v = w / m; c = int(v); if (c < v - 1e-6) c++; print c + 7 }')
zy=$(awk -v h="$(o .zones.height)" -v m="$mmL" 'BEGIN { v = h / m; c = int(v); if (c < v - 1e-6) c++; print c + 7 }')
run "$cli" config set 'sheet.machine_at=B2' > /dev/null
sleep 1.5
shot "$t/b2.png"
cp "$t/b2.png" "$t/notes.png"
ok "zone B2: 7 mm into it, ${zx} mm, ${zy} mm from the frame's corner" "$(exact "$zx" "$zy")" "exact"
# dragged by the title: 10 mm right and 5 mm down, then let go; config.toml keeps the place
gx=$(awk -v f="$fxL" -v m="$mmL" -v k="$zx" 'BEGIN { print f + (k + 20) * m }'); gy=$(awk -v f="$fyL" -v m="$mmL" -v k="$zy" 'BEGIN { print f + (k + 3) * m }')
run "$comp" msg pointer "$gx" "$gy" > /dev/null; sleep 0.2
run "$comp" msg button left down > /dev/null; sleep 0.2
run "$comp" msg pointer "$(awk -v g="$gx" -v m="$mmL" 'BEGIN { print g + 5 * m }')" "$gy" > /dev/null; sleep 0.1
run "$comp" msg pointer "$(awk -v g="$gx" -v m="$mmL" 'BEGIN { print g + 10 * m }')" "$(awk -v g="$gy" -v m="$mmL" 'BEGIN { print g + 5 * m }')" > /dev/null; sleep 0.2
run "$comp" msg button left up > /dev/null
ok "dragged 10 mm right, 5 mm down: kept in millimetres" "$(waitcfg "^machine_at = \\[$((zx + 10)), $((zy + 5))\\]")" "1"
sleep 1.5
shot "$t/dragged.png"
cp "$t/dragged.png" "$t/notes.png"
ok "and drawn there, on its millimetre" "$(exact $((zx + 10)) $((zy + 5)))" "exact"
ok "the load still up, moved out of its way" "$(waitlayers 2)" "2"
# over a wallpaper: still both up (the load's lines on a halo of the paper); SHOT_PIC=FILE keeps it
magick -size 1920x1200 gradient:"#8C9AA8"-"#4A463E" "$t/pic.png" 2> /dev/null || convert -size 1920x1200 gradient:"#8C9AA8"-"#4A463E" "$t/pic.png"
run "$cli" config set "wallpaper=$t/pic.png" > /dev/null
sleep 2
ok "over a wallpaper: both still up" "$(waitlayers 2)" "2"
[ -n "${SHOT_PIC:-}" ] && { sleep "${SHOT_AFTER:-0}"; shot "$t/pic-shot.png"; cp "$t/pic-shot.png" "$SHOT_PIC"; }
run "$cli" config set "wallpaper=" > /dev/null
command -v foot > /dev/null && {
    run foot sh > /dev/null 2>&1 &
    pids="$pids $!"
    ok "a window opened: both away" "$(waitlayers 0)" "0"
    kill "$(run "$comp" msg windows | jq -r ".[0].pid")" 2>/dev/null
    ok "closed: both back" "$(waitlayers 2)" "2"
}
run "$cli" config set sheet.machine=false sheet.load=false > /dev/null
ok "[sheet] machine = false, load = false: both away" "$(waitlayers 0)" "0"
ok "no QML warning from the notes" "$(grep -c 'MachineSchedule.qml\|LoadGraph.qml' "$t/qs.log")" "0"
[ "$fail" = 0 ] && echo "notes: ok" || { grep -a 'MachineSchedule.qml\|LoadGraph.qml' "$t/qs.log" | head -5; echo "notes: FAILED"; }
exit $fail
