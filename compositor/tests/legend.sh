#!/bin/sh
# make pixels (and alone): the legend's marks, set the same by both renderers.
#
# compositor/tests/lettering.sh compares the two renderers on an empty sheet, which is where the title
# block and the margins can be seen. The legend's *marks* cannot be compared there: a mark goes on a
# window, and a window covers the title block the rest of that test is looking at. So they are compared
# here instead, on a sheet with two windows on it.
#
# What is compared is a box around each window's middle, where the legend letters the key that would
# reach it. The mark lays paper over the window before it letters, so what is underneath is the same in
# both runs even though they are two separate compositors.
#
# Which window each key reaches is the compositor's answer (`focus_to` in the outputs, worked out by the
# same code `focus left` runs), and which key that is comes off the keymap it publishes. Neither
# renderer works either out, so what this catches is the drawing: a mark in the wrong place, the wrong
# size, or missing (docs/keyboard.md).
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
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-legend.XXXXXX)
mkdir -p "$t/Pictures" "$t/.config/draftpaper" "$t/run" "$t/.local/share"
ln -s "$HOME/.local/share/fonts" "$t/.local/share/fonts" 2>/dev/null
cat > "$t/.config/draftpaper/config.toml" <<'TOML'
theme = "softglow"
margin = 6
[title]
project = "signal"
drawn = "drafter"
[keys]
# the passing word stands long enough to be in the screenshot: a word takes itself back after
# [keys] word_for, and these two runs are a good few seconds apart (docs/keyboard.md)
word_for = 10000
TOML
pids=
trap 'kill $pids 2>/dev/null; [ -n "${KEEP:-}" ] && echo "  kept: $t" || rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

