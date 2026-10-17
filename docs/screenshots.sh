#!/bin/sh
# make screenshots: the README's pictures (docs/*.jpg), drawn headless: nothing is shown on any display.
# A sheet of 1920x1200 at scale 1.5 (a laptop's panel), the shell of this checkout, terminals (foot)
# reading the repository's own files. Nothing of the machine's own shows: a home of the script's own,
# the title block's project and author set, a stand-in nmcli for the network (a made-up Wi-Fi) and
# bluetoothctl (nothing connected); the readings (time, battery, power, temperatures) are the machine's.
# Needs foot and ImageMagick (convert). OUT=DIR writes them elsewhere.
set -u
repo=$(cd "$(dirname "$0")/.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
fake=$repo/compositor/target/release/examples/fakegreetd
out=${OUT:-$repo/docs}
size=1920x1200 scale=1.5 mm=6.4
for c in foot convert; do command -v $c > /dev/null || { echo "screenshots: $c is needed"; exit 1; }; done
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-shots.XXXXXX)
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM

# stand-ins for what would show the machine's own: a made-up Wi-Fi, no Bluetooth
# (and, below, a few applications of the script's own for the launcher, the machine's left out)
mkdir -p "$t/bin"
printf '#!/bin/sh\ncase "$*" in *"wifi list"*) echo "*:72:drafting-room" ;; esac\n' > "$t/bin/nmcli"
printf '#!/bin/sh\nexit 0\n' > "$t/bin/bluetoothctl"
chmod +x "$t/bin/nmcli" "$t/bin/bluetoothctl"
mkdir -p "$t/share/applications"
entry() { printf '[Desktop Entry]\nType=Application\nName=%s\nGenericName=%s\nExec=true\n' "$2" "$3" > "$t/share/applications/$1.desktop"; }
entry foot "Terminal" "Use the command line"
entry firefox "Firefox" "Web Browser"
entry org.gnome.TextEditor "Text Editor" "Edit text files"
entry org.gnome.Nautilus "Files" "Access and organise files"
entry org.gnome.Calculator "Calculator" "Perform arithmetic"
entry org.gnome.Loupe "Image Viewer" "View images"
entry org.gnome.Evince "Document Viewer" "View PDF documents"
entry org.gnome.Settings "Settings" "The system's settings"
entry pavucontrol "Volume Control" "Adjust the volume"
entry thunderbird "Thunderbird" "Read and write mail"

# a home of the script's own, in theme $1; the Nerd Fonts of the user's, for the glyphs
home=$t/home
setup() {
    rm -rf "$home"
    mkdir -p "$home/.config/draftpaper" "$home/.local/share" "$home/Pictures" "$home/run"
    [ -d "$HOME/.local/share/fonts" ] && ln -s "$HOME/.local/share/fonts" "$home/.local/share/fonts"
    printf 'theme = "%s"\nmargin = 6\n[title]\nproject = "DRAFTPAPER"\ndrawn = "0x64616e6e"\n[apps]\nskip = ["gtk", "qt", "firefox", "x11", "emacs", "tmux", "neovim", "alacritty"]\n' "$1" \
        > "$home/.config/draftpaper/config.toml"
}
as_home() {
    env -u DISPLAY -u WAYLAND_DISPLAY -u DRAFTPAPER_SOCKET PATH="$t/bin:$PATH" HOME="$home" XDG_CONFIG_HOME="$home/.config" \
        XDG_DATA_HOME="$home/.local/share" XDG_DATA_DIRS="$t/share" XDG_CACHE_HOME="$home/.cache" DRAFTPAPER_CACHE="$home/.cache/draftpaper" \
        DRAFTPAPER_RUNTIME="$home/run" DRAFTPAPER_ROOT="$repo" DRAFTPAPER_BIN="$cli" DRAFTPAPER_MM=$mm "$@"
}
# the same in the background: exec, so $! is the program (a function in the background is a subshell)
spawn() { exec env -u DISPLAY -u WAYLAND_DISPLAY -u DRAFTPAPER_SOCKET PATH="$t/bin:$PATH" HOME="$home" XDG_CONFIG_HOME="$home/.config" \
        XDG_DATA_HOME="$home/.local/share" XDG_DATA_DIRS="$t/share" XDG_CACHE_HOME="$home/.cache" DRAFTPAPER_CACHE="$home/.cache/draftpaper" \
        DRAFTPAPER_RUNTIME="$home/run" DRAFTPAPER_ROOT="$repo" DRAFTPAPER_BIN="$cli" DRAFTPAPER_MM=$mm "$@"; }

