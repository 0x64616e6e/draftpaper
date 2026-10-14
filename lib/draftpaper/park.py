"""Parking: windows go to the side margins and come back.

  draftpaper park toggle      the focused window goes to i3's scratchpad, its title to the first free
                              margin cell; on a workspace with no window left showing, every parked window
                              comes back
  draftpaper park at <cell> [<workspace>]
                              park the focused window in that cell (a click on an empty cell), if it is
                              free and the focused window is on <workspace>
  draftpaper park show <id>   bring that parked window back to its workspace
  draftpaper park next|prev   cycle: the parked window in the first (last) cell comes back and the focused
                              one takes the next free cell after the last (before the first) occupied
                              one, round the margin; with nothing parked on the workspace these move the
                              focus right or left
  draftpaper park save        rewrite the state file

A parked window carries the i3 mark "park:<workspace>:<cell>"; cells count down the left margin, then the
right, from 0. The shell tells how many cells a screen has in $XDG_RUNTIME_DIR/draftpaper/cells-<output>.
After every change the parked windows are written to $XDG_RUNTIME_DIR/draftpaper/parked.json,
{workspace: [{id, cls, title, cell}]}, which the shell watches. i3 brings a window back tiled next to the
focused one, not into its old split.
"""
import json, subprocess, sys
from . import paths

PREFIX = "park:"


def i3(*args):
    return subprocess.run(["i3-msg", *args], capture_output=True, text=True).stdout


def cmd(command):
    i3("-q", command)


def tree():
    return json.loads(i3("-t", "get_tree"))


def leaves(node):
    if node.get("window"):
        yield node
        return
    for c in node.get("nodes", []) + node.get("floating_nodes", []):
        yield from leaves(c)


def windows(t):
    """(workspace name, leaf) for every window; the scratchpad is "__i3_scratch"."""
    def walk(node):
        if node.get("type") == "workspace":
            for leaf in leaves(node):
                yield node["name"], leaf
            return
        for c in node.get("nodes", []) + node.get("floating_nodes", []):
            yield from walk(c)
    return list(walk(t))


def mark_of(leaf):
    """(workspace, n) from a park mark, or None."""
    for m in leaf.get("marks", []):
        if m.startswith(PREFIX):
            ws, _, n = m[len(PREFIX):].rpartition(":")
            try:
                return ws, int(n), m
            except ValueError:
                pass
    return None


def parked(t, ws=None):
    """{workspace: [(n, leaf, mark)]} of windows parked in the scratchpad, in cell order."""
    out = {}
    for where, leaf in windows(t):
        pm = mark_of(leaf)
        if pm and where == "__i3_scratch" and (ws is None or pm[0] == ws):
            out.setdefault(pm[0], []).append((pm[1], leaf, pm[2]))
    for v in out.values():
        v.sort(key=lambda e: e[0])
    return out


def save(t=None):
    t = t or tree()
    state = {ws: [{"id": l["id"], "cls": (l.get("window_properties") or {}).get("class", ""), "title": l.get("name") or "", "cell": n}
                  for n, l, _ in rows] for ws, rows in parked(t).items()}
    paths.ensure()
    with open(paths.PARKED, "w") as f:  # in place: the shell's file watch follows the file, not the name
        json.dump(state, f)


def focused_ws(t):
    def find(node, ws=None):
        if node.get("type") == "workspace":
            ws = node["name"]
        if node.get("focused"):
            return ws, node
        for c in node.get("nodes", []) + node.get("floating_nodes", []):
            r = find(c, ws)
            if r:
                return r
    return find(t)


def park(leaf, ws, n):
    stale = "".join(f'unmark "{m}", ' for m in leaf.get("marks", []) if m.startswith(PREFIX))   # left by a window taken out by hand
    cmd(f'[con_id={leaf["id"]}] {stale}mark --add "{PREFIX}{ws}:{n}", move scratchpad')


def unpark(leaf, mark):
    cmd(f'[con_id={leaf["id"]}] scratchpad show, floating disable, unmark "{mark}", focus')


def cells(ws):
    """How many margin cells the screen showing `ws` has (the margin writes it), 8 if unknown."""
    try:
        out = next(w["output"] for w in json.loads(i3("-t", "get_workspaces")) if w["name"] == ws)
        with open(paths.RUNTIME / f"cells-{out}") as f:
            return max(1, int(f.read().strip()))
    except (StopIteration, OSError, ValueError):
        return 8


def free(rows, total, after=-1, step=1):
    """The first free cell after `after` going `step`, round the margin; past the end if all are taken."""
    taken = {n for n, _, _ in rows}
    for k in range(1, total + 1):
        c = (after + step * k) % total
        if c not in taken:
            return c
    return max(taken | {total - 1}) + 1


def toggle():
    t = tree()
    ws, node = focused_ws(t)
    showing = [l for w, l in windows(t) if w == ws]
    rows = parked(t, ws).get(ws, [])
    if showing and node.get("window"):
        park(node, ws, free(rows, cells(ws)))
    elif not showing:
        for _, leaf, mark in rows:
            unpark(leaf, mark)
    save()


def park_at(cell, space=None):
    """Park the focused window in `cell`, if it is free, and if `space` is given only when the focused
    window is on that workspace (the click came from that screen's margin)."""
    t = tree()
    ws, node = focused_ws(t)
    rows = parked(t, ws).get(ws, [])
    if node.get("window") and (space is None or space == ws) and cell not in {n for n, _, _ in rows}:
        park(node, ws, cell)
        save()


def show(con_id):
    t = tree()
    for ws, rows in parked(t).items():
        for _, leaf, mark in rows:
            if leaf["id"] == con_id:
                here = [l for w, l in windows(t) if w == ws]
                if here:
                    cmd(f'[con_id={here[0]["id"]}] focus')      # onto its workspace, no back-and-forth
                else:
                    cmd(f'workspace --no-auto-back-and-forth "{ws}"')
                unpark(leaf, mark)
                save()
                return


def cycle(d):
    t = tree()
    ws, node = focused_ws(t)
    rows = parked(t, ws).get(ws, [])
    if not rows:
        cmd("focus right" if d > 0 else "focus left")
        return
    n0, leaf, mark = rows[0] if d > 0 else rows[-1]
    if node.get("window"):
        others = [r for r in rows if r[0] != n0]
        total = cells(ws)
        cell = free(others, total, rows[-1][0], 1) if d > 0 else free(others, total, rows[0][0], -1)
        park(node, ws, cell)
    unpark(leaf, mark)
    save()


def main(a):
    if not a:
        sys.exit(__doc__)
    if a[0] == "toggle":
        toggle()
    elif a[0] == "at":
        park_at(int(a[1]), a[2] if len(a) > 2 else None)
    elif a[0] == "show":
        show(int(a[1]))
    elif a[0] in ("next", "prev"):
        cycle(1 if a[0] == "next" else -1)
    elif a[0] == "save":
        save()
    else:
        sys.exit(__doc__)

