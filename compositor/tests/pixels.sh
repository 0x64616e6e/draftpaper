#!/bin/sh
# make pixels: the sheet drawn at every scale and checked pixel by pixel (examples/sheetcheck.rs). Run from
# a Draftpaper session: a nested compositor (this checkout's) opens fullscreen on the current workspace
# with this checkout's shell, at 1, 1.25, 1.5, 1.75 and 2, without and with a wallpaper's ambient margins;
# each screenshot is checked, then everything is closed and removed.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
check=$repo/compositor/target/release/examples/sheetcheck
cli=$repo/cli/target/release/draftpaper
host=${DRAFTPAPER_SOCKET:?run inside a Draftpaper session}
wallpaper=$(ls /usr/share/backgrounds/*/*.jpg /usr/share/backgrounds/*/*.png 2>/dev/null | head -1)
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-pixels.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run"
plain='theme = "softglow"
margin = 6
[apps]
skip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]'
nest= shell=
stop() {
    [ -n "$shell" ] && kill "$shell" 2>/dev/null
    [ -n "$nest" ] && kill "$nest" 2>/dev/null
    shell= nest=
}
trap 'stop; rm -rf "$t"' EXIT INT TERM

# one screenshot: the nested session at scale $1, saved as $2
shot() {
    : > "$t/log"
    env -u DISPLAY HOME="$t" setsid "$comp" --command true >> "$t/log" 2>&1 &
    nest=$!
    i=0
    until grep -aq "Created new socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
    wl=$(grep -a "Created new socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
    sleep 0.5
    DRAFTPAPER_SOCKET=$host "$comp" msg fullscreen > /dev/null
    sleep 0.5
    WAYLAND_DISPLAY=$wl wlr-randr --output winit --scale "$1"
    env HOME="$t" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$XDG_RUNTIME_DIR/draftpaper-$wl.sock" DRAFTPAPER_BIN="$cli" \
        DRAFTPAPER_ROOT="$repo" DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" \
        setsid qs -p "$repo/qml/shell.qml" -n > "$t/qs.log" 2>&1 &
    shell=$!
    sleep 4
    rm -f "$t"/Pictures/*.png
    kill -USR1 "$nest"
    sleep 1
    mv "$t"/Pictures/*.png "$2"
    mm=$(grep -ao "[0-9.]* px/mm" "$t/log" | head -1 | cut -d' ' -f1)
    stop
    sleep 0.5
}

fail=0
for margins in paper ambient; do
    if [ $margins = ambient ]; then
        [ -n "$wallpaper" ] || { echo "no picture in /usr/share/backgrounds: ambient margins not checked"; continue; }
        printf 'wallpaper = "%s"\nmargins = "ambient"\n%s\n' "$wallpaper" "$plain" > "$t/.config/draftpaper/config.toml"
    else
        printf '%s\n' "$plain" > "$t/.config/draftpaper/config.toml"
    fi
    for scale in 1 1.25 1.5 1.75 2; do
        shot $scale "$t/sheet.png"
        "$check" "$t/sheet.png" $scale "$mm" 6 $([ $margins = ambient ] && echo ambient) | sed "s|$t/sheet.png|$margins margins, scale $scale|" || fail=1
    done
done
exit $fail
