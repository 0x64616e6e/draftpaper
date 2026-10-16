#!/bin/sh
# make pixels (and alone): windows in zones, headless (nothing on any display). On a sheet of 4 by 2
# zones: a window alone resized to a zone and moved about in zone steps, its size kept, never past the
# frame; a move with nowhere to go changes nothing; two windows on different rows, one moved under the
# other keeping both sizes, then swapped; with every zone taken, a ninth window floats.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-zones.XXXXXX)
mkdir -p "$t/.config/draftpaper"
printf 'theme = "softglow"\nmargin = 6\n[apps]\nskip = ["gtk", "qt", "firefox", "x11", "emacs", "alacritty", "foot", "tmux", "neovim"]\n' > "$t/.config/draftpaper/config.toml"
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM

env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" DRAFTPAPER_MM=6.2 "$comp" --headless 1280x800 --command true > "$t/log" 2>&1 &
pids=$!
i=0
until grep -aq "Created new socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
wl=$(grep -a "Created new socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
[ -n "$wl" ] || { echo "the headless compositor did not start"; exit 1; }
sock=$runtime/draftpaper-$wl.sock
msg() { DRAFTPAPER_SOCKET=$sock "$comp" msg "$@" > /dev/null; sleep 0.3; }
# the windows' tiled geometry, one "x y w h floating" line each
geo() { DRAFTPAPER_SOCKET=$sock "$comp" msg windows | tr -d ' ,"' | awk -F: '/^floating/{f=$2} /^geometry/{g=1} g&&/^x/{x=$2} g&&/^y/{y=$2} g&&/^width/{w=$2} g&&/^height/{h=$2} /^id/{if(g)print x, y, w, h, f; g=0}'; }
term() { env -u DISPLAY HOME="$t" XDG_CONFIG_HOME="$t/.config" WAYLAND_DISPLAY="$wl" foot > /dev/null 2>&1 & pids="$pids $!"; sleep 0.8; }
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

zones=$(DRAFTPAPER_SOCKET=$sock "$comp" msg outputs | tr -d ' ,"' | awk -F: '/^across/{a=$2} /^down/{d=$2} END{print a "x" d}')
echo "zones: $zones"
ok "a sheet of 4 by 2 zones" "$zones" "4x2"

term
whole=$(geo)
for i in 1 2 3 4 5; do msg resize left; done
one=$(geo); set -- $one; w1=$3 x1=$1
ok "resized to a zone, no further" "$(echo "$whole" | awk '{print $1}')" "$x1"
msg move left
ok "at the frame, a move left changes nothing" "$(geo)" "$one"
msg move right; set -- $(geo); x2=$1
msg move right; msg move right; set -- $(geo); x4=$1 w4=$3
# as wide as before, but for the zones' own rounding (301 or 302 pixels here; the frame's line is
# wider than a tick)
d=$((w4 - w1)); ok "moved three zones on, a zone wide still ($w1, $w4)" "$([ ${d#-} -le 3 ] && echo yes)" "yes"
[ "$x2" -gt "$x1" ] && [ "$x4" -gt "$x2" ] && echo "  ok: each step a zone to the right ($x1, $x2, $x4)" || { echo "  FAIL: steps $x1 $x2 $x4"; fail=1; }
last=$(geo)
msg move right
ok "at the frame on the right, a move right changes nothing" "$(geo)" "$last"
msg resize up
msg move down; set -- $(geo); yd=$2
msg move down
ok "down a zone, then no further" "$(geo | awk '{print $2}')" "$yd"
for i in 1 2 3 4 5 6 7 8; do msg resize right; msg resize down; done
ok "grown back to the whole frame" "$(geo)" "$whole"

# two windows on different rows: the lower one moves left under the upper one, both keeping their sizes;
# then up, against a window of the same columns: they swap
term
line() { geo | sed -n "$1p"; }
ok "the second takes the right half" "$(line 1 | awk '{print $1 + $3 < 640}')" "1"
msg resize up; msg move down
msg focus left; msg resize up; msg focus right
a=$(line 1); b=$(line 2)
msg move left; msg move left
set -- $a; ax=$1 ay=$2; set -- $b; bw=$3 bh=$4 by=$2
ok "the upper one unchanged" "$(line 1)" "$a"
# its width up to 3 pixels off, as above: by the frame the zone's own rounding differs
ok "the lower one under it, its size kept" "$(line 2 | awk -v w="$bw" '{d = $3 - w; print $1, $2, (d < 0 ? -d : d) <= 3, $4}')" "$ax $by 1 $bh"
msg move left
ok "at the frame, a move left changes nothing" "$(line 2 | awk '{print $1}')" "$ax"
msg move up
ok "up against the same columns: they swap" "$(line 1 | awk '{print $2}') $(line 2 | awk '{print $2}')" "$by $ay"

for i in 3 4 5 6 7 8; do term; done
ok "eight windows tiled, none floating" "$(geo | awk '$5=="false"' | wc -l)" "8"
term
ok "a ninth floats" "$(geo | awk '$5=="true"' | wc -l)" "1"
exit $fail