SC=1.25
MM=6.4
# a compositor with two windows and the legend out, lettered by `$1`; its screenshot to $t/$1.png
render() {
    rm -f "$t"/Pictures/*.png
    : > "$t/log"
    env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" XDG_PICTURES_DIR="$t/Pictures" DRAFTPAPER_MM="$MM" \
        "$comp" --headless 1920x1200 --scale "$SC" --command true > "$t/log" 2>&1 &
    nest=$!
    i=0
    until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 150 ]; do sleep 0.1; i=$((i + 1)); done
    wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
    if [ -z "$wl" ]; then
        echo "  FAIL: the compositor would not start:"
        tail -3 "$t/log"
        fail=1
        return 1
    fi
    sock=$runtime/draftpaper-$wl.sock
    msg() { env DRAFTPAPER_SOCKET="$sock" "$comp" msg "$@"; }
    letter() {
        env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" \
            DRAFTPAPER_ROOT="$repo" DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" \
            DRAFTPAPER_BIN="$repo/cli/target/release/draftpaper" "$@"
    }
    letter "$repo/cli/target/release/draftpaper" apply > /dev/null 2>&1
    if [ "$1" = client ]; then
        letter "$client" > "$t/$1.log" 2>&1 &
    else
        letter qs -p "$repo/qml" > "$t/$1.log" 2>&1 &
    fi
    drawn=$!
    pids="$nest $drawn"
    i=0
    until [ "$(msg outputs 2>/dev/null | grep -c '"draftpaper-')" -ge 5 ] || [ $i -gt 300 ]; do sleep 0.1; i=$((i + 1)); done
    # two windows: one the focus keys act from, one Mod+h would reach. `sleep` keeps them flat, so the
    # two runs put the same thing under the marks.
    msg spawn alacritty -e sleep 90 > /dev/null 2>&1
    sleep 3
    msg spawn alacritty -e sleep 90 > /dev/null 2>&1
    sleep 4
    # Mod cannot be held in a headless compositor, so the legend is asked for the way a binding of one's
    # own would ask (docs/keyboard.md)
    msg legend > /dev/null 2>&1
    sleep 1
    # and a passing word said on it, so the heading is compared with one standing: the phrasing is the
    # document's, and this is what would catch a renderer going back to one of its own
    msg word unbound q > /dev/null 2>&1
    sleep 1
    if [ "$1" = shell ]; then
        msg outputs > "$t/outputs.json"
        msg windows > "$t/windows.json"
    fi
    rm -f "$t"/Pictures/*.png
    kill -USR1 $nest
    i=0
    until [ "$(ls "$t"/Pictures/*.png 2> /dev/null | wc -l)" -ge 1 ] || [ $i -gt 80 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.5
    cp "$t"/Pictures/*.png "$t/$1.png" 2> /dev/null || { echo "  FAIL: no screenshot from the $1"; fail=1; }
    kill $drawn 2> /dev/null
    sleep 0.3
    kill $nest 2> /dev/null
    wait 2> /dev/null
    sleep 1
    pids=
}

render shell || exit 1
render client || exit 1
ok "the shell said nothing" "$(grep -cE "ERROR|TypeError|not a function" "$t/shell.log")" "0"
ok "nor the client" "$(grep -cE "protocol error|panicked" "$t/client.log")" "0"

o() { jq -r ".[0] | $1" < "$t/outputs.json"; }
px() { awk -v v="$1" -v s="$SC" 'BEGIN { printf "%d", v * s }'; }
trace() { "$check" "$t/$1.png" "$SC" "$MM" 6 trace "$2" "$3" "$4" "$5"; }
compare() {
    what=$1; x0=$2; y0=$3; x1=$4; y1=$5; room=${6:-12}
    set -- $(trace shell "$x0" "$y0" "$x1" "$y1"); sm=$1; sa=$2; sb=$3; sc=$4; sd=$5
    set -- $(trace client "$x0" "$y0" "$x1" "$y1"); cm=$1; ca=$2; cb=$3; cc=$4; cd=$5
    if [ "$sm" = 0 ] || [ "$cm" = 0 ]; then
        ok "$what" "shell $sm, client $cm" "both inked"
        return
    fi
    apart=$("$check" "$t/shell.png" "$SC" "$MM" 6 diff "$t/client.png" "$x0" "$y0" "$x1" "$y1")
    box=$(awk -v a="$sa" -v b="$sb" -v c="$sc" -v d="$sd" -v e="$ca" -v f="$cb" -v g="$cc" -v h="$cd" \
        'function gap(x, y) { return x > y ? x - y : y - x } BEGIN { m = gap(a, e); if (gap(b, f) > m) m = gap(b, f); if (gap(c, g) > m) m = gap(c, g); if (gap(d, h) > m) m = gap(d, h); print m }')
    ok "$what: the same words in the same places" "$([ "$apart" -le 20 ] && echo near || echo "$apart% apart")" "near"
    ok "$what: in the same box" "$([ "$box" -le "$room" ] && echo near || echo "off by $box px")" "near"
}

# the compositor found a window each way, or there is nothing here to compare
ok "a window the focus keys act from" "$(jq -r '.[0].focus_to.focused // "none"' < "$t/outputs.json" | sed 's/[0-9][0-9]*/one/')" "one"
ok "and one to the left of it" "$(jq -r '.[0].focus_to.left // "none"' < "$t/outputs.json" | sed 's/[0-9][0-9]*/one/')" "one"

# the legend's heading, with a passing word standing on its right: "LEGEND" at one end and
# "NOTHING ON q" at the other. Both come from the document (content::legend()), so what this catches
# is a renderer phrasing one of them itself — which is how the two came to disagree before
# (docs/keyboard.md). The strip stops short of the rule under the heading, so it is about the words.
mmpx() { awk -v a="$1" -v m="$(o .px_per_mm)" -v b="$2" 'BEGIN { print a + b * m }'; }
compare "the legend's heading, a word on it" \
    "$(px "$(mmpx "$(o .frame.x)" 4)")" "$(px "$(mmpx "$(o .frame.y)" 4)")" \
    "$(px "$(mmpx "$(o .frame.x)" "$(awk -v w="$(o .frame.width)" -v m="$(o .px_per_mm)" 'BEGIN { print w / m / 2 }')")")" \
    "$(px "$(mmpx "$(o .frame.y)" 13)")"

# a box around each window's middle: the mark, and the paper it lays down first
wn() { jq -r ".[$1] | .geometry.$2" < "$t/windows.json"; }
k=0
while [ "$k" -lt "$(jq length < "$t/windows.json")" ]; do
    if [ "$(jq -r ".[$k].geometry" < "$t/windows.json")" = null ]; then
        k=$((k + 1))
        continue
    fi
    r=$(awk -v m="$(o .px_per_mm)" 'BEGIN { print 9 * m }')
    cx=$(awk -v x="$(wn $k x)" -v w="$(wn $k width)" 'BEGIN { print x + w / 2 }')
    cy=$(awk -v y="$(wn $k y)" -v h="$(wn $k height)" 'BEGIN { print y + h / 2 }')
    compare "the mark on window $k" \
        "$(px "$(awk -v c="$cx" -v r="$r" 'BEGIN { print c - r }')")" \
        "$(px "$(awk -v c="$cy" -v r="$r" 'BEGIN { print c - r }')")" \
        "$(px "$(awk -v c="$cx" -v r="$r" 'BEGIN { print c + r }')")" \
        "$(px "$(awk -v c="$cy" -v r="$r" 'BEGIN { print c + r }')")"
    k=$((k + 1))
done

if [ $fail -eq 0 ]; then echo "legend: ok"; else echo "legend: FAILED"; fi
exit $fail
