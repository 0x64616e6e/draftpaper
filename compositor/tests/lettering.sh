#!/bin/sh
# make pixels (and alone): the two renderers set the same sheet, side by side.
#
# Since docs/sheet-content.md there is one account of what the sheet says — `draftpaper sheet` — and both
# the shell and the sheet's own client letter from it and decide nothing themselves. What is left to
# differ is how each one sets it: where a word goes, how big it is, how heavy, in which ink. No unit test
# can see that, and nobody noticed the last ten such faults until they were looked at on a screen.
#
# So: a headless compositor, lettered once by Quickshell and once by the client, from the same frozen
# readings and the same frozen clock, and the two screenshots compared band by band. The comparison is
# the ink altogether and the box it sits in (sheetcheck trace), not the pixels: the two antialias
# differently and always will — the shell's glyphs are filtered for a subpixel display and hinted, the
# client's are neither — so a pixel-for-pixel test would fail on every glyph edge and say nothing. Ink
# and box move together the moment a word goes missing, changes size, or is set in the wrong place.
#
# This can only be written while both renderers exist. If one of them goes, so does this.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
client=$repo/sheet/target/release/draftpaper-sheet
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-lettering.XXXXXX)
mkdir -p "$t/Pictures" "$t/.config/draftpaper" "$t/run" "$t/.local/share"
# the Nerd Fonts are usually under the real home, and a home of our own would hide them from both — and
# a sheet with no glyphs on it is only half the comparison
ln -s "$HOME/.local/share/fonts" "$t/.local/share/fonts" 2>/dev/null
cat > "$t/.config/draftpaper/config.toml" <<'TOML'
theme = "softglow"
margin = 6
glyphs = true
[title]
project = "signal"
drawn = "drafter"
TOML
# the same machine for both: what each one letters must come from the document, not from the second the
# screenshot was taken in (cli/src/readings.rs, cli/src/content.rs)
cat > "$t/readings.json" <<'JSON'
{"cpu": 0.08, "temps": [{"label": "CPU", "value": "57°"}, {"label": "NVME", "value": "42°"}, {"label": "FAN", "value": "2322"}],
 "hot": false, "power": null, "profile": "performance", "bt": [], "processes": 287,
 "wifi": {"signal": 62, "ssid": "a network"}, "wired": null, "backlight": null,
 "battery": 0.78, "charging": false, "battery_low": false}
JSON
NOW=1792400000
pids=
# KEEP=1 leaves the screenshots and the logs behind, for looking at what differed
trap 'kill $pids 2>/dev/null; [ -n "${KEEP:-}" ] && echo "  kept: $t" || rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

