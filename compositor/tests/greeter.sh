#!/bin/sh
# make pixels (and alone): the login screen (--greeter, qml/greeter.qml) against a stand-in for greetd
# (examples/fakegreetd.rs), headless: nothing is shown on any display and no VT is touched.
#
# A user's home with a theme of their own, a margin, ambient margins and (when the system has a PNG in
# /usr/share/pixmaps) a wallpaper; `draftpaper apply` copies its look for the login screen (cli's
# greeter.rs). The greeter runs on that copy as draftpaper-greeter runs it, the name filled in, and is
# driven over its test IPC as the keys drive it: a wrong password (refused), the right one; then greetd
# is asked to start draftpaper-session, the card goes, the compositor exits, and its last frame must be
# the first frame of a compositor on the user's own home, pixel for pixel. SHOT=FILE keeps a
# screenshot of the card.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
fake=$repo/compositor/target/release/examples/fakegreetd
# no spaces in these paths (mktemp under the cache): the environments below are split on them
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-greeter.XXXXXX)
home=$t/home
mkdir -p "$home/.config/draftpaper/themes" "$home/Pictures" "$t/greeter" "$t/gcache"
sed 's/^name *=.*/name = "drafttest"/' "$repo/themes/blueprint.toml" > "$home/.config/draftpaper/themes/drafttest.toml"
wp=$(ls /usr/share/pixmaps/debian-logo.png /usr/share/pixmaps/*.png 2>/dev/null | head -n 1)
{
    printf 'theme = "drafttest"\nmargin = 7\nmargins = "ambient"\n'
    [ -n "$wp" ] && printf 'wallpaper = "%s"\n' "$wp"
    printf '[apps]\nskip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]\n'
} > "$home/.config/draftpaper/config.toml"
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }
clean="-u WAYLAND_DISPLAY -u DISPLAY -u DRAFTPAPER_SOCKET"
common="DRAFTPAPER_ROOT=$repo DRAFTPAPER_BIN=$cli DRAFTPAPER_MM=6.2 DRAFTPAPER_GREETER_DIR=$t/greeter"
# the user, in their home
user_env="HOME=$home USER=drafttest XDG_CONFIG_HOME=$home/.config XDG_CACHE_HOME=$home/.cache DRAFTPAPER_CACHE=$home/.cache/draftpaper"

env $clean $common $user_env "$cli" apply > /dev/null
look=$t/greeter/drafttest
ok "the look copied for the login screen" "$(grep -c '^theme = "drafttest"\|^margin = 7\|^margins = "ambient"' "$look/.config/draftpaper/config.toml" 2>/dev/null)" "3"
ok "and only the look" "$(grep -c '^\[apps\]' "$look/.config/draftpaper/config.toml" 2>/dev/null)" "0"
ok "the user's own theme with it" "$([ -f "$look/.config/draftpaper/themes/drafttest.toml" ] && echo yes)" "yes"
if [ -n "$wp" ]; then
    ok "the wallpaper with it" "$(cmp -s "$wp" "$look/wallpaper.png" && grep -c '^wallpaper = "~/wallpaper.png"' "$look/.config/draftpaper/config.toml")" "1"
else
    echo "  (no PNG in /usr/share/pixmaps: no wallpaper)"
fi

# the login screen, as draftpaper-greeter starts it
greeter_env="HOME=$look XDG_CONFIG_HOME=$look/.config DRAFTPAPER_CACHE=$look/.cache/draftpaper XDG_CACHE_HOME=$t/gcache DRAFTPAPER_GREETER_USER=drafttest"
"$fake" "$t/greetd.sock" "$t/started" &
pids=$!
# started with env itself, so $! is the process
env $clean $common $greeter_env GREETD_SOCK="$t/greetd.sock" DRAFTPAPER_GREETER_TEST=1 DRAFTPAPER_LAST_FRAME="$t" \
    "$comp" --headless 3840x2160 --scale 1.5 --greeter > "$t/log" 2>&1 &
greeter=$!
pids="$pids $greeter"
i=0
until grep -aq "Created new socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
wl=$(grep -a "Created new socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
[ -n "$wl" ] || { echo "the headless compositor did not start"; exit 1; }
ipc() { env $clean $common $greeter_env WAYLAND_DISPLAY="$wl" qs ipc -p "$repo/qml/greeter.qml" call greeter "$@"; }
i=0
until ipc state > /dev/null 2>&1 || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
ok "the name filled in, the card asks for the password" "$(ipc state)" "password"
ipc type wrong
ipc enter
sleep 0.5
ok "a wrong password refused" "$(ipc state)" "password: Authentication failure"
if [ -n "${SHOT:-}" ]; then
    mkdir -p "$look/Pictures"
    kill -USR1 "$greeter"
    sleep 1
    mv "$look"/Pictures/*.png "$SHOT"
fi
ipc type pencil
ipc enter
# the card fades (a quarter second), the session is asked for, Quickshell and the compositor quit
i=0
while kill -0 "$greeter" 2>/dev/null && [ $i -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
ok "the compositor gone with the card" "$(kill -0 "$greeter" 2>/dev/null && echo running || echo gone)" "gone"
ok "greetd asked for the session" "$(tr -d ' \n' < "$t/started" 2>/dev/null)" '{"cmd":["draftpaper-session"],"env":[],"user":"drafttest"}'
last=$(ls "$t"/last-*.png 2>/dev/null | head -n 1)
ok "its last frame written" "$([ -n "$last" ] && echo yes)" "yes"

# the session's first frame: a compositor on the user's own home, nothing started yet
env $clean $common $user_env "$comp" --headless 3840x2160 --scale 1.5 --command true > "$t/log2" 2>&1 &
session=$!
pids="$pids $session"
i=0
until grep -aq "Created new socket" "$t/log2" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
wl2=$(grep -a "Created new socket" "$t/log2" | grep -o "wayland-[0-9]*" | head -1)
# up (its IPC answers), and its signals with it
i=0
until DRAFTPAPER_SOCKET="${XDG_RUNTIME_DIR:?}/draftpaper-$wl2.sock" "$comp" msg version > /dev/null 2>&1 || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
sleep 0.5
kill -USR1 "$session"
i=0
until ls "$home"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
sleep 0.3
first=$(ls "$home"/Pictures/*.png 2>/dev/null | head -n 1)
ok "the greeter's last frame is the session's first" "$([ -n "$last" ] && [ -n "$first" ] && cmp -s "$last" "$first" && echo same || echo different)" "same"
exit $fail