# the compositor ($@: more of its arguments) and, unless a greeter, the shell, up and lettered
start() {
    : > "$t/log"
    spawn "$comp" --headless $size --scale $scale "$@" >> "$t/log" 2>&1 &
    nest=$!
    pids="$pids $nest"
    i=0
    until grep -aq "Created new socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
    wl=$(grep -a "Created new socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
    sock=$runtime/draftpaper-$wl.sock
    i=0
    until DRAFTPAPER_SOCKET=$sock "$comp" msg version > /dev/null 2>&1 || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
}
shell() {
    spawn WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" qs -p "$repo/qml/shell.qml" -n > "$t/qs.log" 2>&1 &
    pids="$pids $!"
    i=0
    until DRAFTPAPER_SOCKET=$sock "$comp" msg outputs 2>/dev/null | grep -q draftpaper-title-block || [ $i -gt 300 ]; do sleep 0.1; i=$((i + 1)); done
    # the readings in
    sleep 3
}
act() { DRAFTPAPER_SOCKET=$sock "$comp" msg "$@" > /dev/null; sleep 0.6; }
# a terminal titled $1 running $2 (in the repository)
term() {
    spawn WAYLAND_DISPLAY="$wl" foot -T "$1" -D "$repo" sh -c "$2" > /dev/null 2>&1 &
    pids="$pids $!"
    sleep 1.2
}
# the screen as drawn, to $out/$1.jpg
shot() {
    rm -f "$home"/Pictures/*.png
    kill -USR1 "$nest"
    i=0
    until ls "$home"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    convert "$(ls "$home"/Pictures/*.png | head -1)" -quality 90 "$out/$1.jpg" && echo "  $out/$1.jpg"
}
stop() { kill $pids 2>/dev/null; pids=; sleep 0.5; }

# Soft Glow: the whole sheet; three terminals tiled; one parked; the launcher
setup softglow
as_home "$cli" apply > /dev/null
start --command true
shell
three() {
    term "README.md" "less -R README.md"
    term "layout.rs" "less -N compositor/src/layout.rs"
    term "git log" "git --no-pager log --oneline -n 80 --no-color; exec sleep 600"
}
three
shot tiles
act park
shot parked
# the other two parked as well: the whole sheet; then (park with every window parked) all back
act park
act park
shot sheet
act park
as_home WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" qs ipc -p "$repo/qml/shell.qml" call launcher toggle > /dev/null
sleep 0.6
as_home WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" qs ipc -p "$repo/qml/shell.qml" call launcher type "" > /dev/null
sleep 0.8
shot launcher
stop

# Blueprint: the same three terminals
setup blueprint
as_home "$cli" apply > /dev/null
start --command true
shell
three
shot blueprint
stop

# the lock screen, on the Blueprint sheet: the reader listening (a PAM service of the script's own:
# the reader's prompt, then a minute's wait)
setup blueprint
as_home "$cli" apply > /dev/null
start --command true
mkdir -p "$t/pam"
printf 'auth required pam_echo.so Place your finger on the fingerprint reader\nauth required pam_exec.so quiet /bin/sleep 60\n' > "$t/pam/draftpaper-lock"
as_home WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" DRAFTPAPER_LOCK_TEST=1 DRAFTPAPER_LOCK_PAM_DIR="$t/pam" \
    "$cli" lock
sleep 1.5
shot lock
# the lock screen is daemonized (qs -d): not among pids
as_home WAYLAND_DISPLAY="$wl" qs kill -p "$repo/qml/lock.qml" > /dev/null 2>&1
stop

# the login screen, on the same sheet: the name filled in, the fingerprint reader listening
setup blueprint
as_home "$cli" apply > /dev/null
"$fake" "$t/greetd.sock" "$t/started" "$t/finger" 60 &
pids="$pids $!"
export GREETD_SOCK="$t/greetd.sock" DRAFTPAPER_GREETER_USER=0x64616e6e
start --greeter
sleep 4
shot greeter
stop
