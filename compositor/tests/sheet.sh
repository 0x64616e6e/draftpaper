#!/bin/sh
# make pixels (and alone): the sheet's own client (sheet/, docs/sheet-client.md stage 0), headless
# (nothing on any display). Four margins a display, on the bottom layer, and in each a rule a physical
# pixel wide 50 mm from the frame's corner — drawn into shared memory at the display's own physical
# pixels, with no drawing context on the GPU. The rule is the whole point: it must be one pixel wide and
# on its pixel at every scale, 1.25 and 1.75 included, which is what a client that takes fractional scale
# for granted gets wrong. Two displays of different densities get their own geometry.
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
client=$repo/sheet/target/release/draftpaper-sheet
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-sheet.XXXXXX)
mkdir -p "$t/Pictures" "$t/.config/draftpaper" "$t/run"
cat > "$t/.config/draftpaper/config.toml" <<'TOML'
theme = "softglow"
margin = 6
[title]
project = "signal"
drawn = "drafter"
[sheet]
# the whole test letters the sheet with its own client; the swap below checks that `draftpaper start`
# honours this, and that the settings still find a Quickshell of their own
lettering = "client"
TOML
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

# a compositor of `$1` outputs at scale `$2`, densities `$3`, with the client on it; `$4` layers to wait
# for and `$5` a program for the compositor to run. Starting one can lose a race with the one before it
# letting go of the graphics device, so it is tried twice before the test gives up on it.
start() {
    try=0
    while [ $try -lt 2 ]; do
        try=$((try + 1))
        rm -f "$t"/Pictures/*.png
        : > "$t/log"
        env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" XDG_PICTURES_DIR="$t/Pictures" DRAFTPAPER_MM="$3" \
            "$comp" --headless "$1" --scale "$2" --command "${5:-true}" > "$t/log" 2>&1 &
        nest=$!
        i=0
        until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 150 ]; do sleep 0.1; i=$((i + 1)); done
        wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
        if [ -n "$wl" ]; then
            break
        fi
        echo "  (the compositor did not start; trying again)"
        kill $nest 2>/dev/null
        wait 2>/dev/null
        sleep 1
    done
    if [ -z "$wl" ]; then
        echo "  FAIL: the compositor would not start:"
        tail -3 "$t/log"
        fail=1
        return 1
    fi
    sock=$runtime/draftpaper-$wl.sock
    # the theme the compositor draws the paper in, so the client letters in the same ink
    env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" \
        DRAFTPAPER_CACHE="$t/.cache/draftpaper" "$repo/cli/target/release/draftpaper" apply > /dev/null 2>&1
    env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" \
        DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" DRAFTPAPER_SHEET_RULE=1 "$client" > "$t/client.log" 2>&1 &
    sheet=$!
    pids="$nest $sheet"
    i=0
    until [ "$(layers)" = "$4" ] || [ $i -gt 200 ]; do sleep 0.1; i=$((i + 1)); done
}
stop() { kill $sheet 2>/dev/null; sleep 0.3; kill $nest 2>/dev/null; wait 2>/dev/null; sleep 0.5; pids=; }
layers() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs 2>/dev/null | grep -c '"draftpaper-margin"'; }
blocks() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs 2>/dev/null | grep -c '"draftpaper-title-block"'; }
tool_layers() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs 2>/dev/null | grep -c '"draftpaper-settings"'; }
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
# a schedule's place on a display whose corner is not the layout's is settled by the client's own tests
# (sheet/src/sheet.rs): the schedule goes to the screen in use, and nothing here can change which that is.
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

# the margins: the workspaces along the top, the columns numbered, a letter at the head of every row
# down the sides, and the scale bar's figures and the theme's name along the foot
start 1920x1200 1.5 6.4 4
sc=1.5
mm=$(o 0 .px_per_mm); mg=$(o 0 .margin); w=$(o 0 .width); h=$(o 0 .height); across=$(o 0 .zones.across); down=$(o 0 .zones.down)
shots 1
# ink in a stretch of a band, given in the display's logical pixels
band() {
    awk -v a="$1" -v b="$2" -v c="$3" -v d="$4" -v sc="$sc" 'BEGIN { printf "%d %d %d %d", a * sc, b * sc, c * sc, d * sc }'
}
inked() {
    set -- $(band "$1" "$2" "$3" "$4")
    "$check" "$t/shot-headless.png" "$sc" "$mm" "$mg" ink "$1" "$2" "$3" "$4"
}
# zone k of the top band, between the frame's corners
zone() { awk -v k="$1" -v w="$w" -v mg="$mg" -v n="$across" 'BEGIN { z = (w - 2 * mg) / n; printf "%f %f", mg + k * z, mg + (k + 1) * z }'; }
set -- $(zone 0); first_a=$1; first_b=$2
set -- $(zone 1); second_a=$1; second_b=$2
ok "the workspace is lettered in the top margin's first zone" "$([ "$(inked "$first_a" 0 "$first_b" "$mg")" -gt 20 ] && echo lettered || echo bare)" "lettered"
ok "and the next zone carries its column's number" "$([ "$(inked "$second_a" 0 "$second_b" "$mg")" -gt 20 ] && echo numbered || echo bare)" "numbered"
# the left margin's first row: its letter sits at the head of the cell
ok "a letter at the head of the first row" "$([ "$(inked 0 "$mg" "$mg" "$(awk -v m="$mg" -v mm="$mm" 'BEGIN { print m + 8 * mm }')")" -gt 20 ] && echo lettered || echo bare)" "lettered"
ok "and the right margin's too" "$([ "$(inked "$(awk -v w="$w" -v m="$mg" 'BEGIN { print w - m }')" "$mg" "$w" "$(awk -v m="$mg" -v mm="$mm" 'BEGIN { print m + 8 * mm }')")" -gt 20 ] && echo lettered || echo bare)" "lettered"
# the foot: the scale bar's figures, and the theme's name in the last zone
set -- $(zone $((across - 1))); last_a=$1; last_b=$2
ok "the theme's name along the foot" "$([ "$(inked "$last_a" "$(awk -v h="$h" -v m="$mg" 'BEGIN { print h - m }')" "$last_b" "$h")" -gt 20 ] && echo named || echo bare)" "named"
ok "the scale bar's figures over its rules" "$(awk -v x="$(o 0 .scale_bar.text.y)" 'BEGIN { print (x > 0) ? "placed" : "none" }')" "placed"
ok "nothing said by the client" "$(grep -ci "error\|protocol" "$t/client.log")" "0"
stop

# the margins answer the pointer: a workspace shown, a window parked and called back, and the cell under
# the pointer marked
start 1920x1200 1.5 6.4 4 "foot"
sc=1.5
mm=$(o 0 .px_per_mm); mg=$(o 0 .margin); w=$(o 0 .width); across=$(o 0 .zones.across)
act() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg "$@" > /dev/null 2>&1; }
shown() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg workspaces | jq -r '[.[] | select(.shown)][0].number'; }
parked() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg windows | jq -r '[.[] | select(.parked)] | length'; }
click() {
    act pointer "$1" "$2"; sleep 0.4
    act button left down; sleep 0.1
    act button left up; sleep 1.2
}
act workspace 2; sleep 1.2
ok "two workspaces, the second shown" "$(shown)" "2"
# the workspaces share the top margin's first zone, a slot each
first=$(awk -v w="$w" -v m="$mg" -v n="$across" 'BEGIN { z = (w - 2 * m) / n; print m + z * 0.25 }')
click "$first" "$(awk -v m="$mg" 'BEGIN { print m / 2 }')"
ok "a click on the first workspace shows it" "$(shown)" "1"
# parked, its title up the margin; a click on its row calls it back
act park; sleep 1.2
ok "a window parked" "$(parked)" "1"
cell=$(o 0 '.parking[0] | .y + .height / 2')
click "$(awk -v m="$mg" 'BEGIN { print m / 2 }')" "$cell"
ok "a click on its row brings it back" "$(parked)" "0"
# and a click on an empty row puts the focused window there
click "$(awk -v m="$mg" 'BEGIN { print m / 2 }')" "$cell"
ok "a click on an empty row parks the focused window" "$(parked)" "1"
# the pointer over the profile's cell marks it: the cell carries more ink than with the pointer away
act pointer 400 400; sleep 0.8
shots 1
tx=$(o 0 '.title.cells[6] | .x'); ty=$(o 0 '.title.cells[6] | .y')
tw=$(o 0 '.title.cells[6] | .width'); th=$(o 0 '.title.cells[6] | .height')
box() { awk -v a="$tx" -v b="$ty" -v c="$tw" -v d="$th" -v sc="$sc" 'BEGIN { printf "%d %d %d %d", a * sc + 2, b * sc + 2, (a + c) * sc - 2, (b + d) * sc - 2 }'; }
set -- $(box); plain=$("$check" "$t/shot-headless.png" "$sc" "$mm" "$mg" ink "$1" "$2" "$3" "$4" 6)
act pointer "$(awk -v a="$tx" -v c="$tw" 'BEGIN { print a + c / 2 }')" "$(awk -v b="$ty" -v d="$th" 'BEGIN { print b + d / 2 }')"
sleep 1
shots 1
set -- $(box); marked=$("$check" "$t/shot-headless.png" "$sc" "$mm" "$mg" ink "$1" "$2" "$3" "$4" 6)
ok "the cell under the pointer is marked" "$([ "$marked" -gt "$plain" ] && echo marked || echo "$marked not over $plain")" "marked"
ok "nothing said by the client" "$(grep -ci "error\|protocol" "$t/client.log")" "0"
stop

# what the daemons say: the volume just moved, Bluetooth, and a notification along the top
start 1920x1200 1.5 6.4 4
sc=1.5
mm=$(o 0 .px_per_mm); mg=$(o 0 .margin); w=$(o 0 .width); h=$(o 0 .height); across=$(o 0 .zones.across)
zone() { awk -v k="$1" -v w="$w" -v mg="$mg" -v n="$across" 'BEGIN { z = (w - 2 * mg) / n; printf "%f %f", mg + k * z, mg + (k + 1) * z }'; }
inked() {
    set -- $(awk -v a="$1" -v b="$2" -v c="$3" -v d="$4" -v sc="$sc" 'BEGIN { printf "%d %d %d %d", a * sc, b * sc, c * sc, d * sc }')
    "$check" "$t/shot-headless.png" "$sc" "$mm" "$mg" ink "$1" "$2" "$3" "$4"
}
# Bluetooth, in the bottom margin's first zone, says so even with nothing connected
set -- $(zone 0)
shots 1
ok "Bluetooth lettered at the foot" "$([ "$(inked "$1" "$(awk -v h="$h" -v m="$mg" 'BEGIN { print h - m }')" "$2" "$h")" -gt 20 ] && echo lettered || echo bare)" "lettered"
# the volume, as `draftpaper volume` leaves it for the sheet to find
set -- $(zone $((across - 1))); vol_a=$1; vol_b=$2
before=$(inked "$vol_a" 0 "$vol_b" "$mg")
printf '{"volume":0.42,"muted":false,"at":%s}\n' "$(date +%s)" > "$t/run/volume.json"
sleep 2
shots 1
ok "the volume said while it is fresh" "$([ "$(inked "$vol_a" 0 "$vol_b" "$mg")" -gt $((before + 20)) ] && echo said || echo quiet)" "said"
stop

# two displays: a schedule is carried by the one it was called up on, and the other stays bare
start 1920x1200,2560x1440 1.5 6.4,5.0 8
run2() {
    env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" \
        DRAFTPAPER_ROOT="$repo" DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" "$@"
}
run2 "$repo/cli/target/release/draftpaper" schedule equipment > /dev/null 2>&1
sleep 2
carried() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r ".[$1].layers | map(select(. == \"draftpaper-schedule\")) | length"; }
ok "the display it was called up on carries it" "$(carried 0)" "1"
ok "and the other does not" "$(env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r '.[1].schedule')" ""
run2 "$repo/cli/target/release/draftpaper" schedule hide > /dev/null 2>&1
stop

# the swap: `draftpaper start` letters the sheet with its own client where the config says so, and the
# settings are still Quickshell's, started for them and gone once closed
start 1920x1200 1.5 6.4 4
run() {
    env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" \
        DRAFTPAPER_ROOT="$repo" DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" \
        PATH="$repo/sheet/target/release:$PATH" "$@"
}
# the client the test already started is this one; stop it and let `draftpaper start` choose
kill $sheet 2>/dev/null; sleep 1
run "$repo/cli/target/release/draftpaper" start > "$t/start.log" 2>&1 &
started=$!
pids="$nest $started"
i=0
until [ "$(layers)" = "4" ] || [ $i -gt 150 ]; do sleep 0.1; i=$((i + 1)); done
ok "draftpaper start letters the sheet with its own client" "$(layers)/$(blocks)" "4/1"
ok "and nothing of the shell is on the sheet" "$(tool_layers)" "0"
# the settings: a Quickshell of their own, drawing none of the sheet
run "$repo/cli/target/release/draftpaper" settings > /dev/null 2>&1
sleep 2
ok "the settings open on a shell started for them" "$(tool_layers)" "1"
ok "and the sheet is still the client's" "$(layers)/$(blocks)" "4/1"
# the Displays page is built from the outputs as they stand when the settings open, and a shell started
# for the settings alone opens them the moment it starts: it must wait for the compositor to answer
# first, or that page comes up with no display on it
ok "the Displays page knows about the display" "$(run qs ipc -p "$repo/qml" call settings displays 2>/dev/null | tr -d '\r\n')" "1"
run "$repo/cli/target/release/draftpaper" settings > /dev/null 2>&1
sleep 3
ok "closed, the settings go with their shell" "$(tool_layers)" "0"
# the schedules: called up by name, taking turns, and away again
sched() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg schedule "$1" > /dev/null 2>&1 || run "$repo/cli/target/release/draftpaper" schedule "$1" > /dev/null 2>&1; }
layer_sched() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs 2>/dev/null | grep -c '"draftpaper-schedule"'; }
run "$repo/cli/target/release/draftpaper" schedule equipment > /dev/null 2>&1
sleep 2
ok "the equipment schedule comes up" "$(layer_sched)" "1"
mm=$(o 0 .px_per_mm); mg=$(o 0 .margin)
shots 1
tx=$(o 0 .title.rect.x); ty=$(o 0 .title.rect.y); fy=$(o 0 .frame.y); oy=$(o 0 .y)
set -- $(awk -v a="$tx" -v b="$fy" -v c="$oy" -v mm="$mm" -v sc="1.5" 'BEGIN { printf "%d %d %d %d", a * sc + 2, (b - c + 7 * mm) * sc, (a + 300) * sc, (b - c + 40 * mm) * sc }')
ok "and carries lettering in the title block's column" "$([ "$("$check" "$t/shot-headless.png" 1.5 "$mm" "$mg" ink "$1" "$2" "$3" "$4")" -gt 100 ] && echo lettered || echo bare)" "lettered"
run "$repo/cli/target/release/draftpaper" schedule next > /dev/null 2>&1
sleep 2
ok "the next takes its place" "$(layer_sched)" "1"
run "$repo/cli/target/release/draftpaper" schedule hide > /dev/null 2>&1
sleep 2
# a schedule's cell in the title block is a switch: PROJECT calls the equipment schedule up, and a
# second click on it puts the schedule away again, as the graph's cells do (docs/sheet-content.md)
sched_now() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r '.[0].schedule'; }
# the compositor's answer, waited for until it is no longer what it was: a click travels to whoever is
# lettering the sheet and back before the schedule changes, and how long that takes is not ours to
# assume. Waiting for the value we want instead would pass the moment it was asked, where what we want
# is "none" and none is what is already there (notes.sh's waitgraph, for the same reason).
sched_change() { i=0; until [ "$(sched_now)" != "$1" ] || [ $i -gt 80 ]; do sleep 0.1; i=$((i + 1)); done; sched_now; }
# the cell is found by what the document says it does, never by a measured offset: which cells there are
# depends on the machine, and the grid's rows shift by a millimetre with the sheet (notes.sh says the
# same, for the same reason)
cell_click() {
    i=$(run "$repo/cli/target/release/draftpaper" sheet | jq -r --arg a "$1" '[.displays[]][0].title | map(.action == $a) | index(true)')
    if [ "$i" = null ]; then echo "  FAIL: no cell does $1 on this machine"; fail=1; return 1; fi
    act pointer "$(o 0 ".title.cells[$i] | .x + .width / 2")" "$(o 0 ".title.cells[$i] | .y + .height / 2")"
    sleep 0.4; act button left down; sleep 0.1; act button left up
}
cell_click schedule.equipment
ok "a click on PROJECT calls the equipment schedule up" "$(sched_change "")" "equipment"
cell_click schedule.equipment
ok "and a second click on it puts the schedule away" "$(sched_change equipment)" ""
# another schedule's cell takes its place rather than toggling: PROCESSES over the equipment
cell_click schedule.equipment; sched_change "" > /dev/null; cell_click schedule.processes
ok "another cell's schedule takes its place" "$(sched_change equipment)" "processes"
cell_click schedule.processes
ok "and that one goes the same way" "$(sched_change processes)" ""
# the leader (docs/keyboard.md): Mod tapped arms it, and the legend stands while it is armed — a mode
# you cannot see is the one the design argues against, so the two are one state, not two. A key cannot
# be tapped in a headless compositor, so it is armed the way a binding of one's own would arm it.
leader_state() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r '.[0] | "\(.leader)/\(.legend)"'; }
env DRAFTPAPER_SOCKET="$sock" "$comp" msg leader > /dev/null 2>&1; sleep 1
ok "the leader armed brings the legend with it" "$(leader_state)" "true/true"
env DRAFTPAPER_SOCKET="$sock" "$comp" msg leader off > /dev/null 2>&1; sleep 1
ok "and spent, both go" "$(leader_state)" "false/false"
# the legend put away takes the leader with it: the legend is how the leader is not silent, so the
# leader cannot outlive it
env DRAFTPAPER_SOCKET="$sock" "$comp" msg leader > /dev/null 2>&1; sleep 1
env DRAFTPAPER_SOCKET="$sock" "$comp" msg legend off > /dev/null 2>&1; sleep 1
ok "the legend put away takes the leader with it" "$(leader_state)" "false/false"

# the passing word (docs/keyboard.md): the one line the sheet says in answer to a key and then takes
# back. It stands on the legend's paper, so saying one holds the legend up for its moment, and the
# legend going takes it with it. A key cannot be pressed in a headless compositor, so it is said the
# way the compositor itself says it.
word_state() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r '.[0] | "\(.word.say // "-")/\(.word.about // "-")/\(.legend)"'; }
env DRAFTPAPER_SOCKET="$sock" "$comp" msg word unbound q > /dev/null 2>&1; sleep 0.4
ok "a word said stands, and brings the legend under it" "$(word_state)" "unbound/q/true"
# and takes itself back when [keys] word_for is up, the legend with it, since the word raised it
sleep 1.6
ok "and is taken back when its time is up, the legend with it" "$(word_state)" "-/-/false"
# the legend put away takes a word with it: a word has no paper of its own
env DRAFTPAPER_SOCKET="$sock" "$comp" msg word unbound F5 > /dev/null 2>&1; sleep 0.3
env DRAFTPAPER_SOCKET="$sock" "$comp" msg legend off > /dev/null 2>&1; sleep 0.3
ok "the legend put away takes the word with it" "$(word_state)" "-/-/false"
# how it reads is the document's, as the heading's other words are: neither renderer phrases it
ok "the word's phrasing is in the document" "$(run "$repo/cli/target/release/draftpaper" sheet | jq -r '.legend.unbound')" "NOTHING ON"

# the graph: a reading's year, called up by name and by its cell in the title block
run "$repo/cli/target/release/draftpaper" graph load > /dev/null 2>&1
sleep 2
ok "a reading's year comes up" "$(env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs 2>/dev/null | grep -c '"draftpaper-load"')" "1"
ok "and the compositor keeps which" "$(env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r '.[0].graph')" "load"
run "$repo/cli/target/release/draftpaper" graph hide > /dev/null 2>&1
sleep 1.5
ok "away again" "$(env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r '.[0].graph')" ""
# the title block's CPU / MEMORY cell plots the load
cx=$(o 0 '.title.cells[7] | .x + .width / 2'); cy=$(o 0 '.title.cells[7] | .y + .height / 2')
act pointer "$cx" "$cy"; sleep 0.4
act button left down; sleep 0.1
act button left up; sleep 1.5
ok "a click on its cell plots it" "$(env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r '.[0].graph')" "load"
ok "and hide takes it away" "$(env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs | jq -r '.[0].schedule')" ""
kill $started 2>/dev/null
stop

[ "$fail" = 0 ] && echo "sheet: ok" || echo "sheet: FAILED"
exit $fail
