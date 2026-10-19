#!/bin/sh
# make pixels (and alone): the notes on the sheet (issue #49: qml/Schedule.qml, qml/MachineSchedule.qml,
# qml/ProcessSchedule.qml, qml/LoadGraph.qml), headless (nothing on any display), on the T14's panel
# (1920x1200 at 1.5, 6.4 px/mm). Nothing until asked. The schedules take turns (Mod+i: next): the
# equipment, in the title block's column from the top, its rules on their pixels; the processes; neither.
# A reading's graph comes and goes with its cell (load, temperature, power, network; the disks from the
# equipment's STORAGE row), its heavy line on the sheet's 50 mm. Zone B2 and a drag move
# the schedule, a whole millimetre at a time, kept in config.toml; the graph level with the title block,
# across to it. Over a wallpaper still there. SHOT=FILE keeps a
# screenshot of the equipment and the load (SHOT_PROC, SHOT_NET, SHOT_DISK: of those graphs; KEPT=FILE: from
# that store).
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
# the readings over time (draftpaper-metrics.service), a sample a second; KEPT=FILE starts it from that store
[ -n "${KEPT:-}" ] && { mkdir -p "$t/run"; cp "$KEPT" "$t/run/metrics.json"; }
DRAFTPAPER_RUNTIME="$t/run" XDG_STATE_HOME="$t/.local/state" DRAFTPAPER_METRICS_EVERY=1 "$cli" metrics keep &
pids="$pids $!"
i=0
until run "$comp" msg outputs 2>/dev/null | grep -q draftpaper-title-block || [ $i -gt 300 ]; do sleep 0.1; i=$((i + 1)); done
layers() { run "$comp" msg outputs | jq -r ".[0].layers[]" | grep -c "draftpaper-$1"; }
waitcfg() { i=0; until grep -q "$1" "$cfg" || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done; grep -c "$1" "$cfg"; }
waitlayers() { i=0; until [ "$(layers "$1")" = "$2" ] || [ $i -gt 80 ]; do sleep 0.1; i=$((i + 1)); done; layers "$1"; }
shot() {
    rm -f "$t"/Pictures/*.png
    kill -USR1 $nest
    i=0
    until ls "$t"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$t"/Pictures/*.png | head -n 1)" "$1"
}
sched() { run qs ipc -p "$repo/qml/shell.qml" call schedules "$@" 2> /dev/null; }
graph() { run qs ipc -p "$repo/qml/shell.qml" call graph "$@" 2> /dev/null; }
o() { run "$comp" msg outputs | jq -r ".[0] | $1"; }
mmL=$(o .px_per_mm); fxL=$(o .frame.x); fyL=$(o .frame.y)
colAt() { awk -v f="$fxL" -v m="$mmL" -v k="$1" 'BEGIN { v = (f + k * m) * 1.5; printf "%d", int(v + 0.5) }'; }
rowAt() { awk -v f="$fyL" -v m="$mmL" -v k="$1" 'BEGIN { v = (f + k * m) * 1.5; printf "%d", int(v + 0.5) }'; }
lum() { "$check" "$t/notes.png" 1.5 6.4 6 lum "$1" "$2"; }
# a 1-pixel rule down the sheet $1 mm from the frame's corner, at $2 mm down: darker than either side
rule() {
    x=$(colAt "$1"); y=$(rowAt "$2")
    l=$(lum "$x" "$y"); a=$(lum $((x - 1)) "$y"); b=$(lum $((x + 1)) "$y")
    [ "$l" -lt $((a - 10)) ] && [ "$l" -lt $((b - 10)) ] && echo exact || echo "off: $a $l $b at $x"
}

sleep 1
ok "nothing until asked" "$(layers schedule)$(layers load)" "00"
sched next
ok "Mod+i: the equipment" "$(waitlayers schedule 1)/$(sched state)" "1/equipment"
sleep 1
shot "$t/notes.png"
# in the title block's column, from the top: its first column's rule 9 mm from the title block's left rule,
# the heading's heavy rule 7 mm under the frame's top line, each on its pixel
tx=$(o .title.rect.x)
tcol=$(awk -v t="$tx" -v m="$mmL" 'BEGIN { x0 = int(t * 1.5 + 0.5) / 1.5; printf "%d", int((x0 + 9 * m) * 1.5 + 0.5) }')
yrow=$(rowAt 30)
l=$(lum "$tcol" "$yrow"); a=$(lum $((tcol - 1)) "$yrow"); b=$(lum $((tcol + 1)) "$yrow")
ok "in the title block's column: its first column's rule 9 mm in, on its pixel" "$([ "$l" -lt $((a - 10)) ] && [ "$l" -lt $((b - 10)) ] && echo exact || echo "off: $a $l $b at $tcol")" "exact"
hy=$(rowAt 13); hx=$(awk -v t="$tx" 'BEGIN { printf "%d", t * 1.5 + 100 }')
ok "its heading's rule 13 mm under the frame's top (6 mm in, 7 of heading)" "$([ "$(lum "$hx" "$hy")" -lt 110 ] && [ "$(lum "$hx" $((hy - 2)))" -gt 130 ] && echo exact || echo "off: $(lum "$hx" $((hy - 2))) $(lum "$hx" "$hy")")" "exact"
sched next
sleep 0.5
ok "again: the processes, in its place" "$(waitlayers schedule 1)/$(sched state)" "1/processes"
sched next
ok "again: the revisions (no repository focused here: nothing to show)" "$(waitlayers schedule 0)/$(sched state)" "0/revisions"
sched next
ok "again: none" "$(waitlayers schedule 0)/$(sched state)" "0/"
# a reading's graph: its cell shows it, again puts it away; from the keeper's first samples
i=0; until [ -s "$t/run/metrics.json" ] || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done; sleep 1
ok "the readings kept: every period" "$(run "$cli" metrics | jq "[.tiers[] | select(.slots | length > 0)] | length")" "6"
graph toggle load
ok "the load's cell: its graph" "$(waitlayers load 1)" "1"
sched equipment
sleep 1.5
shot "$t/notes.png"
[ -n "${SHOT:-}" ] && { sleep "${SHOT_AFTER:-0}"; shot "$t/notes.png"; cp "$t/notes.png" "$SHOT"; }
fh=$(awk -v h="$(o .frame.height)" -v m="$mmL" 'BEGIN { print int(h / m + 1e-6) }')
# the graph level with the title block: from its top line (the first whole millimetre) to the frame's
zt=$(awk -v y="$(o .title.rect.y)" -v f="$fyL" -v m="$mmL" 'BEGIN { v = (y - f) / m; c = int(v); if (c < v - 1e-6) c++; print c }')
ymid=$(rowAt $(( (zt + fh) / 2 ))); x50=$(colAt 50)
l50=$(lum "$x50" "$ymid"); l49=$(lum "$(colAt 49)" "$ymid"); l51=$(lum "$(colAt 51)" "$ymid")
ok "the graph's heavy line on the sheet's 50 mm, the 49 and 51 mm lines light" "$([ "$l50" -lt $((l49 - 6)) ] && [ "$l50" -lt $((l51 - 6)) ] && echo on || echo "off: $l49 $l50 $l51")" "on"
yabove=$(rowAt $((zt - 1))); a50=$(lum "$x50" "$yabove"); a49=$(lum "$(colAt 49)" "$yabove")
ok "and nothing of it above the title block's top line" "$([ "$a50" -ge $((a49 - 3)) ] && echo within || echo "a line: $a49 $a50")" "within"
graph toggle load
ok "again: away" "$(waitlayers load 0)" "0"
graph toggle temperature
ok "the temperature's cell: its graph" "$(waitlayers load 1)/$(graph state)" "1/temperature"
graph toggle power
ok "the power's: switched to it (on mains here)" "$(waitlayers load 1)/$(graph state)" "1/power"
graph toggle network
ok "the network: its graph (down and up)" "$(waitlayers load 1)/$(graph state)" "1/network"
graph toggle disk
ok "the disks: their graph (read and written)" "$(waitlayers load 1)/$(graph state)" "1/disk"
# a click on the title block's NETWORK cell: the network's graph; again: away
click() { run "$comp" msg pointer "$1" "$2" > /dev/null; sleep 0.15; run "$comp" msg button left down > /dev/null; sleep 0.1; run "$comp" msg button left up > /dev/null; sleep 0.3; }
cx=$(awk -v x="$(o .title.rect.x)" -v m="$mmL" 'BEGIN { print x + 17 * m }'); cy=$(awk -v y="$(o .title.rect.y)" -v m="$mmL" 'BEGIN { print y + 30 * m }')
click "$cx" "$cy"
ok "a click on NETWORK: its graph" "$(graph state)" "network"
click "$cx" "$cy"
ok "again: away" "$(graph state)" ""
# a click on the equipment schedule's STORAGE row (wherever this machine's rows put it): the disks' graph
sched equipment > /dev/null
sx=$(awk -v x="$(o .title.rect.x)" -v m="$mmL" 'BEGIN { print x + 60 * m }')
k=20; top=$(awk -v y="$(o .title.rect.y)" -v f="$fyL" -v m="$mmL" 'BEGIN { print int((y - f) / m) - 2 }')
while [ "$(graph state)" != "disk" ] && [ $k -lt "$top" ]; do click "$sx" "$(awk -v f="$fyL" -v m="$mmL" -v k="$k" 'BEGIN { print f + k * m }')"; k=$((k + 3)); done
ok "a click on STORAGE: the disks' graph" "$(graph state)" "disk"
graph toggle power > /dev/null
# SHOT_NET=FILE, SHOT_DISK=FILE: the network's graph, the disks'
snap() { [ -n "$2" ] || return 0; graph toggle "$1"; sleep "${SHOT_AFTER:-3}"; shot "$t/$1.png"; cp "$t/$1.png" "$2"; graph toggle power > /dev/null; sleep 0.5; }
snap network "${SHOT_NET:-}"
snap disk "${SHOT_DISK:-}"
# SHOT_PROC=FILE: the processes and the temperature's graph
if [ -n "${SHOT_PROC:-}" ]; then
    sched processes; graph toggle temperature; sleep "${SHOT_AFTER:-3}"; shot "$t/proc.png"; cp "$t/proc.png" "$SHOT_PROC"
    graph toggle power; sched equipment; sleep 1
fi
# zone B2: the schedule 7 mm into it, its first column's rule 9 mm on
zx=$(awk -v w="$(o .zones.width)" -v m="$mmL" 'BEGIN { v = w / m; c = int(v); if (c < v - 1e-6) c++; print c + 7 }')
zy=$(awk -v h="$(o .zones.height)" -v m="$mmL" 'BEGIN { v = h / m; c = int(v); if (c < v - 1e-6) c++; print c + 7 }')
run "$cli" config set 'sheet.schedule_at=B2' > /dev/null
sleep 1.5
shot "$t/notes.png"
ok "zone B2: ${zx} mm, ${zy} mm from the frame's corner" "$(rule $((zx + 9)) $((zy + 20)))" "exact"
# dragged by its heading: 10 mm right and 5 mm down; config.toml keeps the place
gx=$(awk -v f="$fxL" -v m="$mmL" -v k="$zx" 'BEGIN { print f + (k + 20) * m }'); gy=$(awk -v f="$fyL" -v m="$mmL" -v k="$zy" 'BEGIN { print f + (k + 3) * m }')
run "$comp" msg pointer "$gx" "$gy" > /dev/null; sleep 0.2
run "$comp" msg button left down > /dev/null; sleep 0.2
run "$comp" msg pointer "$(awk -v g="$gx" -v m="$mmL" 'BEGIN { print g + 5 * m }')" "$gy" > /dev/null; sleep 0.1
run "$comp" msg pointer "$(awk -v g="$gx" -v m="$mmL" 'BEGIN { print g + 10 * m }')" "$(awk -v g="$gy" -v m="$mmL" 'BEGIN { print g + 5 * m }')" > /dev/null; sleep 0.2
run "$comp" msg button left up > /dev/null
ok "dragged 10 mm right, 5 mm down: kept in millimetres" "$(waitcfg "^schedule_at = \\[$((zx + 10)), $((zy + 5))\\]")" "1"
sleep 1.5
shot "$t/notes.png"
ok "and drawn there, on its millimetre" "$(rule $((zx + 19)) $((zy + 25)))" "exact"
ok "the graph still up, out of its way" "$(layers load)" "1"
# over a wallpaper: both still there (on the paper's halo)
magick -size 1920x1200 gradient:"#8C9AA8"-"#4A463E" "$t/pic.png" 2> /dev/null || convert -size 1920x1200 gradient:"#8C9AA8"-"#4A463E" "$t/pic.png"
run "$cli" config set "wallpaper=$t/pic.png" > /dev/null
sleep 2
ok "over a wallpaper: both still there" "$(layers schedule)$(layers load)" "11"
[ -n "${SHOT_PIC:-}" ] && { sleep "${SHOT_AFTER:-0}"; shot "$t/pic-shot.png"; cp "$t/pic-shot.png" "$SHOT_PIC"; }
ok "no QML warning from the notes" "$(grep -c 'Schedule.qml\|LoadGraph.qml\|sheet.js' "$t/qs.log")" "0"
[ "$fail" = 0 ] && echo "notes: ok" || { grep -a 'Schedule.qml\|LoadGraph.qml\|sheet.js' "$t/qs.log" | head -5; echo "notes: FAILED"; }
exit $fail
