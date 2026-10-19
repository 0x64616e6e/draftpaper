#!/bin/sh
# make pixels (and alone): the sheet's own client (sheet/, docs/sheet-client.md stage 0), headless
# (nothing on any display). Four margins a display, on the bottom layer, and in each a rule a physical
# pixel wide 50 mm from the frame's corner — drawn into shared memory at the display's own physical
# pixels, with no drawing context on the GPU. The rule is the whole point: it must be one pixel wide and
# on its pixel at every scale, 1.25 and 1.75 included, which is what a client that takes fractional scale
# for granted gets wrong. Two displays of different densities get their own geometry.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
client=$repo/sheet/target/release/draftpaper-sheet
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-sheet.XXXXXX)
mkdir -p "$t/Pictures"
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

# a compositor of `$1` outputs at scale `$2`, densities `$3`, with the client on it
start() {
    rm -f "$t"/Pictures/*.png
    env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" XDG_PICTURES_DIR="$t/Pictures" DRAFTPAPER_MM="$3" \
        "$comp" --headless "$1" --scale "$2" --command true > "$t/log" 2>&1 &
    nest=$!
    i=0
    until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
    wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
    sock=$runtime/draftpaper-$wl.sock
    env -u DISPLAY HOME="$t" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" "$client" > "$t/client.log" 2>&1 &
    sheet=$!
    pids="$nest $sheet"
    i=0
    until [ "$(layers)" = "$4" ] || [ $i -gt 200 ]; do sleep 0.1; i=$((i + 1)); done
}
stop() { kill $sheet 2>/dev/null; sleep 0.3; kill $nest 2>/dev/null; wait 2>/dev/null; sleep 0.5; pids=; }
layers() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs 2>/dev/null | grep -c '"draftpaper-margin"'; }
o() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r ".[$1] | $2"; }
# every display as it is drawn, to $t/shot-<output>.png
shots() {
    rm -f "$t"/Pictures/*.png "$t"/shot-*.png
    kill -USR1 $nest
    i=0
    until [ "$(ls "$t"/Pictures/*.png 2> /dev/null | wc -l)" = "$1" ] || [ $i -gt 80 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    for f in "$t"/Pictures/*.png; do
        mv "$f" "$t/shot-$(basename "$f" .png | sed 's/^draftpaper-[0-9]*-//').png"
    done
}
# the rule 50 mm from the frame's corner, in physical pixels: the sheet's own `at` (qml/sheet.js)
ruleAt() { awk -v f="$1" -v o="$2" -v m="$3" -v s="$4" 'BEGIN { printf "%d", int((f - o + 50 * m) * s + 0.5) }'; }
# dark on its pixel, paper on both sides: one physical pixel wide, wherever the scale falls
sharp() {
    img=$1; x=$2; y=$3; dx=$4; dy=$5
    on=$("$check" "$img" "$sc" "$mm" "$mg" lum "$x" "$y")
    a=$("$check" "$img" "$sc" "$mm" "$mg" lum $((x - dx)) $((y - dy)))
    b=$("$check" "$img" "$sc" "$mm" "$mg" lum $((x + dx)) $((y + dy)))
    if [ "$on" -lt $((a - 60)) ] && [ "$on" -lt $((b - 60)) ]; then echo sharp; else echo "blurred: $a $on $b"; fi
}

for sc in 1 1.25 1.5 1.75 2; do
    start 1920x1200 "$sc" 6.4 4
    ok "scale $sc: a margin a side, on the bottom layer" "$(layers)" "4"
    mm=$(o 0 .px_per_mm); mg=$(o 0 .margin); fx=$(o 0 .frame.x); fy=$(o 0 .frame.y); ox=$(o 0 .x); oy=$(o 0 .y)
    shots 1
    x=$(ruleAt "$fx" "$ox" "$mm" "$sc")
    y=$(ruleAt "$fy" "$oy" "$mm" "$sc")
    ok "scale $sc: the top margin's rule on its pixel, one pixel wide" "$(sharp "$t/shot-headless.png" "$x" 12 1 0)" "sharp"
    ok "scale $sc: the left margin's too" "$(sharp "$t/shot-headless.png" 12 "$y" 0 1)" "sharp"
    ok "scale $sc: nothing said by the client" "$(grep -ci "error\|protocol" "$t/client.log")" "0"
    stop
done

# two displays, each its own density: each gets its own four margins and its own geometry
start 1920x1200,2560x1440 1.5 6.4,5.0 8
ok "two displays: four margins each" "$(layers)" "8"
mm=$(o 0 .px_per_mm); mg=$(o 0 .margin); fx=$(o 0 .frame.x); fy=$(o 0 .frame.y); ox=$(o 0 .x); oy=$(o 0 .y)
mm2=$(o 1 .px_per_mm); fx2=$(o 1 .frame.x); ox2=$(o 1 .x)
sc=1.5
shots 2
x=$(ruleAt "$fx" "$ox" "$mm" "$sc")
ok "the first display's rule" "$(sharp "$t/shot-headless.png" "$x" 12 1 0)" "sharp"
# the second display's sheet is its own: a different millimetre, so a different place for 50 mm
x2=$(ruleAt "$fx2" "$ox2" "$mm2" "$sc")
ok "the second's, at its own millimetre" "$(sharp "$t/shot-headless-2.png" "$x2" 12 1 0)" "sharp"
ok "and they are not the same place" "$([ "$x" != "$x2" ] && echo differ || echo "both $x")" "differ"
stop

[ "$fail" = 0 ] && echo "sheet: ok" || echo "sheet: FAILED"
exit $fail
