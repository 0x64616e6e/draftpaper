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
mkdir -p "$t/Pictures" "$t/.config/draftpaper"
cat > "$t/.config/draftpaper/config.toml" <<'TOML'
theme = "softglow"
margin = 6
[title]
project = "signal"
drawn = "drafter"
TOML
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

# a compositor of `$1` outputs at scale `$2`, densities `$3`, with the client on it
start() {
    rm -f "$t"/Pictures/*.png
    env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" XDG_PICTURES_DIR="$t/Pictures" DRAFTPAPER_MM="$3" \
        "$comp" --headless "$1" --scale "$2" --command true > "$t/log" 2>&1 &
    nest=$!
    i=0
    until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
    wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
    sock=$runtime/draftpaper-$wl.sock
    # the theme the compositor draws the paper in, so the client letters in the same ink
    env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" \
        DRAFTPAPER_CACHE="$t/.cache/draftpaper" "$repo/cli/target/release/draftpaper" apply > /dev/null 2>&1
    env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" \
        DRAFTPAPER_CACHE="$t/.cache/draftpaper" "$client" > "$t/client.log" 2>&1 &
    sheet=$!
    pids="$nest $sheet"
    i=0
    until [ "$(layers)" = "$4" ] || [ $i -gt 200 ]; do sleep 0.1; i=$((i + 1)); done
}
stop() { kill $sheet 2>/dev/null; sleep 0.3; kill $nest 2>/dev/null; wait 2>/dev/null; sleep 0.5; pids=; }
layers() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs 2>/dev/null | grep -c '"draftpaper-margin"'; }
blocks() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs 2>/dev/null | grep -c '"draftpaper-title-block"'; }
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

# the title block: twelve cells, each with its label lettered and its reading under it
start 1920x1200 1.5 6.4 4
ok "the title block is up" "$(blocks)" "1"
sc=1.5
mm=$(o 0 .px_per_mm); mg=$(o 0 .margin)
shots 1
# ink anywhere in a band of a cell, counted in physical pixels: the label sits 1.2 mm down, the reading
# stands 1.6 mm above the cell's foot
ink() {
    env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r --argjson i "$1" --argjson sc "$sc" --argjson mm "$mm" \
        '.[0].title.cells[$i] | "\(.x) \(.y) \(.width) \(.height)"' |
    while read -r cx cy cw ch; do
        awk -v cx="$cx" -v cy="$cy" -v cw="$cw" -v ch="$ch" -v sc="$sc" -v mm="$mm" -v band="$2" \
            'BEGIN {
                x0 = int((cx + 2 * mm) * sc); x1 = int((cx + cw - 1 * mm) * sc)
                if (band == "label") { y0 = int((cy + 1.0 * mm) * sc); y1 = int((cy + 4.2 * mm) * sc) }
                else { y0 = int((cy + ch - 5.4 * mm) * sc); y1 = int((cy + ch - 1.2 * mm) * sc) }
                print x0, y0, x1, y1
            }'
    done
}
# how many pixels in that band are darker than the paper around them
darkness() {
    read -r x0 y0 x1 y1 <<EOF
$(ink "$1" "$2")
EOF
    "$check" "$t/shot-headless.png" "$sc" "$mm" "$mg" ink "$x0" "$y0" "$x1" "$y1" 2> /dev/null
}
# a lettered band carries dozens of inked pixels; a band with nothing in it carries under a dozen, from
# the rules that edge the cells, so forty tells them apart with room to spare
labelled=0; valued=0
i=0
while [ $i -lt 12 ]; do
    [ "$(darkness $i label)" -gt 40 ] 2> /dev/null && labelled=$((labelled + 1))
    [ "$(darkness $i value)" -gt 40 ] 2> /dev/null && valued=$((valued + 1))
    i=$((i + 1))
done
ok "every cell carries its label" "$labelled" "12"
ok "and eleven of the twelve a reading (the custom cell is empty here)" "$([ "$valued" -ge 11 ] && echo most || echo "$valued")" "most"
ok "nothing said by the client" "$(grep -ci "error\|protocol" "$t/client.log")" "0"
stop

[ "$fail" = 0 ] && echo "sheet: ok" || echo "sheet: FAILED"
exit $fail
