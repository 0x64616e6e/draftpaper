#!/bin/sh
# make pixels (and alone): the settings' pages of config.toml's rows (issue #36, qml/Settings.qml), headless
# (nothing on any display). Opened on the Sheet page: the margin stepped from 6 to 8 mm and the revision
# block turned on reach config.toml a moment later, its comments kept; the compositor takes the margin up
# (the frame moves in). Escape puts config.toml back as it was, byte for byte. Opened again on Idle · lock:
# lock after 5 minutes, kept with Enter, is in config.toml and in swayidle's file. The shell loads the
# pages without a QML warning. SHOT=FILE keeps a screenshot of the Sheet page.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-settings.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run" "$t/bin"
cfg=$t/.config/draftpaper/config.toml
cat > "$cfg" <<'TOML'
# the test's settings
theme = "softglow"
margin = 6                 # millimetres between the screen's edge and the frame
[apps]
skip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]
[session]
restore = false
apps = false
# ---- leaving the machine ----
[idle]
screens_off = 15           # minutes
# lock_after = 0           # minutes until the screen locks
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
ipc() { run qs ipc -p "$repo/qml/shell.qml" call settings "$@" 2> /dev/null; }
shot() {
    rm -f "$t"/Pictures/*.png
    kill -USR1 $nest
    i=0
    until ls "$t"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$t"/Pictures/*.png | head -n 1)" "$1"
}
# config.toml once it says $1 (the settings write a moment after the picking pauses)
waitfor() { i=0; until grep -q "$1" "$cfg" || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done; grep -c "$1" "$cfg"; }
# the frame's left edge, 2 mm under the top margin: dark at 6 mm, paper again once the margin is 8 mm
edge6=$(awk 'BEGIN { printf "%d", 6 * 6.4 + 1 }'); row=$(awk 'BEGIN { printf "%d", 60 * 6.4 }')

shot "$t/before.png"
ok "the frame at 6 mm" "$([ "$("$check" "$t/before.png" 1.5 6.4 6 lum $edge6 $row)" -lt 120 ] && echo dark)" "dark"
ipc toggle
sleep 1
ipc page sheet
sleep 0.3
ipc row 0
ipc step 1
ok "margin 8 written, its comment kept" "$(waitfor '^margin = 8                 # millimetres between')" "1"
ipc row 2
ipc step 1
ok "the revision block on, in a [title] of its own" "$(waitfor '^revisions = true')" "1"
ok "[title] added once" "$(grep -c '^\[title\]' "$cfg")" "1"
sleep 1
shot "$t/sheet.png"
[ -n "${SHOT:-}" ] && cp "$t/sheet.png" "$SHOT"
ok "the frame moved in: paper at 6 mm" "$([ "$("$check" "$t/sheet.png" 1.5 6.4 6 lum $edge6 $row)" -gt 120 ] && echo light)" "light"
ipc cancel
i=0
until cmp -s "$cfg" "$t/before.toml" || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
ok "escape: config.toml as it was" "$(cmp -s "$cfg" "$t/before.toml" && echo same)" "same"
sleep 0.5
ok "no mark left" "$(ls "$t/run" | grep -c config.mark)" "0"

ipc toggle
sleep 1
ipc page idle
sleep 0.3
ipc row 1
ipc step 1
ok "lock after 5 set in the commented example's place" "$(waitfor '^lock_after = 5             # minutes until')" "1"
ipc keep
sleep 1
ok "enter: kept" "$(grep -c '^lock_after = 5' "$cfg")" "1"
ok "no mark left" "$(ls "$t/run" | grep -c config.mark)" "0"
ok "swayidle locks after 300 s" "$(grep -c "^timeout 300 " "$t/.cache/draftpaper/swayidle.conf")" "1"
ok "toml still reads" "$(run "$cli" config json > /dev/null && echo yes)" "yes"
ok "no QML warning from the settings" "$(grep -c 'Settings.qml' "$t/qs.log")" "0"
[ "$fail" = 0 ] && echo "settings: ok" || { grep -a 'Settings.qml' "$t/qs.log" | head -5; echo "settings: FAILED"; }
exit $fail
