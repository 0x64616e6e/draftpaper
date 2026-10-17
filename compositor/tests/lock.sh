#!/bin/sh
# make pixels (and alone): the lock screen (qml/lock.qml, `draftpaper lock`), headless: nothing is shown
# on any display. Its PAM service comes from a directory of the test's own (DRAFTPAPER_LOCK_PAM_DIR, with
# DRAFTPAPER_LOCK_TEST=1), shaped as common-auth with a fingerprint reader: pam_echo says to touch the
# reader, a stand-in reader (pam_exec, sufficient) waits 3 s for a file, then pam_exec asks the password
# (pencil). `draftpaper lock` returns once locked, with the reader listening; a password typed meanwhile
# is held, given when PAM asks and refused; the reader listens again, and again each time it gives up
# with nothing typed; the finger unlocks; the sheet is as it was before the lock, and blurred while
# locked. SHOT=FILE keeps a screenshot of the lock screen.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-lock.XXXXXX)
home=$t/home
mkdir -p "$home/.config/draftpaper" "$home/Pictures" "$t/pam"
printf 'theme = "blueprint"\nmargin = 6\n[apps]\nskip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]\n' > "$home/.config/draftpaper/config.toml"
# the reader: the finger on it is the file $t/finger, for 3 s; the password: pencil
printf '#!/bin/sh\ni=0\nwhile [ ! -e %s/finger ] && [ $i -lt 30 ]; do sleep 0.1; i=$((i + 1)); done\n[ -e %s/finger ]\n' "$t" "$t" > "$t/finger.sh"
printf '#!/bin/sh\n[ "$(tr -d "\\000")" = pencil ]\n' > "$t/password.sh"
chmod +x "$t/finger.sh" "$t/password.sh"
cat > "$t/pam/draftpaper-lock" <<EOF
auth required pam_echo.so Place your finger on the fingerprint reader
auth sufficient pam_exec.so quiet $t/finger.sh
auth required pam_exec.so quiet expose_authtok $t/password.sh
EOF
pids=
# the lock screen is daemonized (qs -d): stopped by its path, should a check fail before it unlocks
trap '[ -n "${wl:-}" ] && env -u DISPLAY WAYLAND_DISPLAY="$wl" qs kill -p "$repo/qml/lock.qml" > /dev/null 2>&1; kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }
# no spaces in these paths (mktemp under the cache): the environment is split on them
env_home="HOME=$home XDG_CONFIG_HOME=$home/.config XDG_CACHE_HOME=$home/.cache DRAFTPAPER_CACHE=$home/.cache/draftpaper DRAFTPAPER_ROOT=$repo DRAFTPAPER_BIN=$cli DRAFTPAPER_MM=6.2 DRAFTPAPER_LOCK_TEST=1 DRAFTPAPER_LOCK_PAM_DIR=$t/pam"
clean="-u WAYLAND_DISPLAY -u DISPLAY -u DRAFTPAPER_SOCKET"
env $clean $env_home "$cli" apply > /dev/null

# started with env itself, so $! is the process
env $clean $env_home "$comp" --headless 1920x1200 --scale 1.5 --command true > "$t/log" 2>&1 &
comp_pid=$!
pids=$comp_pid
i=0
until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
sock=$runtime/draftpaper-$wl.sock
i=0
until DRAFTPAPER_SOCKET=$sock "$comp" msg version > /dev/null 2>&1 || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
in_session() { env $clean $env_home WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" "$@"; }
ipc() { in_session qs ipc -p "$repo/qml/lock.qml" call lock "$@" 2>/dev/null; }
# the screen as drawn, to $1
shot() {
    rm -f "$home"/Pictures/*.png
    kill -USR1 $comp_pid
    i=0
    until ls "$home"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$home"/Pictures/*.png | head -1)" "$1"
}
# a state of the lock screen, waited for (up to N tenths of a second): what it was last
until_state() {
    i=0
    while s=$(ipc state); ! printf '%s' "$s" | grep -q "$1" && [ $i -lt "$2" ]; do sleep 0.1; i=$((i + 1)); done
    printf '%s' "$s"
}

sleep 0.5
shot "$t/before.png"
in_session "$cli" lock
ok "draftpaper lock: back once locked" "$?:$(ipc state | cut -d' ' -f1)" "0:locked"
ok "the reader listening" "$(until_state "finger" 20)" "locked finger"
in_session "$cli" lock
ok "locking again: back at once, still the one lock screen" "$?:$(ipc state)" "0:locked finger"
sleep 0.6
shot "$t/locked.png"
[ -n "${SHOT:-}" ] && cp "$t/locked.png" "$SHOT"
ok "locked: the sheet blurred, the card on it" "$(cmp -s "$t/before.png" "$t/locked.png" && echo same || echo different)" "different"
ipc type wrong
ipc enter
ok "a password typed meanwhile, held" "$(ipc state)" "locked finger password"
# the reader gives up (3 s), PAM asks for the password, the held one goes and is refused
ok "then given when PAM asks, and refused" "$(until_state "Not unlocked" 60)" "locked password: Not unlocked"
ok "the reader listening again" "$(until_state "finger" 20)" "locked finger password: Not unlocked"
# nothing typed: each time the reader gives up (3 s), it listens again, with no refusal
for n in 1 2; do
    sleep 3.4
    ok "the reader given up, listening again ($n)" "$(until_state "finger" 10)" "locked finger password: Not unlocked"
done
touch "$t/finger"
i=0
while ipc state | grep -q "^locked\|^unlocking" && [ $i -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
ok "the finger unlocks, the lock screen gone" "$(ipc state | grep -q "^locked\|^unlocking" && echo running || echo gone)" "gone"
sleep 0.6
shot "$t/after.png"
ok "unlocked: the sheet as it was" "$(cmp -s "$t/before.png" "$t/after.png" && echo same || echo different)" "same"
exit $fail
