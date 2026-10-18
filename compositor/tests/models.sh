#!/bin/sh
# make pixels (and alone): a display's settings follow it (theme.rs's output_settings), headless, with two:
# the T14's panel (its model a code, 0x1488) and an ASUS MB166C on the second port. config.toml's
# [outputs."ASUS MB166C"] scale = 2 reaches the Asus, and [outputs.headless-2], the port's, gives way to
# it. The settings' Displays page writes the Asus's scale into its own table, by model, and the panel's
# under its connector.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-models.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run" "$t/bin"
cfg=$t/.config/draftpaper/config.toml
cat > "$cfg" <<'TOML'
theme = "softglow"
[apps]
theme = false
[session]
restore = false
apps = false

[outputs.headless-2]       # the port's
scale = 1.25

[outputs."ASUS MB166C"]    # the Asus's, wherever it is plugged in
scale = 2
TOML
cp "$cfg" "$t/before.toml"
# idle's service is the user's: left alone
printf '#!/bin/sh\nexit 0\n' > "$t/bin/systemctl"
chmod +x "$t/bin/systemctl"
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" DRAFTPAPER_MM=6.4,5.6 DRAFTPAPER_MODELS="0x1488,ASUS MB166C" "$comp" --headless 1920x1200,1920x1080 --scale 1.5 --command true > "$t/log" 2>&1 &
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

# the compositor reads config.toml again within 2 s of a change: once now, the plugged-in state set
sleep 1; touch "$cfg"
ok "the Asus at its own scale, 2, not the port's" "$(waitgeom headless-2 "1280 0 960 540 2.0")" "1280 0 960 540 2.0"
ipc toggle
sleep 2
ipc page displays
sleep 0.5
ipc row 1
ipc step -1
ok "the page writes the Asus's scale into its table, by model" "$(i=0; until sed -n '/^\[outputs."ASUS MB166C"\]/,/^\[/p' "$cfg" | grep -q '^scale = 1.75' || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done; sed -n '/^\[outputs."ASUS MB166C"\]/,/^\[/p' "$cfg" | grep -c '^scale = 1.75')" "1"
ok "the port's table as it was" "$(sed -n '/^\[outputs.headless-2\]/,/^$/p' "$cfg" | grep -c '^scale = 1.25')" "1"
ok "the panel under its connector" "$(grep -c '^\[outputs.headless\]' "$cfg")" "1"
ok "no table named for the panel's code" "$(grep -c '0x1488' "$cfg")" "0"
ok "config.toml reads" "$(run "$cli" config json > /dev/null && echo yes)" "yes"
ipc keep
[ "$fail" = 0 ] && echo "models: ok" || echo "models: FAILED"
exit $fail
