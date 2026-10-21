#!/bin/sh
# make pixels (and alone): session restore (session.rs), headless (nothing on any display). Three
# terminals, each its own process in its own directory: one tiled and resized, one parked in its cell,
# one sent to workspace 2. Two are found by a desktop entry of the test's own, one by its command line
# alone. The session is written once it has held; the compositor ends as a logout ends it, and a new
# one starts each terminal again in its directory and puts every window back where it was. With
# [session] restore = false nothing is started.
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
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-session.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/data/applications" "$t/sys" "$t/one" "$t/two" "$t/three"
config() { printf 'theme = "softglow"\nmargin = 6\n%s\n[apps]\nskip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]\n' "$1" > "$t/.config/draftpaper/config.toml"; }
config ""
# the test's own entry (the system's are left out: XDG_DATA_DIRS)
printf '[Desktop Entry]\nType=Application\nName=Draft test\nExec=foot --app-id=drafttest\n' > "$t/data/applications/drafttest.desktop"
# a viewer of documents (its entry takes files: %f)
printf '[Desktop Entry]\nType=Application\nName=Draft viewer\nExec=foot --app-id=draftdoc less %%f\n' > "$t/data/applications/draftdoc.desktop"
seq 1 500 > "$t/three/notes.txt"; seq 1 50 > "$t/one/doc.txt"
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }
user_env="HOME=$t XDG_CONFIG_HOME=$t/.config XDG_STATE_HOME=$t/state XDG_DATA_HOME=$t/data XDG_DATA_DIRS=$t/sys SHELL=/bin/sh DRAFTPAPER_SESSION=1"

# a compositor; its pid in $comp_pid, its IPC in $sock
start() {
    : > "$t/log"
    env -u WAYLAND_DISPLAY -u DISPLAY $user_env DRAFTPAPER_MM=6.2 "$comp" --headless 1280x800 --command true > "$t/log" 2>&1 &
    comp_pid=$!
    pids="$pids $comp_pid"
    i=0
    until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
    wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
    [ -n "$wl" ] || { echo "the headless compositor did not start"; exit 1; }
    sock=$runtime/draftpaper-$wl.sock
    i=0
    until DRAFTPAPER_SOCKET=$sock "$comp" msg version > /dev/null 2>&1 || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
}
msg() { DRAFTPAPER_SOCKET=$sock "$comp" msg "$@" > /dev/null; sleep 0.3; }
# a terminal of app id $1 in directory $2
term() { a=$1 d=$2; shift 2; (cd "$d" && env -u DISPLAY $user_env WAYLAND_DISPLAY="$wl" foot --app-id="$a" "$@" > /dev/null 2>&1) & pids="$pids $!"; sleep 0.8; }
# the windows, one line each, as the session would put them back: app id, workspace, parked, cell,
# floating, geometry (on the workspace shown)
windows() {
    DRAFTPAPER_SOCKET=$sock "$comp" msg windows | tr -d ' ,"' | awk -F: '
        /^app_id/{a=$2} /^workspace/{w=$2} /^parked/{p=$2} /^cell/{c=$2} /^floating/{f=$2}
        /^geometry:null/{g="none"} /^geometry:\{/{g=""; in_g=1}
        in_g&&/^height/{h=$2} in_g&&/^width/{wd=$2} in_g&&/^x:/{x=$2} in_g&&/^y:/{g=x "," $2 "," wd "," h; in_g=0}
        /^id/{n++} /^xwayland/{print a, w, p, c, f, g}' | sort
}
# the number of windows in the session file
saved() { { tr -d ' \n' < "$t/state/draftpaper/session.json"; } 2>/dev/null | grep -o '"title":' | wc -l; }

start
term drafttest "$t/one"
term draftdoc "$t/one" less doc.txt
term drafttest "$t/three" sh -c "less notes.txt; exec sh"
msg park
term draftother "$t/two"
msg send 2
for i in 1 2; do msg resize left; done
before=$(windows)
echo "$before" | sed 's/^/    /'
ok "four windows: tiled, parked, on workspace 2" "$(echo "$before" | wc -l)" "4"

# written once it has held for a whole period (2 s), checked every period
i=0
while [ "$(saved)" != 4 ] && [ $i -lt 80 ]; do sleep 0.1; i=$((i + 1)); done
ok "the session written" "$(saved)" "4"
ok "with the shells' directories" "$(grep -c "\"dir\": \"$t/\(one\|two\|three\)\"" "$t/state/draftpaper/session.json")" "3"
ok "and only the user may read it" "$(stat -c %a "$t/state/draftpaper/session.json")" "600"

# the end of a session: the compositor goes, its windows with it
kill "$comp_pid"
sleep 1

start
i=0
while [ "$(windows | wc -l)" -lt 4 ] && [ $i -lt 100 ]; do sleep 0.1; i=$((i + 1)); done
sleep 0.5
after=$(windows)
ok "every window back where it was" "$after" "$before"
ok "the entry's terminals started in their directories" "$(grep -c "session: started drafttest (foot --app-id=drafttest.*) in $t/\(one\|three\)$" "$t/log")" "2"
ok "the terminal that ran less runs it again, then its shell" "$(grep -c "session: started drafttest (foot --app-id=drafttest sh -c \"\$@\"; exec \"\$0\" /[a-z/]*/\(sh\|dash\) less notes.txt) in $t/three$" "$t/log")" "1"
ok "the other terminal runs its shell alone" "$(grep -c "session: started drafttest (foot --app-id=drafttest) in $t/one$" "$t/log")" "1"
ok "the viewer opens its document again" "$(grep -c "session: started draftdoc (foot --app-id=draftdoc less $t/one/doc.txt) in " "$t/log")" "1"
ok "the other by its command line, in its directory" "$(grep -c "session: started draftother (foot --app-id=draftother) in $t/two$" "$t/log")" "1"
ok "the session file not written half back" "$(saved)" "4"
running() { n=0; for p in $(pgrep -x less); do [ "$(readlink /proc/$p/cwd)" = "$1" ] && n=$((n + 1)); done; echo $n; }
ok "less runs again in its terminal" "$(running "$t/three")" "1"
# the windows put back on a workspace not shown, or parked, were told to draw too: shown, they have
# drawn (what the client draws is as large as its tile)
drawn() {
    DRAFTPAPER_SOCKET=$sock "$comp" msg windows | tr -d ' ,"' | awk -F: -v app="$1" '
        /^app_id/{a=$2} /^geometry:\{/{g=1} /^surface:\{/{g=0; s=1} s&&/^width/{w=$2; s=0}
        /^xwayland/{if (a == app) print (w > 100 ? "drawn" : "blank " w)}' | sort | uniq -c | awk '{$1=$1; print}'
}
msg workspace 2
sleep 0.5
ok "the window put back on workspace 2 draws once shown" "$(drawn draftother)" "1 drawn"
msg workspace 1
# parked: the one shown goes to a cell too, then, nothing showing, both come back
msg park
msg park
sleep 0.5
ok "the parked one draws once back" "$(drawn drafttest)" "2 drawn"
kill "$comp_pid"
sleep 1

config "[session]
restore = false"
start
sleep 1.5
ok "restore = false: nothing started" "$(grep -c "session: started" "$t/log")" "0"
kill "$comp_pid"
exit $fail
