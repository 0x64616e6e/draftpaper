#!/bin/sh
# make pixels (and alone): an idle desktop costs nearly nothing, headless (nothing on any display), with
# two outputs of different scales and the shell. Once settled, over 5 s: each output draws only the frames
# something changed (the shell's clock and readings: a few, not one every refresh), and the compositor
# takes under 1.5% of a core and is woken under 20 times a second: it looked again every refresh,
# forever (5% idle, the GPU woken 60 times a second); now only when something may have changed.
# Something changing is still drawn at once: a workspace switch, a frame within 0.3 s.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-idle.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/run" "$t/bin"
cat > "$t/.config/draftpaper/config.toml" <<'TOML'
theme = "softglow"
margin = 6
[apps]
skip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]
[session]
restore = false
apps = false
TOML
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
# settled: the opening over, the shell's first readings in
sleep 6

frames() { run "$comp" msg frames | jq -r ".[\"$1\"] // 0"; }
ticks() { for th in /proc/$nest/task/*; do cut -d' ' -f14,15 "$th/stat"; done | awk '{s += $1 + $2} END {print s}'; }
wakeups() { for th in /proc/$nest/task/*; do awk '/ctxt_switches/ { s += $2 } END { print s }' "$th/status"; done | awk '{ s += $1 } END { print s }'; }
a0=$(frames headless); b0=$(frames headless-2); c0=$(ticks); w0=$(wakeups)
sleep 5
a1=$(frames headless); b1=$(frames headless-2); c1=$(ticks); w1=$(wakeups)
hz=$(getconf CLK_TCK)
few() { [ $(($2 - $1)) -le 15 ] && echo few || echo "$(($2 - $1)) in 5 s"; }
ok "idle: the panel draws only what changed" "$(few "$a0" "$a1")" "few"
ok "idle: the 4K output too" "$(few "$b0" "$b1")" "few"
ok "idle: the compositor under 1.5% of a core" "$(awk -v c=$((c1 - c0)) -v hz="$hz" 'BEGIN { p = 100 * c / hz / 5; print (p < 1.5 ? "under" : sprintf("%.1f%%", p)) }')" "under"
ok "idle: the compositor woken under 20 times a second (not every refresh)" "$(awk -v w=$((w1 - w0)) 'BEGIN { r = w / 5; print (r < 20 ? "under" : sprintf("%.0f/s", r)) }')" "under"

# a change is drawn at once
f0=$(frames headless)
run "$comp" msg workspace 2 > /dev/null
sleep 0.3
f1=$(frames headless)
ok "a workspace switch: drawn within 0.3 s" "$([ "$f1" -gt "$f0" ] && echo drawn || echo "not drawn")" "drawn"
ok "and on workspace 2" "$(run "$comp" msg workspaces | jq -r '[.[] | select(.focused)][0].number // .[0].number')" "2"

[ "$fail" = 0 ] && echo "idle: ok" || echo "idle: FAILED"
exit $fail
