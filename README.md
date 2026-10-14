# Draftpaper

An i3 desktop drawn as a drafting sheet.

There is no bar. The wallpaper is a sheet of paper with a frame, a dot grid and zone ticks, and windows
end exactly on the frame line. What a bar would show is lettered onto the sheet:

- **The title block** in the bottom-right corner carries the readings: time, date, the workspace,
  power draw, battery, power profile, CPU and memory, network, temperatures and fan.
- **The top margin** letters the workspaces in its first zone. The focused one is bold black; one shown
  on another screen is bold. Alerts (temperatures past a threshold, a low battery) appear in its last
  zone in red, and volume and brightness appear there for a few seconds after they change.
- **The bottom margin** has Bluetooth, a screenshot command and the theme name (click it for the next
  theme).
- **The side margins** hold parked windows. Super+Escape parks the focused window in the first free
  cell, its title lettered up the margin; clicking an empty cell parks it in that cell. Click a title to
  bring the window back. Park every window of a workspace and the whole sheet shows; Super+Escape there
  brings them all back.

Everything is sized in millimetres of the screen (from the physical size xrandr reports), so the sheet
looks the same on a laptop panel and a 4K monitor.

![A window open on the sheet, two more parked in the left margin](docs/parked.png)

*One window open; `build.log` and `notes.md` parked in the left margin's cells B and C.*

![Every window parked: the whole sheet with its title block](docs/sheet.jpg)

*Every window of the workspace parked: the sheet, the three parked titles, and the title block.*

## Install

Draftpaper is a Debian package for i3 on X11. It needs Quickshell 0.3 or later, which Debian 13
provides in trixie-backports:

    sudo apt install -t trixie-backports quickshell
    sudo apt install ./draftpaper_0.1.1_all.deb      # from the latest release

Then, in `~/.config/i3/config`, after `set $mod ...`:

    include /usr/share/draftpaper/i3/draftpaper.conf

and remove your `bar { }` block. Restart i3 (Super+Shift+R). The include starts Draftpaper, sets the
window colours and the gaps, and binds Super+Escape for parking.

Cycling through parked windows uses Super+Left/Right, which are i3's default focus keys, so it is a
separate include. Remove your own `bindsym $mod+Left` and `bindsym $mod+Right` lines, then add:

    include /usr/share/draftpaper/i3/draftpaper-cycle.conf

With nothing parked on a workspace, those keys still move the focus left and right. To keep your keys,
bind others to the same commands instead:

    bindsym $mod+bracketright exec --no-startup-id draftpaper park next
    bindsym $mod+bracketleft  exec --no-startup-id draftpaper park prev

A compositor (picom) is recommended: the margins and the title block are transparent windows.

The sheet is lettered in IBM Plex Mono when it is installed, DejaVu Sans Mono otherwise. Plex is in
Debian's `contrib` section: add `contrib` to your apt sources, then `sudo apt install fonts-ibm-plex`.

## Keys

| Key | |
|---|---|
| Super+Escape | Park the focused window in the side margin; on a workspace with every window parked, bring them all back |
| Super+Right / Super+Left | With `draftpaper-cycle.conf`: bring back the parked window in the first / last cell and park the focused one in the next free cell; with nothing parked, move the focus |

Clicks: a workspace number switches to it; an empty side cell parks the focused window there and a
parked window's title brings it back; the profile cell cycles
the power profile; volume mutes on click and changes on scroll; the theme name switches theme.

## Configuration

`~/.config/draftpaper/config.toml`, every key optional:

```toml
theme = "softglow"
margin = 6                 # millimetres between the screen edge and the frame (3 to 20)

[title]
project = "WORKSTATION"    # default: the host name
drawn = "me"               # default: the user name (shown when there is no custom cell)

[[cell]]                   # cells of your own: the first in the title block's last row, the others
                           # in the bottom margin's free zones, one per zone
label = "TICKET"
command = ["my-timer"]     # its first line of output is the value
interval = 5               # seconds
left = ["my-timer", "toggle"]
right = ["my-timer", "pick"]

[alerts]                   # when temperatures and the battery turn red
cpu = 85                   # degrees Celsius
nvme = 60
fan = 5000                 # RPM
battery = 15               # percent

[commands]
screenshot = ["flameshot", "gui"]
```

Apply changes with `draftpaper apply`.

## Themes

Two themes ship: **Soft Glow** (warm greige paper, above) and **Blueprint** (a dark sheet: Prussian-blue
paper drawn in pale blue-white).

![The Blueprint theme with two windows parked](docs/blueprint.jpg)

`draftpaper theme` lists them; `draftpaper theme <name>` switches; clicking the theme name in the bottom
margin moves to the next. A theme is a TOML file naming the paper,
the ink, the focused workspace's colour (`strong`) and the window colours (see
`/usr/share/draftpaper/themes/softglow.toml`); your own go in
`~/.config/draftpaper/themes/`.

## How it works

`draftpaper start` (run by i3 on every start) renders a sheet for each output, writes the window colours
and gaps to `~/.cache/draftpaper/i3.conf`, and starts the Quickshell shell. The margins and the title
block are unmanaged windows that `draftpaper lower` pushes under every i3 window, so they read as part of
the paper and fullscreen windows cover them. Parked windows sit in i3's scratchpad, marked
`park:<workspace>:<n>`.

A window brought back is tiled next to the focused one: i3 cannot return it to its old place in the
layout.

## Licence

MIT. See [LICENSE](LICENSE).
