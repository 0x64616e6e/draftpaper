# draftpaper-compositor

Draftpaper as a Wayland compositor, on Smithay 0.7. See `../docs/wayland-design.md` for the design and
the M0 to M3 milestones on GitHub for the plan.

Status: **M0, nested prototype.** It runs as a window inside an existing desktop (Smithay's winit
backend), accepts Wayland clients, and draws the sheet under them with a GPU shader (`src/sheet.rs`,
the same picture as the i3 edition's `render.py`, from the same theme files). The zone-aligned layout
comes next (#11).

    DRAFTPAPER_MM=6.4 cargo run       # pixels per millimetre for the nested window (default 6.2)

    cargo run                         # opens the compositor window and starts alacritty in it
    cargo run -- --command foot       # or another Wayland client

Clients started from a terminal inside it connect through the `WAYLAND_DISPLAY` it sets.

The crate starts from smallvil, Smithay's example compositor (see NOTICE).