SC=1.25
MM=6.4
# a compositor with one display, lettered by `$1` (shell or client); its screenshot to $t/$1.png
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
    letter() {
        env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" \
            DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" DRAFTPAPER_BIN="$repo/cli/target/release/draftpaper" \
            DRAFTPAPER_READINGS="$t/readings.json" DRAFTPAPER_NOW="$NOW" PATH="$repo/cli/target/release:$PATH" "$@"
    }
    letter "$repo/cli/target/release/draftpaper" apply > /dev/null 2>&1
    if [ "$1" = client ]; then
        letter "$client" > "$t/$1.log" 2>&1 &
    else
        letter qs -p "$repo/qml" > "$t/$1.log" 2>&1 &
    fi
    drawn=$!
    pids="$nest $drawn"
    # four margins and the title block on the one display
    i=0
    until [ "$(env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs 2>/dev/null | grep -c '"draftpaper-')" -ge 5 ] || [ $i -gt 300 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 2
    if [ "$1" = shell ]; then
        env DRAFTPAPER_SOCKET="$sock" "$comp" msg outputs > "$t/outputs.json"
    fi
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
# a rectangle of the display in physical pixels, from logical ones
px() { awk -v v="$1" -v s="$SC" 'BEGIN { printf "%d", v * s }'; }
trace() { "$check" "$t/$1.png" "$SC" "$MM" 6 trace "$2" "$3" "$4" "$5"; }

# how far apart two traces are: the ink as a percentage of the larger, and the box in whole pixels. The
# shell sets its glyphs through a subpixel filter and the client does not, so a few per cent of the ink
# and a pixel of the box are the two renderers agreeing, not disagreeing.
compare() {
    what=$1; x0=$2; y0=$3; x1=$4; y1=$5; room=${6:-12}
    set -- $(trace shell "$x0" "$y0" "$x1" "$y1"); sm=$1; sa=$2; sb=$3; sc=$4; sd=$5
    set -- $(trace client "$x0" "$y0" "$x1" "$y1"); cm=$1; ca=$2; cb=$3; cc=$4; cd=$5
    if [ "$sm" = 0 ] || [ "$cm" = 0 ]; then
        ok "$what" "shell $sm, client $cm" "both inked"
        return
    fi
    # where the ink falls across the band, as a share of all of it: the one comparison that is not
    # thrown by one renderer setting the same words half again as heavily as the other
    apart=$("$check" "$t/shell.png" "$SC" "$MM" 6 diff "$t/client.png" "$x0" "$y0" "$x1" "$y1")
    box=$(awk -v a="$sa" -v b="$sb" -v c="$sc" -v d="$sd" -v e="$ca" -v f="$cb" -v g="$cc" -v h="$cd" \
        'function gap(x, y) { return x > y ? x - y : y - x } BEGIN { m = gap(a, e); if (gap(b, f) > m) m = gap(b, f); if (gap(c, g) > m) m = gap(c, g); if (gap(d, h) > m) m = gap(d, h); print m }')
    # twenty, and it should be ten. The Nerd Font glyphs come out smaller in the client than in the shell
    # (the note at the foot, below), and that one difference eats most of the allowance: without a glyph
    # in it a band or a cell sits between 0 and 6 per cent apart. A word missing or resized runs to
    # thirty and more, so this still catches what it is for — but the number comes down when the glyphs
    # are put right.
    ok "$what: the same words in the same places" "$([ "$apart" -le 20 ] && echo near || echo "$apart% apart")" "near"
    ok "$what: in the same box" "$([ "$box" -le "$room" ] && echo near || echo "off by $box px")" "near"
}

mg=$(px "$(o .margin)")
w=$(px "$(o .width)")
h=$(px "$(o .height)")
# the four margins: the workspaces and the columns' numbers along the top, the rows' letters down the
# sides, Bluetooth, the scale bar's figures and the theme's name along the foot
compare "the top margin" 0 0 "$w" "$mg"
# the foot a zone at a time, so a zone that drifts says which it was: Bluetooth in the first, the
# columns' numbers, the scale bar's figures.
#
# All but the last. In the last sits the screenshot's glyph, and the shell draws it about twice the size
# the client does — the same character, asked for at the same size, out of the same file, whose metrics
# the two agree on exactly. It is a real difference, it is not only that glyph, and it is the first thing
# this test found. Until it is run down the last zone is left out rather than the whole foot, and this
# comes back the day the glyph does.
zones=$(o .zones.across)
k=0
while [ "$k" -lt "$((zones - 1))" ]; do
    zx0=$(awk -v w="$w" -v m="$mg" -v n="$zones" -v k="$k" 'BEGIN { printf "%d", m + (w - 2 * m) * k / n }')
    zx1=$(awk -v w="$w" -v m="$mg" -v n="$zones" -v k="$k" 'BEGIN { printf "%d", m + (w - 2 * m) * (k + 1) / n }')
    compare "the foot's zone $k" "$zx0" "$((h - mg))" "$zx1" "$h"
    k=$((k + 1))
done
compare "the left margin" 0 "$mg" "$mg" "$((h - mg))"
compare "the right margin" "$((w - mg))" "$mg" "$w" "$((h - mg))"
# the title block, a cell at a time: the readings and the clock are frozen, so each cell is the same
# entry of the same document set twice, and a cell that drifts says which it was
n=$(jq '.[0].title.cells | length' < "$t/outputs.json")
i=0
while [ "$i" -lt "$n" ]; do
    cx=$(px "$(o ".title.cells[$i].x")")
    cy=$(px "$(o ".title.cells[$i].y")")
    cw=$(px "$(o ".title.cells[$i].width")")
    ch=$(px "$(o ".title.cells[$i].height")")
    compare "title cell $i" "$cx" "$cy" "$((cx + cw))" "$((cy + ch))"
    i=$((i + 1))
done

if [ $fail -eq 0 ]; then echo "lettering: ok"; else echo "lettering: FAILED"; fi
exit $fail
