# The sheet's own client: letting go of Quickshell for the always-on sheet

Status: stages 0 and 1 built, 2026-10-19; the rest proposed. Measured against 0.7.4.

## Why

The shell is the sheet's lettering: the four margins and the title block, drawn on every display for as
long as the session lasts. It is written in QML and run by Quickshell, which brings Qt Quick with it: a
scene graph, a JavaScript engine, and a drawing context on the GPU for every window. The sheet puts five
windows on each display, so the cost is paid five times a screen.

Measured headless, two displays, nothing but the sheet:

| | PSS | Threads |
|---|---|---|
| Quickshell with an empty shell (the floor) | 59 MB | — |
| The sheet's shell | 204 MB | 69 |
| One display | 166 MB | 45 |
| Three displays | 246 MB | 96 |

About 40 MB and 26 threads per display, for lettering that changes a few times a second. For comparison,
`foot` — a Wayland terminal that loads fonts, shapes text and draws it into shared memory — runs at 2 MB.

Three cheaper ways out were tried and rejected (0.7.4):

- **Software rendering** (`QT_QUICK_BACKEND=software`): 133 MB, 17 threads. Two and a half times the CPU
  on every repaint, warnings about fractional scaling on every window update, and the settings page drew
  differently.
- **One render thread** (`QSG_RENDER_LOOP=basic`): 176 MB, 58 threads. It broke the title block's edge at
  scale 1.25 — the lettering crossed the frame's line. `make pixels` catches it. Pixel-exactness is the
  whole point of the sheet, so this is not a trade we can make.
- **Lazy windows** for the schedules and the graph: shipped in 0.7.4, about 20 MB. Worth having, not an
  answer.

What is left is to stop asking a toolkit built for animated application interfaces to draw a static
drawing.

## What it is

`draftpaper-sheet`: a Wayland client in Rust that draws the always-on sheet — the four margins and the
title block on every display — into shared-memory buffers on the CPU, with no GPU context of its own.

It is a client, not part of the compositor, for one reason: a fault in text layout should lose the
lettering, not the session. The compositor already draws the paper, the frame, the ticks, the dots, the
title block's rules and the scale bar's bar; it knows where every one of those is in whole physical
pixels. The client only letters into rectangles the compositor gives it.

### What stays in QML

Settings (1,209 lines), the launcher and the rulers are interactive interfaces where Qt earns its cost.
They stay as they are, but Quickshell is started when one is opened and exits when it closes, so nothing
of it is resident. The greeter and the lock screen are already their own Quickshell processes, started
only when needed; they do not change.

A first open of the settings then costs the second or so Qt takes to start. That is acceptable for a page
opened a few times a week.

## Foundation

- **`smithay-client-toolkit`** for the Wayland side: the layer shell, shared-memory pools, the seat,
  outputs, `wp_fractional_scale_v1` and `wp_viewporter`. The compositor is already built on Smithay, so
  this is the same ecosystem and the same release cadence, not a second one to follow.
- **`tiny-skia`** for drawing: fills, strokes, dashes and gradients on the CPU. The graph's lines and
  fading grid need it; the margins barely do.
- **`cosmic-text`** for text, with a font database holding only the families the theme names, never the
  whole system. Shaping is not needed for monospace Latin, but the sheet letters `°`, `·`, `×`, `″` and,
  when `[glyphs]` is on, Nerd Font glyphs from the private use area.
- **`zbus`** for the DBus services below. Already a dependency of the CLI since the readings stopped
  starting `nmcli`, `bluetoothctl` and `busctl`.
- **The `draftpaper` crate itself**, as a library. `readings`, `machine`, `processes` and `revisions` are
  already Rust in `cli/src`; the client calls them in process. No `draftpaper readings` is forked every
  five seconds, which removes the last of the shell's child processes.

Estimated 10–20 MB resident for the client, plus about 4 MB of buffers per 4K display.

## What it draws

Everything the always-on shell draws today, and nothing it does not. Taken from the QML so none of it is
forgotten:

**Title block** (`TitleBlock.qml`): twelve cells, each lettered with a label and a reading — time, date,
sheet, project, power, battery, profile, CPU / memory, network, temperature · fan, processes, drawn, and
the config's own cell. Each whole cell is a click area with a hover tint; a click opens a reading's graph,
the project's equipment schedule, the processes' schedule, or cycles the power profile.

**Top margin** (`shell.qml`): the workspaces in the first zone — up to five share a zone, more run on into
the next in fixed slots, the focused one bold, each a click away; a notification in the zone before the
last, growing leftwards, with `+2` for those waiting; the column numbers in the zones nothing else takes;
volume, brightness and the tray in the last zone.

**Bottom margin**: Bluetooth in the first zone; the config's extra cells, one per zone; the column
numbers; the scale bar's centimetres lettered over the compositor's rules, `mm` after the last, `NTS` on a
display that reports no size; the screenshot and the theme's name in the last zone.

**Side margins** (`SideCells.qml`): a cell per zone lettered with its zone letter, and the title of each
window parked on the workspace this screen shows, running up the margin. A click on a title brings the
window back, a click on an empty cell parks the focused window there.

**The schedules and the graph** (`Schedule.qml`, `LoadGraph.qml`, 660 lines of QML): drawings with click
areas, not interfaces. They move in the last stage, and until they do they stay in the on-demand
Quickshell.

## Invariants

The sheet's rules (`wayland-design.md`, "Invariants: drawing on the sheet") hold here unchanged, and this
is where the risk of the whole project sits:

- Everything is placed in whole millimetres from the frame's corner, then snapped to whole physical
  pixels.
- The client never computes the sheet's geometry. The compositor publishes `frame`, `zones`, `margin`,
  `px_per_mm`, `scale`, `title.cells` (a rectangle each), `scale_bar.marks`, `scale_bar.text` and
  `key_plan` over the IPC socket today. The client subscribes and letters into what it is given.
- Fractional scale is handled with `wp_fractional_scale_v1` and `wp_viewporter`, so the buffer is in
  physical pixels and nothing is resampled. Scale 1.25 and 1.75 are the cases that break naive clients,
  and `make pixels` tests both.

## The services

Four things the shell does besides drawing. They are the hardest part of this plan, not the text.

| | Where it goes | Note |
|---|---|---|
| Battery | `cli/src/readings.rs` | Already read from `/sys/class/power_supply`. UPower not needed. |
| Audio (volume, mute) | `pipewire-rs` in the client | Or `wpctl` on change only — never polled. |
| Notifications | `zbus` server in the client | `org.freedesktop.Notifications`: Notify, CloseNotification, GetCapabilities, GetServerInformation, and the signals. |
| System tray | `zbus`: StatusNotifierWatcher and Host, DBusMenu client | The largest and least pleasant of the four. |

The tray is the one that may be deferred. If it is, the last zone of the top margin keeps volume and
brightness and loses its icons until the client grows them — a visible loss, to be agreed before starting,
not discovered halfway.

## Stages

Each stage ends with something that runs and is tested. The client is a new crate, `sheet/`, in the
workspace.

