# Draftpaper as a Wayland compositor: design

Status: in progress. Decided 2026-10-14: Draftpaper is this compositor. The i3 edition is frozen at 0.1.x
(no further releases; the last one stays installable), and nothing in the compositor imitates i3: its IPC
is its own, and the shell is ported to it.

## Why

Draftpaper 0.1 draws a drafting sheet behind i3 and letters the bar's readings onto it. It works, but
every part of the idea is negotiated with a window manager that does not know about it:

| What Draftpaper wants | What i3 forces |
|---|---|
| Windows end on the frame line | Gaps tuned per DPI so i3's scaled pixels land on a line drawn elsewhere (off by one on some outputs) |
| The margins and title block are part of the paper | Unmanaged X11 windows pushed down the stacking order by a helper (`draftpaper lower`) and re-pushed after every remap |
| A parked window comes back where it was | It comes back tiled next to the focused window; the scratchpad keeps no place in the layout |
| Parking is a gesture: the window recedes into its cell | i3 has no animation; the window vanishes |
| The sheet is drawn at each screen's millimetres | One gap value for all outputs; per-output scale is approximated |
| One coherent thing | A 3 px dock strip only so i3 reserves something, a theme engine writing i3 config, scripts calling `i3-msg` |

A compositor that knows the sheet removes all of this: the frame is the layout's boundary, the margins
are a layer under the windows, a parked window keeps its place in the tree, and every movement can be
animated. Wayland is where Linux desktops are going, and X11 is in maintenance.

## What it is

A tiling Wayland compositor whose output is a drafting sheet:

- Each output shows one **sheet**: paper, a **frame** a margin in from the edge, **zones** of about 50 mm
  along the margins, a **title block** in the bottom-right corner.
- Windows tile **inside the frame**. The frame is the layout's outer boundary; there are no gaps to tune.
- The **margins** are the shell: the top margin letters the workspaces, alerts and transient readings, the
  bottom margin a few actions, the side margins hold **parked windows**, one per cell.
- **Parking** sends a window into a margin cell, keeping its place in the layout; bringing it back
  restores it there. Parking every window shows the whole sheet.
- The **title block** carries the readings a bar would show.

It keeps 0.1's model and vocabulary (sheet, frame, zones, cells, title block, parking) so users of the
i3 add-on find the same desktop.

## Goals for 1.0

1. Daily-drivable on a laptop with an external monitor: DRM/KMS session from a TTY or a display manager,
   libinput, multiple outputs with hotplug, per-output scale including fractional.
2. The Draftpaper layout: tiling inside the frame, workspaces per output, focus by keyboard and pointer,
   fullscreen (covers the sheet), floating for dialogs and popups only.
3. Parking with exact restore, by key, by clicking a cell, and cycling.
4. The sheet rendered by the compositor, live (theme switch without restart), at physical scale.
5. The margins and title block from the existing Quickshell code, unchanged where possible.
6. X11 applications through XWayland.
7. The everyday plumbing: clipboard and primary selection, drag and drop, screen locking, idle
   (blanking, inhibitors), screenshots and screencasting through the desktop portal, notifications.

## Non-goals for 1.0

- Tabs, stacked containers, manual split trees beyond what the layout below needs.
- Floating-window management (moving and resizing arbitrary windows as a mode).
- Gestures beyond workspace switching on a touchpad.
- HDR, colour management, VR, multi-GPU render offload beyond what the base library gives.
- Running on other compositors: this replaces i3 and sway, it is not a plugin.

## Foundation: Rust on Smithay

| Option | For | Against |
|---|---|---|
| **Smithay (Rust)** | The library behind niri and cosmic-comp, both daily-driven; memory safety in a long-running process that holds every client; good protocol coverage; anvil as a reference compositor | Fewer compositors built on it than wlroots; some protocols land later |
| wlroots (C) | Behind sway, river, labwc; widest protocol coverage; tinywl is a 1,000-line start | C for a large, long-lived process; wlroots' API changes between releases |
| river + a layout program | Smallest amount of code | The layout protocol cannot express parking with exact restore or the sheet as a surface; Draftpaper would be a client of someone else's model again |

