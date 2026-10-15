# Draftpaper cheatsheet

Mod is Super (the Windows key); in a nested session, Alt. Every key below can be changed in
`~/.config/draftpaper/config.toml` under `[bindings]`.

## Windows and workspaces

| Keys | |
|---|---|
| Mod+Return | Terminal (alacritty) |
| Mod+d | Launcher (fuzzel) |
| Mod+h j k l · Mod+Up/Down | Move the focus |
| Mod+Shift+h j k l · Mod+Shift+arrows | Move the window |
| Mod+Ctrl+h j k l · Mod+Ctrl+arrows | Resize by a zone (hold to keep going) |
| Mod+f | Fullscreen |
| Mod+Shift+q | Close the window |
| Mod+1 … Mod+9, Mod+0 | Workspace 1 … 9, 10 |
| Mod+Shift+1 … Mod+Shift+0 | Send the window to that workspace |
| Mod+Shift+e | Leave the session |
| Ctrl+Alt+F1 … F12 | Virtual terminal |

The first two windows sit side by side; a third opens a new column, or a row under the focused one
(`[layout] new_window`). Dialogs, and windows that cannot fill a tile (one fixed size, or a maximum
below the tile), float centred. A browser's picture-in-picture (Meet, videos) floats in the bottom-right
corner, follows you from workspace to workspace and leaves the focus where it was. `[[rule]]` in
config.toml floats, pins or places other windows.

## Parking (the side margins)

| Keys | |
|---|---|
| Mod+Escape | Park the focused window in the first free cell; with every window parked, bring them all back |
| Mod+Left / Mod+Right | Bring back the previous / next parked window, parking the focused one; with nothing parked, move the focus |
| Click a parked title | Bring that window back |
| Click an empty side cell | Park the focused window there |

## Screen, lock, media

| Keys | |
|---|---|
| Print | A region (slurp) into swappy: annotate, save, copy |
| Shift+Print | The screen you are on into ~/Pictures |
| Mod+Print | Every screen into ~/Pictures (the compositor's own) |
| Mod+Shift+x | Lock now (swaylock) |
| Mod+Shift+p | Lock before every suspend, until turned off or the next login |
| Volume, mic, brightness keys | wpctl (volume up to 150%), brightnessctl |

The displays turn off after 15 minutes idle (`[idle] screens_off`); any input, a key, the touchpad,
the touchscreen, wakes them.

## Settings (colour scheme, wallpaper)

| Keys | |
|---|---|
| Mod+t · click the theme name | Open the settings on the board |
| Tab · click a page's cell | Colour scheme, wallpaper |
| ↑ ↓ · k j | Between the drawings (the themes) and the builder's rows |
| ← → · h l · wheel | Along them: a theme, or a choice (the sheet previews it) |
| Enter · click the picked drawing | Keep it (the applications too; a built scheme becomes the theme `custom`) |
| Esc · click elsewhere | Back to the theme and wallpaper you had |
| R twice | Your copy of a shipped theme back to the shipped one |

The builder's rows: paper (light, dark), tone, accent, windows (dark, light), windows shade (deep, dark,
charcoal, slate; light: white, paper, mist, grey), terminal background (as the windows, or a mid tone:
charcoal, graphite, slate, petrol, moss, olive, umber, wine, plum), terminal text, contrast
(standard about 7:1, high 10:1 and more). Every scheme it builds keeps every word at 4.5:1 or more.

The wallpaper page: the pictures (`wallpaper_dirs`: `~/Pictures/Wallpapers` and the system's
backgrounds; none for the paper), the margin cells (paper, or ambient: the picture's colours), the
title block's tint, mounted (fill, fit, its own size), position, brightness, paper wash, blur; ↑ ↓
between them, ← → along, each change shown at once.

## The sheet's margins and title block

| Click | |
|---|---|
| A workspace number (top left) | Switch to it |
| A notification (top) | Left: its action, then gone; right: gone |
| Volume (top right, while it shows) | Mute; scroll to change |
| Tray icons (top right) | The application's own; right click: its menu |
| BT (bottom left) | blueman-manager |
| SHOT (bottom right) | A region into swappy (`[commands] screenshot`) |
| The theme name (bottom right) | Left: the settings; right: the next theme |
| PROFILE (title block) | Cycle power-saver, balanced, performance |
| A cell of your own | Its `left` / `right` commands |

## Commands

| | |
|---|---|
| `draftpaper theme [list]` | The themes, the current one marked |
| `draftpaper theme NAME [--preview]` | Switch (the sheet only, with `--preview`) |
| `draftpaper theme next` | The next theme |
| `draftpaper theme check [NAME]` | Themes against the contrast rules |
| `draftpaper scheme options\|json\|preview\|keep [choice=value]` | A scheme built from choices, as the settings build it |
| `draftpaper theme revert NAME` | A shipped theme as shipped again |
| `draftpaper settings` | Open or close the settings |
| `draftpaper wallpaper [FILE \| none]` | A picture in the frame, or none |
| `draftpaper margins paper\|ambient` | With a wallpaper: the margins in paper, or the picture's colours |
| `draftpaper lock-on-sleep [on\|off\|toggle]` | Lock before every suspend |
| `draftpaper apply` | Write the theme's files again (shell, launcher, lock, idle, applications) |
| `draftpaper readings` | What the title block shows, as JSON |
| `draftpaper-as USER [COMMAND]` | A program, or a shell, as another user on the session |
| `draftpaper-compositor msg outputs\|workspaces\|windows` | The compositor's state, as JSON |
| `draftpaper-compositor msg ACTION` | Any bindable action, e.g. `workspace 3`, `power off` |
| `wlr-randr` | The displays: names, modes, `--scale`, `--pos`, `--transform`, `--off` (until changed again) |

## Files

| | |
|---|---|
| `~/.config/draftpaper/config.toml` | Every setting, read again when it changes (`man draftpaper`, the README) |
| `~/.config/draftpaper/themes/` | Your themes, and your copies of shipped ones |
| `/usr/share/draftpaper/themes/` | The shipped themes |
| `~/.cache/draftpaper/` | Generated: the shell's theme, launcher, lock screen, idle |
| `journalctl --user -u draftpaper-shell` | The shell's log |

## Settings at a glance

Top level: `theme`, `margin`, `glyphs`, `wallpaper`, `margins`, `title_tint`. Tables: `[title]`
(project, drawn), `[[cell]]` (cells of your own), `[alerts]`, `[idle]`, `[apps]` (theme, skip,
firefox_text, firefox_ui_pt), `[hooks]` (theme), `[commands]` (screenshot), `[input]` (keyboard layout,
variant, options, repeat, tap, natural_scroll, focus_follows_mouse), `[layout]` (new_window), `[[rule]]` (window rules:
app_id, title, pip; float, sticky, place, focus),
`[animations]`, `[outputs.NAME]` (scale, position, mm), the wallpaper's `wallpaper_fit`,
`wallpaper_position`, `wallpaper_brightness`, `wallpaper_wash`, `wallpaper_blur`, `wallpaper_dirs`, `[bindings]`. Displays are matched by connector
name (`eDP-1`, `DP-3`); one without settings gets its preferred mode at scale 1, right of the others.