0. **A surface and nothing on it.** *(done)* Layer shell, one surface per margin per display, placed from
   the compositor's geometry and laid out again when the displays change. Each margin carries one rule a
   physical pixel wide, 50 mm from the frame's corner, and `compositor/tests/sheet.sh` (part of `make
   pixels`) measures it at scales 1, 1.25, 1.5, 1.75 and 2, and on two displays of different densities.
   Fractional scale is handled as planned: the buffer is in physical pixels and a `wp_viewport` maps it to
   the logical size, so the rule is one pixel wide and on its pixel at 1.25 and 1.75 as much as at 1.
   Two displays, one of them 4K: **1 MB of memory, one thread, no measurable CPU**, an 836 KB binary —
   against the shell's 204 MB and 69 threads for the same two. Nothing is lettered yet, so this is the
   floor rather than the finished cost, but the per-window drawing contexts that the 40 MB a display went
   on are simply not there.
1. **Text, and the title block.** *(done)* The face the theme names, found with `fontdb` and rasterised
   with `fontdue` — the sheet letters in one monospaced family, so nothing has to be shaped, and a glyph
   is rasterised once at each size and kept. A second family covers what the first does not have, a Nerd
   Font among them. The twelve cells are lettered where the shell lettered them, from the readings taken
   in process: `cli/` now builds a library as well as a program, so no `draftpaper readings` is started.
   `tests/sheet.sh` counts the ink in each cell's label and reading, against a floor that tells lettering
   from the rules that edge the cells. Two displays: 15 MB, three threads, 0.1% of a core — the last for
   reading the machine every five seconds, as the shell did.
2. **The margins.** Workspaces, notifications, zone letters, parked titles, the scale bar's figures, the
   column numbers, the config's cells, the theme's name. The existing `pixels.sh`, `zones.sh`, `scale.sh`,
   `text.sh` and `displays.sh` cover most of this already.
3. **Input.** Clicks and hover: workspace switching, parking and unparking, the title block's cells, the
   cursor shapes. Verified with the compositor's own `msg pointer` and `msg button`, as the tests do now.
4. **The services.** Audio, notifications, Bluetooth, and the tray or a decision not to have one.
5. **The swap.** `draftpaper start` runs the client instead of Quickshell; the settings, launcher and
   rulers are started on demand and exit when closed. The session's service file changes with it.
6. **The schedules and the graph**, moved across. Only after everything else is steady.

Stages 0–3 are the sheet. 4 is the plumbing. 5 is the day the memory is actually saved.

## Testing

The pixel tests are the reason this is worth attempting at all. `make pixels` runs eighteen headless
checks that already assert where every rule and every piece of lettering lands, at five scales, on one
display and on several. They start `qs`; they would start the client instead. A text stack that puts a
word a pixel off fails them.

Two new checks are needed:

- The client's memory and thread count, as `idle.sh` does for the compositor's frames, so this does not
  creep back.
- The lettering compared with Quickshell's, during the stages where both exist: the same sheet drawn by
  both, their screenshots compared with `sheetcheck`.

## Risks

- **Glyph rendering differs from Qt's.** Hinting and subpixel positioning will not match exactly. The
  pixel tests mostly check rules and the luminance of lettered regions, not glyph pixels, but some will
  need their tolerances looked at — carefully, one at a time, never loosened in bulk to make a stage pass.
- **The tray.** StatusNotifierItem and DBusMenu are poorly specified and widely varied. Budget as much for
  it as for the text, or decide to do without.
- **Fractional scale.** The whole project fails if this is wrong. It should be the first thing proved, in
  stage 0, on a 1.25 display with nothing drawn but a one-pixel rule.
- **Two shells at once.** Between stages 1 and 5 some of the sheet is drawn by the client and some by
  Quickshell. They must not both letter the same zone. Keeping the stages in the order above means the
  title block moves first and whole margins move together.
- **Effort.** About 3,000–3,500 lines of Rust, against the compositor's 11,700 and the CLI's 4,200. Call
  it a week of clear days, and more if the tray is kept.

## What success looks like

| | 0.7.4 | After |
|---|---|---|
| Idle session, two displays | ~330 MB | ~60–70 MB |
| Processes resident | compositor, shell, metrics | compositor, sheet, metrics |
| Child processes a second | 1 | 0 |
| Compositor idle CPU | 0.15% | unchanged |

On a laptop with 62 GB the difference is a rounding error. On the 4 GB machines Draftpaper is being
packaged for, it is the difference between a desktop that fits and one that does not. That — not this
laptop — is the reason to do it.

## What we give up

- QML's hot reload for the sheet's lettering: a change means a rebuild.
- The breadth of Quickshell's services, which we would be reimplementing a piece at a time.
- A single language for the shell: the sheet becomes Rust, the settings stay QML.

The second is the real cost, and it is the reason to keep the settings, launcher and rulers where they
are rather than chasing a desktop with no Qt in it at all.
