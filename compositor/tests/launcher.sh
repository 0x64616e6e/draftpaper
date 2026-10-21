#!/bin/sh
# make pixels (and alone): the launcher and the menu (qml/Launcher.qml), headless (nothing on any display),
# driven over the shell's IPC as the keys drive it. A desktop entry of the test's own is found by its
# name, launched through the compositor (it touches a file) and counted; a menu over stdin's lines gives
# the line chosen (exit 0), and nothing when closed (exit 1). SHOT=FILE keeps a screenshot of the open
# launcher.
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
cli=$repo/cli/target/release/draftpaper
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-launcher.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/.local/share/applications" "$t/Pictures" "$t/run"
printf 'theme = "softglow"\nmargin = 6\n[apps]\nskip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]\n' > "$t/.config/draftpaper/config.toml"
printf '[Desktop Entry]\nType=Application\nName=Drafttest Probe\nGenericName=A test of the launcher\nExec=touch %s/launched %%U\n' "$t" > "$t/.local/share/applications/drafttest-probe.desktop"
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM

env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" DRAFTPAPER_MM=6.2 "$comp" --headless 3840x2160 --scale 1.5 --command true > "$t/log" 2>&1 &
pids=$!
i=0
until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
[ -n "$wl" ] || { echo "the headless compositor did not start"; exit 1; }
# the shell and the command as the session runs them, with this checkout's QML and a home of the test's own
run() {
    env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" XDG_DATA_HOME="$t/.local/share" WAYLAND_DISPLAY="$wl" \
        DRAFTPAPER_SOCKET="$runtime/draftpaper-$wl.sock" DRAFTPAPER_BIN="$cli" DRAFTPAPER_ROOT="$repo" \
        DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" "$@"
}
# the theme's files, as the session writes them before the shell starts
run "$cli" apply > /dev/null
run qs -p "$repo/qml/shell.qml" -n > "$t/qs.log" 2>&1 &
pids="$pids $!"
ipc() { run qs ipc -p "$repo/qml/shell.qml" call launcher "$@"; }
i=0
until ipc picked > /dev/null 2>&1 || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

run "$cli" launcher
sleep 0.5
ipc type "drafttest"
sleep 0.3
ok "found by its name" "$(ipc picked)" "Drafttest Probe"
ipc type "a test of the"
sleep 0.3
ok "and by its generic name" "$(ipc picked)" "Drafttest Probe"
if [ -n "${SHOT:-}" ]; then
    rm -f "$t"/Pictures/*.png
    kill -USR1 "${pids%% *}"
    sleep 1
    mv "$t"/Pictures/*.png "$SHOT"
fi
ipc launch
i=0
until [ -e "$t/launched" ] || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
ok "launched through the compositor" "$([ -e "$t/launched" ] && echo yes)" "yes"
ok "and closed" "$(ipc picked)" ""
sleep 0.5
ok "the launch counted" "$(tr -d ' \n' < "$t/.cache/draftpaper/launcher.json" 2>/dev/null | grep -o '"drafttest-probe":{"count":1')" '"drafttest-probe":{"count":1'

printf 'alpha\nbeta\ngamma\n' | run timeout 20 "$cli" menu --prompt "pick one" > "$t/choice" &
m=$!
sleep 0.8
ok "a menu over the lines, the first picked" "$(ipc picked)" "alpha"
# a menu that did not show would keep its command waiting (20 s at most: timeout)
if [ $fail = 1 ]; then kill $m; exit 1; fi
ipc type "gam"
sleep 0.3
ipc launch
wait $m
ok "the line chosen, on stdout (exit $?)" "$(cat "$t/choice")" "gamma"
printf 'alpha\n' | run timeout 20 "$cli" menu > "$t/choice" &
m=$!
sleep 0.8
ipc close
wait $m
ok "closed: nothing, exit 1" "$?:$(cat "$t/choice")" "1:"
exit $fail
