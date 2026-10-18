#!/bin/sh
# make pixels (and alone): the settings' Displays page (issue #36, qml/Settings.qml), headless (nothing on any
# display), with two: the T14's panel (1920x1200, 6.4 px/mm) and a 4K display (3840x2160, 6.2 px/mm) right of
# it, both at 1.5. The second's scale picked at 1.75 reaches config.toml with every display's position, and
# the compositor takes it up (2195 x 1235 logical, rounded up as the compositor does, right of the panel's 1280); put left of the panel, the
# panel moves right of it. Escape puts config.toml back as it was and both displays as plugged in (the
# compositor puts a display back when its keys go). SHOT=FILE keeps a screenshot of the page.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-displays.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run" "$t/bin"
cfg=$t/.config/draftpaper/config.toml
cat > "$cfg" <<'TOML'
theme = "softglow"
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

env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" DRAFTPAPER_MM=6.4,6.2 "$comp" --headless 1920x1200,3840x2160 --scale 1.5 --command true > "$t/log" 2>&1 &
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
ipc() { run qs ipc -p "$repo/qml/shell.qml" call settings "$@" 2> /dev/null; }
shot() {
    rm -f "$t"/Pictures/*.png
    kill -USR1 $nest
    i=0
    until ls "$t"/Pictures/*-headless.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$t"/Pictures/*-headless.png | head -n 1)" "$1"  # the panel's, where the settings are
}
waitfor() { i=0; until grep -q "$1" "$cfg" || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done; grep -c "$1" "$cfg"; }
# NAME's x, y, width, height and scale as the compositor has them
geom() { run "$comp" msg outputs | jq -r --arg n "$1" '.[] | select(.name == $n) | "\(.x) \(.y) \(.width) \(.height) \(.scale)"'; }
# once NAME has $2 (the compositor reads config.toml a moment after it is written)
waitgeom() { i=0; until [ "$(geom "$1")" = "$2" ] || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done; geom "$1"; }

ok "as plugged in: the panel" "$(geom headless)" "0 0 1280 800 1.5"
ok "as plugged in: the 4K display right of it" "$(geom headless-2)" "1280 0 2560 1440 1.5"
ipc toggle
sleep 1
ipc page displays
sleep 0.3
ipc row 1
ipc step 1
ok "its scale 1.75 written" "$(waitfor '^scale = 1.75$')" "1"
ok "and every display's position" "$(grep -c '^position = ' "$cfg")" "2"
ok "the compositor: the 4K display at 1.75" "$(waitgeom headless-2 "1280 0 2195 1235 1.75")" "1280 0 2195 1235 1.75"
ipc row 2
ipc step 1
ok "left of the panel" "$(waitgeom headless-2 "0 0 2195 1235 1.75")" "0 0 2195 1235 1.75"
ok "the panel right of it" "$(waitgeom headless "2195 0 1280 800 1.5")" "2195 0 1280 800 1.5"
sleep 0.5
shot "$t/displays.png"
[ -n "${SHOT:-}" ] && cp "$t/displays.png" "$SHOT"
ipc cancel
i=0
until cmp -s "$cfg" "$t/before.toml" || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
ok "escape: config.toml as it was" "$(cmp -s "$cfg" "$t/before.toml" && echo same)" "same"
ok "escape: the panel as plugged in" "$(waitgeom headless "0 0 1280 800 1.5")" "0 0 1280 800 1.5"
ok "escape: the 4K display as plugged in" "$(waitgeom headless-2 "1280 0 2560 1440 1.5")" "1280 0 2560 1440 1.5"
ok "no QML warning from the settings" "$(grep -c 'Settings.qml' "$t/qs.log")" "0"
[ "$fail" = 0 ] && echo "displays: ok" || { grep -a 'Settings.qml' "$t/qs.log" | head -5; echo "displays: FAILED"; }
exit $fail
