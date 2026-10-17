#!/bin/sh
# make pixels (and alone): the revision block above the title block (issue #45), headless (nothing on any
# display), on the T14's panel (1920x1200 at scale 1.5, 6.4 px/mm). A repository of the test's own with
# five commits; a terminal in it, narrowed to the left half so the corner shows. `draftpaper revisions`
# names the repository and its last four commits; the shell's block comes up, the compositor draws its
# rules (examples/sheetcheck.rs checks them pixel by pixel, and the first row lettered); the window parked,
# nothing focused, the rows stay; a terminal outside any repository takes them away; and so does
# [title] revisions = false. SHOT=FILE keeps the screenshot of the block.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
cli=$repo/cli/target/release/draftpaper
check=$repo/compositor/target/release/examples/sheetcheck
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
command -v foot > /dev/null || { echo "  (no foot: the revision block is not checked)"; exit 0; }
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-revisions.XXXXXX)
mkdir -p "$t/.config/draftpaper" "$t/Pictures" "$t/run" "$t/plain"
config() { printf 'theme = "softglow"\nmargin = 6\n[title]\nrevisions = %s\n[apps]\nskip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]\n[session]\nrestore = false\napps = false\n' "$1" > "$t/.config/draftpaper/config.toml"; }
config true
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

# the repository: five commits by one author on known days
git init -q "$t/drawing"
for n in one two three four five; do
    echo "$n" > "$t/drawing/$n"
    git -C "$t/drawing" add "$n"
    GIT_AUTHOR_DATE="2026-10-0${#n}T12:00:00" GIT_COMMITTER_DATE="2026-10-0${#n}T12:00:00" \
        git -C "$t/drawing" -c user.name=Drafter -c user.email=d@example.invalid commit -q -m "Sheet $n"
done

env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" DRAFTPAPER_MM=6.4 "$comp" --headless 1920x1200 --scale 1.5 --command true > "$t/log" 2>&1 &
nest=$!
pids=$nest
i=0
until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
sock=$runtime/draftpaper-$wl.sock
run() {
    env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" DRAFTPAPER_SOCKET="$sock" DRAFTPAPER_BIN="$cli" \
        DRAFTPAPER_ROOT="$repo" DRAFTPAPER_CACHE="$t/.cache/draftpaper" DRAFTPAPER_RUNTIME="$t/run" "$@"
}
run "$cli" apply > /dev/null
run qs -p "$repo/qml/shell.qml" -n > "$t/qs.log" 2>&1 &
pids="$pids $!"
i=0
until run "$comp" msg outputs 2>/dev/null | grep -q draftpaper-title-block || [ $i -gt 300 ]; do sleep 0.1; i=$((i + 1)); done
act() { run "$comp" msg "$@" > /dev/null; sleep 0.4; }
# whether the shell's block is up, waited for (N tenths of a second) to be $1
layer() {
    i=0
    while s=$(run "$comp" msg outputs | grep -c '"draftpaper-revisions"'); [ "$s" != "$1" ] && [ $i -lt "$2" ]; do sleep 0.1; i=$((i + 1)); done
    echo "$s"
}
shot() {
    rm -f "$t"/Pictures/*.png
    kill -USR1 $nest
    i=0
    until ls "$t"/Pictures/*.png > /dev/null 2>&1 || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
    sleep 0.3
    mv "$(ls "$t"/Pictures/*.png | head -n 1)" "$1"
}

# a terminal in the repository (its shell there), narrowed to the left half: the corner shows
run foot -D "$t/drawing" sh > /dev/null 2>&1 &
pids="$pids $!"
i=0
until run "$comp" msg windows | grep -q '"focused": true' || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
for n in 1 2 3; do act resize left; done
out=$(run "$cli" revisions)
ok "the repository named" "$(echo "$out" | grep -o '"repo": "drawing"')" '"repo": "drawing"'
ok "its last four commits, newest first" "$(echo "$out" | grep -o '"description": "Sheet [a-z]*"' | tr '\n' ' ')" \
    '"description": "Sheet five" "description": "Sheet four" "description": "Sheet three" "description": "Sheet two" '
ok "dated and signed" "$(echo "$out" | grep -o '"date": "2026-10-04", "by": "Drafter"' | head -1)" '"date": "2026-10-04", "by": "Drafter"'
ok "the block up" "$(layer 1 80)" "1"
[ $fail = 0 ] || grep -a -i "error\|warn" "$t/qs.log" | grep -v -i "QFont\|notification" | head -5
sleep 0.5
shot "$t/shown.png"
[ -n "${SHOT:-}" ] && cp "$t/shown.png" "$SHOT"
"$check" "$t/shown.png" 1.5 6.4 6 revisions shown | sed 's/^/  /' || fail=1

# parked: nothing focused, the rows stay
act park
sleep 5
ok "parked, nothing focused: still up" "$(layer 1 10)" "1"
# a terminal outside any repository: none
run foot -D "$t/plain" sh > /dev/null 2>&1 &
pids="$pids $!"
ok "a window outside any repository: gone" "$(layer 0 80)" "0"
shot "$t/plain.png"
act park
"$check" "$t/plain.png" 1.5 6.4 6 revisions none | sed 's/^/  /' || fail=1
# back to the repository's window, then switched off
act park_next
ok "back in the repository: up again" "$(layer 1 80)" "1"
config false
run "$cli" apply > /dev/null
ok "[title] revisions = false: gone" "$(layer 0 80)" "0"
exit $fail
