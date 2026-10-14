# draftpaper-compositor

Draftpaper as a Wayland compositor, on Smithay 0.7. See `../docs/wayland-design.md` for the design and
the M0 to M3 milestones on GitHub for the plan.

Status: **M0, nested prototype.** It runs as a window inside an existing desktop (Smithay's winit
backend), accepts Wayland clients, and draws the sheet under them with a GPU shader (`src/sheet.rs`,
the same picture as the i3 edition's `render.py`, from the same theme files). Windows tile in columns
and rows measured in the sheet's zones (`src/layout.rs`), so every edge lines up with the ticks in the
margins.

Keys (on Alt while nested, because the outer window manager keeps Super):

| Key | |
|---|---|
| Alt+arrows, Alt+h/j/k/l | Focus the window left, right, above, below |
| Alt+Shift+arrows | Move the window: left/right into the neighbouring column (or a new column at the edge), up/down within its column |
| Alt+Ctrl+arrows | Move the boundary beside the focused window one zone that way |
| Alt+Return | Open a terminal |
| Alt+Shift+q | Close the focused window |

A new window opens a column right of the focused one, and the zones are shared out again; with every
zone across taken, it stacks as a row instead (#12 decides which should be the rule).

    DRAFTPAPER_MM=6.4 cargo run       # pixels per millimetre for the nested window (default 6.2)

    cargo run                         # opens the compositor window and starts alacritty in it
    cargo run -- --command foot       # or another Wayland client

Clients started from a terminal inside it connect through the `WAYLAND_DISPLAY` it sets.

The crate starts from smallvil, Smithay's example compositor (see NOTICE).
