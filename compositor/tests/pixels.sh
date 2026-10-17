#!/bin/sh
# make pixels: the sheet drawn at every scale and checked pixel by pixel (examples/sheetcheck.rs). The
# compositor runs headless (--headless: drawn offscreen on the GPU's render node, never on a display, no
# window anywhere), with this checkout's shell, at 1, 1.25, 1.5, 1.75 and 2: with paper margins, with a
# wallpaper's ambient margins, with two terminals side by side, and with the magnifier out on the
# title block's corner; each screenshot is checked, then everything is closed and removed. Needs no
# running session. ROUNDS and SCALES choose fewer (ROUNDS=lens SCALES=1.5 make pixels).
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
check=$repo/compositor/target/release/examples/sheetcheck
cli=$repo/cli/target/release/draftpaper
size=3840x2160
mm=6.2
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
wallpaper=$(ls /usr/share/backgrounds/*/*.jpg /usr/share/backgrounds/*/*.png 2>/dev/null | head -1)
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-pixels.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run"
plain='theme = "softglow"
margin = 6
[apps]
skip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]'
nest= shell= terms=
stop() {
    for p in $terms $shell $nest; do kill "$p" 2>/dev/null; done
    terms= shell= nest=
}
trap 'stop; rm -rf "$t"' EXIT INT TERM

# one screenshot: headless at scale $1, saved as $2; $3 "windows" opens two terminals
shot() {
    : > "$t/log"
    env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" DRAFTPAPER_MM=$mm "$comp" --headless $size --scale "$1" --command true >> "$t/log" 2>&1 &
    nest=$!
    i=0
    until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
    wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
    [ -n "$wl" ] || { echo "the headless compositor did not start:"; tail -5 "$t/log"; return 1; }
    env -u DISPLAY HOME="$t" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$runtime/draftpaper-$wl.sock" DRAFTPAPER_BIN="$cli" \
        DRAFTPAPER_ROOT="$repo" DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" \
        qs -p "$repo/qml/shell.qml" -n > "$t/qs.log" 2>&1 &
    shell=$!
    # two terminals side by side, grey: unlike either border colour, so their edges show
    if [ "${3:-}" = windows ]; then
        for i in 1 2; do
            env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" foot -o colors.background=808080 > /dev/null 2>&1 &
            terms="$terms $!"
            sleep 0.5
        done
    fi
    # the shell up: its title block on the output (the compositor draws the block's rules once it is;
    # with a large wallpaper that can take longer than any fixed wait), then a moment for the frames
    i=0
    until DRAFTPAPER_SOCKET="$runtime/draftpaper-$wl.sock" "$comp" msg outputs 2>/dev/null | grep -q draftpaper-title-block || [ $i -gt 300 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 1.5
    rm -f "$t"/Pictures/*.png
    # the magnifier: a screenshot without it, then the pointer at $4 $5 and the lens out
    if [ "${3:-}" = lens ]; then
        kill -USR1 "$nest"
        sleep 1
        mv "$t"/Pictures/*.png "${2%.png}-before.png"
        DRAFTPAPER_SOCKET="$runtime/draftpaper-$wl.sock" "$comp" msg pointer "$4" "$5" > /dev/null
        DRAFTPAPER_SOCKET="$runtime/draftpaper-$wl.sock" "$comp" msg zoom > /dev/null
        sleep 0.5
    fi
    kill -USR1 "$nest"
    sleep 1
    mv "$t"/Pictures/*.png "$2"
    stop
    sleep 0.3
}

fail=0
command -v foot > /dev/null || echo "no foot: windows not checked"
for round in ${ROUNDS:-paper ambient windows lens}; do
    case $round in
        ambient)
            [ -n "$wallpaper" ] || { echo "no picture in /usr/share/backgrounds: ambient margins not checked"; continue; }
            printf 'wallpaper = "%s"\nmargins = "ambient"\n%s\n' "$wallpaper" "$plain" > "$t/.config/draftpaper/config.toml" ;;
        windows | lens)
            [ $round = lens ] || command -v foot > /dev/null || continue
            printf '%s\n' "$plain" > "$t/.config/draftpaper/config.toml" ;;
        *)
            printf '%s\n' "$plain" > "$t/.config/draftpaper/config.toml" ;;
    esac
    for scale in ${SCALES:-1 1.25 1.5 1.75 2}; do
        # the lens on the pointer just inside the title block's top-right corner (physical 3797, 1897)
        x=$(echo "3797.5 / $scale" | bc -l) y=$(echo "1897.5 / $scale" | bc -l)
        shot $scale "$t/sheet.png" $round $x $y || { fail=1; continue; }
        case $round in
            paper) flag= ;;
            lens) flag="lens $t/sheet-before.png $x $y" ;;
            *) flag=$round ;;
        esac
        # (the check's own status: through a pipe it would be sed's)
        out=$("$check" "$t/sheet.png" $scale $mm 6 $flag) || fail=1
        echo "$out" | sed "s|$t/sheet.png|$round, scale $scale|"
    done
done
exit $fail