**Choice: Smithay.** niri is the closest prior art: a Smithay compositor with an opinionated layout model,
its own IPC, configuration in a text file, XWayland (through xwayland-satellite; Draftpaper uses Smithay's built-in X11 window manager instead, see Plumbing), and a GNOME-portal
integration for screencasting. Its source is the reference for every piece of plumbing below; Draftpaper
differs in the layout model and the sheet, not in the plumbing.

## Architecture

```
            ┌───────────────────────── draftpaper (compositor process) ─────────────────────────┐
 libinput ─▶│ input ─▶ bindings ─▶ actions ─▶ layout (per output: workspaces, tiles, parked)  │
 DRM/KMS ◀─▶│ outputs, scale ◀── render: sheet (shader) ▸ background layers ▸ windows ▸ top/overlay │
            │ xdg-shell, layer-shell, Xwayland + X11 WM, session-lock, idle, data-device, ...    │
            │ IPC: Draftpaper's own protocol (Unix socket, newline-delimited JSON)                │
            └──────────────────────────────────────────┬──────────────────────────────────────────┘
                                                       │
       Quickshell (layer-shell, "bottom" layer) ◀──────┘   margins, title block, cells: shell.qml,
       talks to the compositor over the IPC                 adapted from 0.1
```

### What lives where

| Part | 0.1 (i3) | Compositor |
|---|---|---|
| Paper, dots, frame, zone ticks | Python renders PNGs, xwallpaper sets them | A shader in the compositor, per output at its own px/mm and scale; no files |
| Window placement on the frame | i3 gaps from a generated config | The layout's outer rectangle is the frame |
| Margins, title block, cells | Unmanaged X11 windows lowered by a helper | Quickshell layer-shell surfaces on the **bottom** layer: above the paper, below windows, by protocol |
| Parking | i3 scratchpad, marks, a state file | Layout state in the compositor; IPC events |
| Readings | Python `draftpaper readings` + QML | Unchanged |
| Theme | TOML, `draftpaper apply` | Same TOML, read by the compositor (paper, frame) and the shell (ink, alert) |

Keeping the margins in Quickshell (rather than drawing text in the compositor) reuses 0.1's QML,
keeps the compositor free of text layout, fonts and D-Bus readings, and lets the shell be restarted
without restarting the session. The compositor owns geometry; the shell draws in it.

### IPC

A Unix socket at `$XDG_RUNTIME_DIR/draftpaper-<wayland display>.sock`, its path in `DRAFTPAPER_SOCKET`
for the session's programs. Newline-delimited JSON: a client writes one request per line and reads one
reply per line, in order (`{"ok": true, "reply": ...}` or `{"ok": false, "error": "..."}`).

- `version`, `outputs`, `workspaces`, `windows`: the state now. Outputs carry the sheet's geometry
  (frame, margin, px/mm, zones), so the shell draws in the compositor's own numbers.
- `action`: any action a key binding can name (`workspace 3`, `focus left`, `send 4`, `spawn foot`...),
  one vocabulary for keys, scripts and the shell.
- `subscribe` to `workspaces`, `outputs`, `windows`: the current state at once, then the whole new state
  of a kind whenever it changes. Clients replace their model and never apply deltas, so a client that
  joins late or misses nothing to reconcile.
- Parking (#22, #24): outputs carry their number of `cells`, windows whether they are `parked` and in
  which `cell`; actions `park`, `park_at <cell> [<workspace>]`, `unpark <id>`, `park_next`, `park_prev`.
- `draftpaper-compositor msg <request | action...>` is the client for scripts and testing.

Not i3's protocol: an i3-compatible subset would have kept the 0.1 shell's workspace margin unchanged,
but the shell is ported anyway (layer-shell, the sheet's geometry), and i3's model (containers, marks,
the scratchpad) is the one Draftpaper leaves behind.

## The layout

### Within the frame

A workspace is a row of **columns**; a column holds one or more windows stacked vertically (its
**rows**). Both align to the **zones**: a column is a whole number of the top margin's zones wide and a
row a whole number of the side margins' zones tall, so every window edge lines up with a tick in the
margins. A new column takes whole zones; resizing moves an edge zone by zone. The drafting grid is the
layout grid.

On a laptop panel (6 zones across, 4 down) that allows, for example, a 4 + 2 split side by side and a
2 + 2 split within a column; on a 4K monitor (12 across, 7 down) the steps are finer. When the windows
on a workspace outnumber the zones along an axis, the last column or row takes what remains, still
ending on the frame.

This is deliberately simpler than i3's arbitrary split tree: it covers 2 to 5 windows per workspace,
which is how the desktop is used, and it makes "the place a window was" easy to record.

Focus moves left/right between columns and up/down within one. Moving a window left/right moves it into
the neighbouring column or makes a new one at the edge.

### Workspaces

Per output, numbered, optionally named (`9:mail`, lettered `9M`). Created on demand, removed when empty
and unfocused. Moving a workspace between outputs keeps its windows and parked set.

### Fullscreen and floating

Fullscreen covers the whole output, sheet included (the top layer stays hidden). Dialogs, popups and
windows that ask for a fixed size float, centred over their parent, inside the frame.

### Parking

A parked window leaves the layout but keeps a **placeholder** recording its column, its position in the
column and its size. Parking animates the window shrinking into its cell; unparking reverses it and
reinserts the window at the placeholder, re-establishing the column if it has gone, at the nearest
position otherwise. Placeholders are invisible and take no space.

- Super+Escape parks the focused window in the first free cell; on a workspace with nothing showing it
  brings every parked window back to its place.
- A click on an empty cell parks the focused window there; a click on a title brings it back.
- Super+Left/Right cycle (as in 0.1), each unpark restoring the exact place.
- Cells are per workspace; a screen shows the cells of its visible workspace. Cells count down the left
  margin, then the right, as in 0.1.

### Title blocks and margins per workspace

Each workspace has its own sheet: a screen shows only the title block, cells and lettering of the
workspace visible on it, never another workspace's, so two screens never repeat or crowd each other.
The bottom margin stays, as part of the drafting-sheet layout (Bluetooth, screenshot, theme), as in 0.1.

## The sheet

Rendered by a fragment shader into the output's background, once per frame only when something changed
(theme, scale, output size). Inputs: paper colours and shading (the 0.1 theme `sheet` table), ink, pattern
strength, margin in millimetres, px/mm and scale for the output. Output: the gradient with blooms, the
5 mm dots, the frame (2 physical px), zone ticks, crop marks, the 8-bit dither. The same maths as
`render.py`, so a theme looks the same on 0.1 and on the compositor.

Physical size comes from the output's EDID (as wl_output reports it); a configuration override per output
covers panels that report nonsense.

## Plumbing (what has to work before anyone uses it)

| Need | How |
|---|---|
| Session, devices | libseat (logind or seatd), udev, DRM/KMS with atomic modesetting; GBM + EGL/GLES renderer from Smithay |
| Input | libinput: keyboards with xkb layouts, touchpad (tap, natural scroll, two-finger gestures for workspaces), TrackPoint, touchscreen (on the laptop panel, turning with its transform) |
| Clients | xdg-shell, xdg-decoration (server-side: a 1 px border, no titles), wlr-layer-shell, xdg-output, viewporter, fractional-scale-v1, presentation-time, linux-dmabuf, single-pixel-buffer, cursor-shape |
| X11 apps | Xwayland (Debian's `xwayland`) started with the compositor as a rootless client, and Smithay's built-in X11 window manager: X11 windows tile like Wayland ones, transient windows and dialogs float, menus and tooltips place themselves; clipboard and primary selection bridged both ways, X11 clients reading them only while an X11 window has the keyboard. Chosen over niri's xwayland-satellite, which is neither in Debian nor on crates.io and would be a second program to ship in the one package |
| Clipboard | wl_data_device, primary selection, data-control (for clipboard managers) |
| Lock and idle | ext-session-lock (swaylock), ext-idle-notify (swayidle), idle-inhibit (video players) |
| Screens | wlr-screencopy for grim and flameshot; screencasting through xdg-desktop-portal (the GNOME backend with Mutter's screencast D-Bus API, as niri does, or xdg-desktop-portal-wlr) |
| Toplevels for tools | ext-foreign-toplevel-list, wlr-foreign-toplevel-management |
| Output configuration | wlr-output-management (kanshi, wdisplays) |
| Startup | a `draftpaper.desktop` session file for display managers; systemd user units (`graphical-session.target`) for the shell and helpers |

## Configuration

One TOML file, `~/.config/draftpaper/config.toml`, extending 0.1's: the theme, the margin, title-block
fields, custom cells, alert thresholds, and new sections for `[input]` (layout, repeat, touchpad),
`[outputs.<name>]` (mode, position, scale, px/mm override) and `[bindings]` (key → action, with 0.1's keys
as defaults). Reloaded on change without restarting.

## Milestones

| | What | Done when |
|---|---|---|
| M0 | Nested prototype: Smithay winit backend inside the current desktop; the sheet shader; xdg-shell windows tiled inside the frame in columns | A terminal and Firefox tile on a sheet in a window |
| M1 | Daily-drivable base: DRM session, libinput, multiple outputs, workspaces, focus, fullscreen, layer-shell, XWayland, clipboard, lock and idle, config and bindings | One working day on it, falling back to i3 only for screencasting |
| M2 | Draftpaper: zones, parking with placeholders and exact restore, cells, the IPC, the shell ported to it | 0.1's features on Wayland, parking better than on i3 |
| M3 | Finish: animations (park, unpark, workspace switch), fractional scale, screencasting through the portal, session files, the compositor and the shell in the `draftpaper` .deb | Installable from the .deb on a clean Debian 13 |

## Effort and risks

A rough guide, for one person working steadily, with niri's source as the reference: M0 one to two
weeks, M1 one to two months, M2 three to five weeks, M3 one to two months. Most of the time goes to M1
and the portal part of M3: the plumbing every compositor needs, not the ideas that make this one
different. Doing M2's layout and parking first in the nested prototype keeps the distinctive part in
front of the plumbing.

| Risk | Mitigation |
|---|---|
| Screencasting (video calls, recording) is the hardest piece to get right | Follow niri's portal integration closely; until it works, keep an i3 session (with the frozen 0.1.x) for calls |
| Smithay API churn | Pin a release; follow niri's upgrades, which track Smithay closely |
| XWayland edge cases (old apps, games, Java) | Smithay's X11 window manager handles the common cases (anvil and cosmic-comp use it); keep an i3 session (with the frozen 0.1.x) as the fallback |
| One maintainer for a compositor | Keep the compositor small: text, readings and theming stay in the shell and Python; scope stays at the non-goals above |

## Decisions

Settled on 2026-10-14:

1. **Columns and rows align to zones**, always: widths in the top margin's zones, heights in the side
   margins' zones (see The layout).
2. **Each workspace shows only its own title block** (and cells and lettering); a screen shows the sheet
   of the workspace visible on it.
3. **The bottom margin stays**: it is part of the drafting-sheet layout.
4. **One package**, `draftpaper`, holding the compositor, its session and the shell; **Debian only** for
   now. (First decided as one package for both editions; the i3 edition was then frozen, 2026-10-14.)
5. **Public repository**: anyone may open issues and send pull requests (see CONTRIBUTING.md).
6. **No i3 compatibility** (2026-10-14): the IPC is Draftpaper's own, the shell is ported to it, and the i3
   edition is frozen at 0.1.x.

## Open questions

1. How a third or fourth window joins: a new column or a row in the focused column? Both are there
   (`[layout] new_window = "column" | "row"`, 2026-10-15, column by default) to be tried in use; the
   choice goes into the decisions above (#12).
2. Whether the free resize (off the zones) is needed at all.

## References

- Smithay: https://github.com/Smithay/smithay (anvil, its example compositor)
- niri: https://github.com/YaLTeR/niri (Smithay compositor; IPC, portals, xwayland-satellite)
- xwayland-satellite: https://github.com/Supreeeme/xwayland-satellite
- cosmic-comp: https://github.com/pop-os/cosmic-comp
- wlr-layer-shell and the other protocols: https://wayland.app/protocols/
- Quickshell: https://quickshell.org (layer-shell; `Quickshell.Io` sockets for the IPC)
