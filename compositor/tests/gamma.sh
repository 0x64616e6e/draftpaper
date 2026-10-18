#!/bin/sh
# make pixels (and alone): colour correction (issue #50: src/gamma.rs, src/icc.rs), headless (nothing on any
# display; headless keeps the gamma tables it is given). A display with [outputs.headless] icc = a profile
# whose calibration curves warm the screen (examples/iccwarm.rs: green to 90 %, blue to 75 %) has them in
# its table; a gamma-control client (examples/gammaset.rs, as gammastep) takes its place while it runs, a
# second is refused, and when it goes the profile's curves come back; the profile taken out of
# config.toml, the table straight; a file that is not a profile leaves it straight.
set -u
repo=$(cd "$(dirname "$0")/../.." && pwd)
comp=$repo/compositor/target/release/draftpaper-compositor
ex=$repo/compositor/target/release/examples
runtime=${XDG_RUNTIME_DIR:?no XDG_RUNTIME_DIR}
t=$(mktemp -d -p "${XDG_CACHE_HOME:-$HOME/.cache}" draftpaper-gamma.XXXXXX)
mkdir -p "$t/.config/draftpaper"
cfg=$t/.config/draftpaper/config.toml
"$ex/iccwarm" "$t/warm.icc"
printf 'theme = "softglow"\n[outputs.headless]\nicc = "%s"\n' "$t/warm.icc" > "$cfg"
pids=
trap 'kill $pids 2>/dev/null; rm -rf "$t"' EXIT INT TERM
fail=0
ok() { if [ "$2" = "$3" ]; then echo "  ok: $1"; else echo "  FAIL: $1: $2, not $3"; fail=1; fi; }

env -u WAYLAND_DISPLAY -u DISPLAY HOME="$t" DRAFTPAPER_MM=6.4 "$comp" --headless 1920x1200 --scale 1.5 --command true > "$t/log" 2>&1 &
pids=$!
i=0
until grep -aq "draftpaper: wayland socket" "$t/log" || [ $i -gt 100 ]; do sleep 0.1; i=$((i + 1)); done
wl=$(grep -a "draftpaper: wayland socket" "$t/log" | grep -o "wayland-[0-9]*" | head -1)
sock=$runtime/draftpaper-$wl.sock
i=0; until [ -S "$sock" ] || [ $i -gt 50 ]; do sleep 0.1; i=$((i + 1)); done
gamma() { DRAFTPAPER_SOCKET=$sock "$comp" msg gamma | jq -c '.[0] | [.source, .top]'; }
waitgamma() { i=0; until [ "$(gamma)" = "$1" ] || [ $i -gt 40 ]; do sleep 0.1; i=$((i + 1)); done; gamma; }

ok "the profile's curves in the table" "$(gamma)" '["icc",[65535,58982,49151]]'
env WAYLAND_DISPLAY="$wl" "$ex/gammaset" 1 0.5 0.25 > "$t/set.out" 2>&1 &
setter=$!
pids="$pids $setter"
ok "a tool (gammaset 1 0.5 0.25) takes its place" "$(waitgamma '["client",[65535,32768,16384]]')" '["client",[65535,32768,16384]]'
ok "and was told the table's size" "$(head -n 1 "$t/set.out")" "size 256"
ok "a second tool on the display is refused" "$(env WAYLAND_DISPLAY="$wl" timeout 5 "$ex/gammaset" 1 1 1 2>&1 | head -n 1)" "failed"
kill $setter
ok "the tool gone: the profile's curves again" "$(waitgamma '["icc",[65535,58982,49151]]')" '["icc",[65535,58982,49151]]'
printf 'theme = "softglow"\n[outputs.headless]\n' > "$cfg"
ok "the profile taken out: straight" "$(waitgamma '["identity",[65535,65535,65535]]')" '["identity",[65535,65535,65535]]'
echo "not a profile" > "$t/bad.icc"
printf 'theme = "softglow"\n[outputs.headless]\nicc = "%s"\n' "$t/bad.icc" > "$cfg"
sleep 2.5
ok "not a profile: straight, and said" "$(gamma)/$(grep -c 'no calibration curves' "$t/log")" '["identity",[65535,65535,65535]]/1'
[ "$fail" = 0 ] && echo "gamma: ok" || echo "gamma: FAILED"
exit $fail
