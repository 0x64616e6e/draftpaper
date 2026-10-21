#!/bin/sh
# make pixels (and alone): the settings' Text page (issue #36, qml/Settings.qml, cli/src/text.rs), headless
# (nothing on any display), gsettings stubbed (its gets fixed, its sets logged: the user's own are never
# touched), Qt's and Alacritty's files in the test's home. The system has Cantarell 11 and DejaVu Sans Mono
# 10. The interface size picked at 12 reaches config.toml's [text], GTK (gsettings) and Qt (qt6ct.conf, the
# rest of its line kept); the monospace size at 9 reaches Alacritty's draftpaper.toml. Escape puts
# config.toml back, and GTK, Qt and Alacritty as the system had them. SHOT=FILE keeps a screenshot.
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
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-text.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run" "$t/bin"
cfg=$t/.config/draftpaper/config.toml
cat > "$cfg" <<'TOML'
theme = "softglow"
[apps]
skip = ["firefox", "x11", "emacs", "foot", "tmux", "neovim"]
[session]
restore = false
apps = false
TOML
cp "$cfg" "$t/before.toml"
# idle's service is the user's: left alone
printf '#!/bin/sh\nexit 0\n' > "$t/bin/systemctl"
chmod +x "$t/bin/systemctl"
# gsettings: the system's fonts on get, each set logged
cat > "$t/bin/gsettings" <<'SH'
#!/bin/sh
# what it was last set to, else the system's; each set logged
case "$1 $3" in
    "get font-name") [ -f "$HOME/gs-font-name" ] && echo "'$(cat "$HOME/gs-font-name")'" || echo "'Cantarell 11'" ;;
    "get monospace-font-name") [ -f "$HOME/gs-monospace-font-name" ] && echo "'$(cat "$HOME/gs-monospace-font-name")'" || echo "'DejaVu Sans Mono 10'" ;;
    set\ *) echo "$3=$4" >> "$HOME/gsettings.log"; printf "%s" "$4" > "$HOME/gs-$3" ;;
esac
exit 0
SH
chmod +x "$t/bin/gsettings"
mkdir -p "$t/.config/qt6ct" "$t/.config/alacritty"
printf '[Appearance]\nstyle=Fusion\n\n[Fonts]\nfixed="DejaVu Sans Mono,10,-1,5,400,0,0,0,0,0,0,0,0,0,0,1,Book"\ngeneral="Cantarell,11,-1,5,400,0,0,0,0,0,0,0,0,0,0,1,Regular"\n' > "$t/.config/qt6ct/qt6ct.conf"

printf '[general]\nimport = ["~/.config/alacritty/draftpaper.toml"]\n' > "$t/.config/alacritty/alacritty.toml"
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
waitfor() { i=0; until grep -q "$1" "$2" 2>/dev/null || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done; grep -c "$1" "$2" 2>/dev/null || echo 0; }
qtline() { grep "^$1=" "$t/.config/qt6ct/qt6ct.conf"; }

ok "config json: the system's text where config.toml sets none" "$(run "$cli" config json | jq -c '.text')" '{"interface":"Cantarell","size":11.0,"monospace":"DejaVu Sans Mono","mono_size":10.0,"min_degrees":0.2}'
ipc toggle
# the settings load the config (config json) as they open: time for that on a loaded machine
sleep 2.5
ipc page text
sleep 1
ipc row 2
ipc step 1
ok "interface size 12 in config.toml's [text]" "$(waitfor '^size = 12$' "$cfg")" "1"
ok "GTK: Cantarell 12" "$(waitfor '^font-name=Cantarell 12$' "$t/gsettings.log")" "1"
ok "Qt: Cantarell 12, the rest of its line kept" "$(qtline general)" 'general="Cantarell,12,-1,5,400,0,0,0,0,0,0,0,0,0,0,1,Regular"'
ipc row 3
ipc step -1
ok "monospace size 9 in Alacritty's draftpaper.toml" "$(waitfor '^size = 9.0$' "$t/.config/alacritty/draftpaper.toml")" "1"
ok "what the system had, kept" "$([ -s "$t/.local/state/draftpaper/text.json" ] && echo kept)" "kept"
sleep 0.5
shot "$t/text.png"
[ -n "${SHOT:-}" ] && cp "$t/text.png" "$SHOT"
: > "$t/gsettings.log"
ipc cancel
i=0
until cmp -s "$cfg" "$t/before.toml" || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
ok "escape: config.toml as it was" "$(cmp -s "$cfg" "$t/before.toml" && echo same)" "same"
ok "escape: GTK as the system had it" "$(waitfor '^font-name=Cantarell 11$' "$t/gsettings.log")$(grep -c '^monospace-font-name=DejaVu Sans Mono 10$' "$t/gsettings.log")" "11"
ok "escape: Qt as it was" "$(qtline general) $(qtline fixed)" 'general="Cantarell,11,-1,5,400,0,0,0,0,0,0,0,0,0,0,1,Regular" fixed="DejaVu Sans Mono,10,-1,5,400,0,0,0,0,0,0,0,0,0,0,1,Book"'
ok "escape: Alacritty's size gone" "$(grep -c 'draftpaper text' "$t/.config/alacritty/draftpaper.toml")" "0"
ok "escape: nothing kept any more" "$([ -e "$t/.local/state/draftpaper/text.json" ] && echo kept || echo gone)" "gone"
ok "no QML warning from the settings" "$(grep -c 'Settings.qml\|Fonts.qml' "$t/qs.log")" "0"
[ "$fail" = 0 ] && echo "text: ok" || { grep -a 'Settings.qml\|Fonts.qml' "$t/qs.log" | head -5; echo "text: FAILED"; }
exit $fail
