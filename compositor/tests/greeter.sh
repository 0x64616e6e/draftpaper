#!/bin/sh
# make pixels (and alone): the login screen (--greeter, qml/greeter.qml) against a stand-in for greetd
# (examples/fakegreetd.rs), headless: nothing is shown on any display and no VT is touched.
#
# A user's home with a theme of their own, a margin, ambient margins and (when the system has a PNG in
# /usr/share/pixmaps) a wallpaper; `draftpaper apply` copies its look for the login screen (cli's
# greeter.rs). The greeter runs on that copy as draftpaper-greeter runs it, the name filled in, and is
# driven over its test IPC as the keys drive it, the stand-in's fingerprint reader listening first (as
# pam_fprintd): a password typed meanwhile is held, given when PAM asks and refused; the reader listens
# again each time it gives up with nothing typed; Escape meanwhile goes back to the name once it does;
# then the finger signs in, greetd is asked to start draftpaper-session, and the compositor leaves its
# last frame (the sheet blurred, the name in the title block). A compositor on the user's own home opens on that very
# picture and reveals the desktop as it is without one, pixel for pixel. SHOT=FILE keeps a screenshot
# of the card.
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
fake=$repo/compositor/target/release/examples/fakegreetd
# no spaces in these paths (mktemp under the cache): the environments below are split on them
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-greeter.XXXXXX)
home=$t/home
mkdir -p "$home/.config/draftpaper/themes" "$home/Pictures" "$t/greeter" "$t/gcache"
sed 's/^name *=.*/name = "drafttest"/' "$repo/themes/blueprint.toml" > "$home/.config/draftpaper/themes/drafttest.toml"
wp=
for f in /usr/share/pixmaps/debian-logo.png /usr/share/pixmaps/*.png; do [ -f "$f" ] && { wp=$f; break; }; done
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

# a state of the card, waited for (up to N tenths of a second): what it was last
until_state() {
    i=0
    while s=$(ipc state 2>/dev/null); ! printf '%s' "$s" | grep -q "$1" && [ $i -lt "$2" ]; do sleep 0.1; i=$((i + 1)); done
    printf '%s' "$s"
}

# the sessions installed (wayland-sessions): Draftpaper's, another desktop's, and two that are not
# offered -- one whose program is not installed (TryExec), one that asks not to be shown
s=$t/share/wayland-sessions
mkdir -p "$s" "$t/state"
printf '[Desktop Entry]\nName=Draftpaper (Wayland)\nExec=draftpaper-session\nType=Application\n' > "$s/draftpaper.desktop"
printf "[Desktop Entry]\nName=Tom's WM\nExec=twm-session\nTryExec=sh\nType=Application\n" > "$s/twm.desktop"
printf '[Desktop Entry]\nName=Gone\nExec=gone\nTryExec=no-such-program-anywhere\nType=Application\n' > "$s/gone.desktop"
printf '[Desktop Entry]\nName=Hidden\nExec=hidden\nNoDisplay=true\nType=Application\n' > "$s/hidden.desktop"

# the login screen, as draftpaper-greeter starts it; the reader takes a finger for 3 s
mkdir -p "$t/kept"
greeter_env="HOME=$look XDG_CONFIG_HOME=$look/.config DRAFTPAPER_CACHE=$look/.cache/draftpaper XDG_CACHE_HOME=$t/gcache XDG_STATE_HOME=$t/state DRAFTPAPER_GREETER_STATE=$t/kept XDG_DATA_DIRS=$t/share DRAFTPAPER_GREETER_USER=drafttest"
"$fake" "$t/greetd.sock" "$t/started" "$t/finger" 3 &
pids=$!
# started with env itself, so $! is the process
env $clean $common $greeter_env GREETD_SOCK="$t/greetd.sock" DRAFTPAPER_GREETER_TEST=1 DRAFTPAPER_LAST_FRAME="$t" \
    "$comp" --headless 3840x2160 --scale 1.5 --greeter > "$t/log" 2>&1 &
greeter=$!
pids="$pids $greeter"
i=0
until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
[ -n "$wl" ] || { echo "the headless compositor did not start"; exit 1; }
ipc() { env $clean $common $greeter_env WAYLAND_DISPLAY="$wl" qs ipc -p "$repo/qml/greeter.qml" call greeter "$@"; }
i=0
until ipc state > /dev/null 2>&1 || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
ok "the name filled in, the reader listening at once" "$(until_state "^password finger$" 20)" "password finger"
i=0
until [ -n "$(ipc sessions 2>/dev/null)" ] || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
ok "the sessions offered, Draftpaper's first" "$(ipc sessions)" "[Draftpaper (Wayland)] Tom's WM"
ipc type wrong
ipc enter
ok "a password typed meanwhile, held" "$(ipc state)" "password busy finger"
# the reader gives up (3 s), PAM asks for the password, the held one goes and is refused
ok "then given when PAM asks, and refused" "$(until_state "Authentication failure" 60)" "password: Authentication failure"
ok "the reader listening again" "$(until_state "finger" 20)" "password finger: Authentication failure"
# nothing typed: each time the reader gives up (3 s), it listens again, with no refusal
for n in 1 2; do
    sleep 3.4
    ok "the reader given up, listening again ($n)" "$(until_state "finger" 10)" "password finger: Authentication failure"
done
# Escape while the reader listens sends greetd nothing (it answers no request until PAM is back): the
# replies would come out of order and the sign-in be lost. It is done when the reader gives up
ipc back
ok "Escape while the reader listens: nothing sent, the password still" "$(ipc state)" "password finger: Authentication failure"
ok "the reader given up: the name, as Escape asked" "$(until_state "^name" 45)" "name"
ipc type drafttest
ipc enter
ok "the name again, the reader listening" "$(until_state "^password finger" 20)" "password finger"
if [ -n "${SHOT:-}" ]; then
    mkdir -p "$look/Pictures"
    kill -USR1 "$greeter"
    sleep 1
    mv "$look"/Pictures/*.png "$SHOT"
fi
# Tab: the other desktop, for this sign-in
ipc nextSession
ok "Tab: the next session" "$(ipc sessions)" "Draftpaper (Wayland) [Tom's WM]"
touch "$t/finger"
# the finger taken: the name slides into the title block (under half a second), the session is asked
# for, and the compositor leaves
i=0
while kill -0 "$greeter" 2>/dev/null && [ $i -lt 60 ]; do sleep 0.1; i=$((i + 1)); done
ok "signed in by the finger, the compositor gone" "$(kill -0 "$greeter" 2>/dev/null && echo running || echo gone)" "gone"
ok "greetd asked for the session chosen" "$(tr -d ' \n' < "$t/started" 2>/dev/null)" '{"cmd":["twm-session"],"env":[],"user":"drafttest"}'
ok "and the choice kept for the name" "$(cat "$t/kept/draftpaper-greeter-sessions.json" 2>/dev/null)" '{"drafttest":"twm"}'
# the next login screen (after a restart: its runtime state gone, the kept state there) comes up on it
rm -rf "$t/state" && mkdir -p "$t/state"
"$fake" "$t/greetd2.sock" "$t/started2" "$t/finger2" 3 &
pids="$pids $!"
env $clean $common $greeter_env GREETD_SOCK="$t/greetd2.sock" DRAFTPAPER_GREETER_TEST=1 \
    "$comp" --headless 3840x2160 --scale 1.5 --greeter > "$t/log3" 2>&1 &
again=$!
pids="$pids $again"
i=0
until grep -aq "draftpaper: wayland socket" "$t/log3" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
wl3=$(grep -a "draftpaper: wayland socket" "$t/log3" | grep -o "wayland-[0-9]*" | head -1)
ipc3() { env $clean $common $greeter_env WAYLAND_DISPLAY="$wl3" qs ipc -p "$repo/qml/greeter.qml" call greeter "$@"; }
i=0
until ipc3 state > /dev/null 2>&1 || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
i=0
until ipc3 sessions 2>/dev/null | grep -q '\[' || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
ok "after a restart, the login screen comes up on the session chosen last" "$(ipc3 sessions)" "Draftpaper (Wayland) [Tom's WM]"
kill "$again" 2>/dev/null
wait "$again" 2>/dev/null
last=$(ls "$t"/last-*.png 2>/dev/null | head -n 1)
ok "its last frame written" "$([ -n "$last" ] && echo yes)" "yes"

# a compositor on the user's own home, nothing started; $1: more of its environment. Waits until it is
# up (its IPC answers, its signals with it); its pid in $session
start_session() {
    env $clean $common $user_env "$@" "$comp" --headless 3840x2160 --scale 1.5 --command true > "$t/log2" 2>&1 &
    session=$!
    pids="$pids $session"
    i=0
    until grep -aq "draftpaper: wayland socket" "$t/log2" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
    wl2=$(grep -a "draftpaper: wayland socket" "$t/log2" | grep -o "wayland-[0-9]*" | head -1)
    i=0
    until DRAFTPAPER_SOCKET="${XDG_RUNTIME_DIR:?}/draftpaper-$wl2.sock" "$comp" msg version > /dev/null 2>&1 || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
}
# a screenshot of $session to the file $1
shot() {
    rm -f "$home"/Pictures/*.png
    kill -USR1 "$session"
    i=0
    until ls "$home"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$home"/Pictures/*.png | head -n 1)" "$1" 2>/dev/null
}
same() { [ -s "$1" ] && [ -s "$2" ] && cmp -s "$1" "$2" && echo same || echo different; }

# the desktop as it is, without an opening
start_session
sleep 0.3
shot "$t/plain.png"
kill "$session"
ok "the login screen's last frame blurred, with the name: not the plain sheet" "$(same "$last" "$t/plain.png")" "different"

# the session, taking over that frame: the very picture first, the desktop once revealed (after 4 s,
# no shell coming here, then 0.7 s)
start_session DRAFTPAPER_OPENING="$last" DRAFTPAPER_OPENING_WAIT=4
shot "$t/opening.png"
ok "the session opens on the login screen's last frame" "$(same "$last" "$t/opening.png")" "same"
sleep 5
shot "$t/revealed.png"
ok "and reveals the desktop" "$(same "$t/plain.png" "$t/revealed.png")" "same"
kill "$session"

# a black picture (the screen dark, or the picture not come through: on Intel the previous frame read
# back can be) is no opening
printf '%s' iVBORw0KGgoAAAANSUhEUgAAAAEAAAABAQAAAAA3bvkkAAAACklEQVQI12NgAAAAAgAB4iG8MwAAAABJRU5ErkJggg== | base64 -d > "$t/black.png"
: > "$t/log2"
start_session DRAFTPAPER_OPENING="$t/black.png"
ok "a black picture is no opening" "$(grep -ac "the picture is black: no opening" "$t/log2")" "1"
exit $fail
