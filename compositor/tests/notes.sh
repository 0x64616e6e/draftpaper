#!/bin/sh
# make pixels (and alone): the notes on the sheet (issue #49: qml/MachineSchedule.qml, qml/LoadGraph.qml),
# headless (nothing on any display), on the T14's panel (1920x1200 at 1.5, 6.4 px/mm). On the empty
# workspace both are up (their layer surfaces), the schedule's outline 7 mm in from the frame's corner on
# whole pixels (ink there, paper a millimetre inside), the figure below it; a window opened takes both
# away, closed brings them back; [sheet] machine = false and load = false take them away.
# SHOT=FILE keeps a screenshot of the empty workspace.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-notes.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run" "$t/bin"
cfg=$t/.config/draftpaper/config.toml
cat > "$cfg" <<'TOML'
theme = "softglow"
margin = 6
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
layers() { run "$comp" msg outputs | jq -r '.[0].layers[]' | grep -c "draftpaper-machine\|draftpaper-load"; }
waitlayers() { i=0; until [ "$(layers)" = "$1" ] || [ $i -gt 80 ]; do sleep 0.1; i=$((i + 1)); done; layers; }
shot() {
    rm -f "$t"/Pictures/*.png
    kill -USR1 $nest
    i=0
    until ls "$t"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$t"/Pictures/*.png | head -n 1)" "$1"
}
# the first sample of the load comes 5 s in
ok "an empty workspace: the schedule and the load up" "$(waitlayers 2)" "2"
sleep 1
shot "$t/notes.png"
[ -n "${SHOT:-}" ] && { sleep "${SHOT_AFTER:-0}"; shot "$t/notes.png"; cp "$t/notes.png" "$SHOT"; }
# the frame's corner: the margin (6 mm) and its 2-pixel line; the schedule's outline 7 mm further in, a
# column of ink; a millimetre inside it, the paper
fx=$(awk 'BEGIN { printf "%d", 6 * 6.4 + 2 }'); edge=$(awk -v f="$fx" 'BEGIN { printf "%d", f + 7 * 6.4 }')
lum() { "$check" "$t/notes.png" 1.5 6.4 6 lum "$1" "$2"; }
darkest() { for x in $(($1 - 3)) $(($1 - 2)) $(($1 - 1)) $1 $(($1 + 1)) $(($1 + 2)) $(($1 + 3)); do lum $x "$2"; done | sort -n | head -n 1; }
row=$(awk -v f="$fx" 'BEGIN { printf "%d", f + 20 * 6.4 }')
ok "the schedule's outline, 7 mm in" "$([ "$(darkest "$edge" "$row")" -lt 110 ] && echo ink)" "ink"
ok "the paper a millimetre inside" "$([ "$(lum $((edge + 7)) "$row")" -gt 130 ] && echo paper)" "paper"
command -v foot > /dev/null && {
    run foot sh > /dev/null 2>&1 &
    pids="$pids $!"
    ok "a window opened: both away" "$(waitlayers 0)" "0"
    kill "$(run "$comp" msg windows | jq -r ".[0].pid")" 2>/dev/null
    ok "closed: both back" "$(waitlayers 2)" "2"
}
run "$cli" config set sheet.machine=false sheet.load=false > /dev/null
ok "[sheet] machine = false, load = false: both away" "$(waitlayers 0)" "0"
ok "no QML warning from the notes" "$(grep -c 'MachineSchedule.qml\|LoadGraph.qml' "$t/qs.log")" "0"
[ "$fail" = 0 ] && echo "notes: ok" || { grep -a 'MachineSchedule.qml\|LoadGraph.qml' "$t/qs.log" | head -5; echo "notes: FAILED"; }
exit $fail
