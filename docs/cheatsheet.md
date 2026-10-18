# Draftpaper cheatsheet

Mod is Super (the Windows key); in a nested session, Alt. Every key below can be changed in
`~/.config/draftpaper/config.toml` under `[bindings]`.

## Windows and workspaces

| Keys | |
|---|---|
| Mod+Return | Terminal (alacritty) |
| Mod+d | Launcher: the applications as a register on the sheet (fuzzel stays an alternative) |
| Mod+r | Rulers: two, crossing at a zero point, true millimetres; the pointer held to them (see Rulers) |
| Mod+i | The title block's column in turn: the equipment, the processes, the revisions, none (fading) |
| Mod+h j k l · Mod+Up/Down | Move the focus |
| Mod+Shift+h j k l · Mod+Shift+arrows | Move the window a zone, keeping its size, into free zones; past a window of the same rows (or columns) they swap; else it stays |
| Mod+Ctrl+h j k l · Mod+Ctrl+arrows | Resize by a zone (hold to keep going): right and down larger, left and up smaller |
| Mod+f | Fullscreen |
| Mod+Shift+q | Close the window |
| Mod+1 … Mod+9, Mod+0 | Workspace 1 … 9, 10 |
| Mod+Shift+1 … Mod+Shift+0 | Send the window to that workspace |
| Mod+Shift+e | Leave the session (written first, to come back at the next login) |
| Ctrl+Alt+F1 … F12 | Virtual terminal |

Every window takes a rectangle of zones. A new window takes the largest free one, else the second half of
the focused window; a closed window's zones go to the windows that line up with it, else stay paper.
Dialogs, and windows that cannot fill a tile (one fixed size, or a maximum below the tile), float centred.
A browser's picture-in-picture (Meet, videos) floats in the bottom-right corner, follows you from
workspace to workspace and leaves the focus where it was. `[[rule]]` in config.toml floats, pins or
places other windows.

The session comes back at the next login: every application open when you logged out (or the machine
went down) starts again, by its desktop entry or else its command line, and each window takes its old
place: its workspace, its zones, floating or parked in its cell. Terminals start in the directory their
shell was in, running again what they ran there if it was an editor, a pager, a monitor or tmux (never a
build, a script or ssh). An application started with documents opens them again. And the applications
take up their own state (`[session] apps`, written by `draftpaper apply`): Firefox and Chromium their
windows and tabs, Emacs its files each at its place (desktop-save-mode), Neovim the cursor in each file
and, started with no file, the session it last had in that directory, tmux its sessions, windows and panes
across a reboot. `[session] restore = false` turns it all off, `apps = false` the applications' part;
`skip` leaves applications out (by app id, or firefox, chromium, emacs, neovim, tmux). The session is in
`~/.local/state/draftpaper/session.json`.

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
| Mod+z | The magnifier on the pointer, each pixel a block; scroll zooms (2–64×), Shift+scroll sizes it, Escape or Mod+z puts it away |
| Mod+Shift+x | Lock now (`draftpaper lock`): touch the fingerprint reader (it listens until it knows the finger), or type the password and Enter |
| Mod+Shift+p | Lock before every suspend, until turned off or the next login |
| Volume, mic, brightness keys | wpctl (volume up to 150%), brightnessctl |

The displays turn off after 15 minutes idle (`[idle] screens_off`); any input, a key, the touchpad,
the touchscreen, wakes them.

## Launcher

| Keys | |
|---|---|
| Mod+d | Open the register of applications in the frame's top-left corner; what you type, in the margin above |
| Type · Backspace · Ctrl+u | Find (fuzzy: the name, then what it is, the keywords); the most used and recent first |
| ↑ ↓ · Tab · Ctrl+k j · Ctrl+p n · wheel | Along the register; Page up and down a page |
| Enter · click a row | Launch it, on the active workspace |
| Esc · click elsewhere | Close |

`draftpaper menu [--prompt TEXT] < lines` shows any list in the same register and prints the line
chosen (exit 1 when closed): a `fuzzel --dmenu` for scripts. For fuzzel on Mod+d instead, bind
`"Mod+d" = "spawn fuzzel --config \"${XDG_CACHE_HOME:-$HOME/.cache}/draftpaper/fuzzel.ini\""` in
`[bindings]`; `draftpaper apply` keeps its config in the theme's colours.

## Rulers

| Keys | |
|---|---|
| Mod+r | Bring the rulers out, or put them away (Esc) |
| ← → ↑ ↓ · drag a ruler | Move the crossing 1 mm (Shift: 10 mm) |
| Ctrl+← → · Ctrl+↑ ↓ · drag a red zero line · double-click a ruler | Slide the horizontal, the vertical ruler's zero 1 mm (Shift: 10 mm); the graduations count from it both ways |
| [ ] · , . · Ctrl+drag a ruler | Turn the horizontal, the vertical ruler 1° (Shift: 15°) |
| l | Lock the angle between them, or free them |
| 0 | Level both |
| s | Hold the pointer to their millimetres (Shift: ten), or let it go |
| Type in the panel's H, V, Enter | Turn a ruler to that angle |
| p · the panel's × · its tab | Hide the panel by the crossing (a tab stays), or show it |

`draftpaper rulers` does what Mod+r does. The steps, the numbered edges, the centre line, the width and whether the
pointer is held: the settings' Rulers page, or `[rulers]` in config.toml.

## Settings (colour scheme, wallpaper, displays, text, sheet, rulers, session, input, idle)

| Keys | |
|---|---|
| Mod+t · click the theme name | Open the settings on the board |
| Tab · Shift+Tab · click a page's cell | Colour scheme, wallpaper, displays, text, sheet, rulers, session, input, idle · lock |
| ↑ ↓ · k j | Between the drawings (the themes) and the builder's rows |
| ← → · h l · wheel | Along them: a theme, or a choice (the sheet previews it) |
| Enter · click the picked drawing | Keep it (the applications too; a built scheme becomes the theme `custom`) |
| ↑ ↓ then ← → on the other pages | A row, then its value: config.toml changes at once, comments kept (Escape puts the file back) |
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
| CPU / MEMORY, TEMPERATURE · FAN, POWER, BATTERY (the title block) | Their year as a graph on the sheet (a log time scale: 5 min, 1 h, 24 h, 7 d, 30 d, 1 y); again: away; another: switched |
| PROJECT, PROCESSES (the title block) | The schedule of equipment, the process schedule, in the title block's column; again: away |
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
| `draftpaper config set KEY=VALUE ...` | Keys in config.toml (`title.scale=false`), comments kept, as the settings' rows write them |
| `draftpaper wallpaper [FILE \| none]` | A picture in the frame, or none |
| `draftpaper margins paper\|ambient` | With a wallpaper: the margins in paper, or the picture's colours |
| `draftpaper lock` | Lock the screen; back once it is locked (swaylock if the lock screen does not lock) |
| `draftpaper lock-on-sleep [on\|off\|toggle]` | Lock before every suspend |
| `draftpaper apply` | Write the theme's files again (shell, launcher, lock, idle, applications) |
| `draftpaper readings` | What the title block shows, as JSON |
| `draftpaper-as USER [COMMAND]` | A program, or a shell, as another user on the session |
| `draftpaper-compositor msg outputs\|workspaces\|windows` | The compositor's state, as JSON |
| `draftpaper-compositor msg ACTION` | Any bindable action, e.g. `workspace 3`, `power off` |
| `draftpaper menu [--prompt TEXT] < lines` | A choice among stdin's lines, in the launcher's register |
| `draftpaper audio save\|restore` | The output's volume and mute, kept from one session to the next |
| `draftpaper tmux save\|keep\|restore` | tmux's sessions kept across a reboot (`[session] apps`; run by tmux itself) |
| `draftpaper-greeter [USER]` | The login screen, for greetd (README: Login screen) |
| `draftpaper-compositor grab DEVICE DIR [WIDTH]` | As root: what each display shows, read back to PNGs |
| `wlr-randr` | The displays: names, modes, `--scale`, `--pos`, `--transform`, `--off` (until changed again) |

## Files

| | |
|---|---|
| `~/.config/draftpaper/config.toml` | Every setting, read again when it changes (`man draftpaper`, the README) |
| `~/.config/draftpaper/themes/` | Your themes, and your copies of shipped ones |
| `/usr/share/draftpaper/themes/` | The shipped themes |
| `~/.cache/draftpaper/` | Generated: the shell's theme, launcher, lock screen, idle |
| `journalctl --user -u draftpaper-shell` | The shell's log |
| `journalctl -t draftpaper -t draftpaper-greeter` | The compositor's log, and the login screen's |
| `/var/lib/draftpaper/greeter/USER/` | The login screen's copy of your sheet (`draftpaper apply` writes it, when the directory exists) |

## Login screen

| | |
|---|---|
| (the name filled in) | The fingerprint reader listens at once, and again each time it gives up; touch it, or type the password and Enter |
| Enter | The name to the password, the password to greetd |
| Escape | Clear what is typed; again, the name too (while the reader listens: once it gives up) |
| Ctrl+u | Clear what is typed |
| Ctrl+Alt+F2 | A text console (VT1 has none: `fbcon=vc:2-63`) |

## Settings at a glance

Top level: `theme`, `margin`, `glyphs`, `wallpaper`, `margins`, `title_tint`. Tables: `[title]`
(project, drawn), `[[cell]]` (cells of your own), `[alerts]`, `[idle]`, `[apps]` (theme, skip,
firefox_text, firefox_ui_pt), `[hooks]` (theme), `[commands]` (screenshot), `[input]` (keyboard layout,
variant, options, repeat, tap, natural_scroll, focus_follows_mouse), `[[rule]]` (window rules:
app_id, title, pip; float, sticky, place, focus),
`[animations]`, `[session]` (restore, apps, skip), `[outputs.NAME]` (scale, position, mm), the wallpaper's `wallpaper_fit`,
`wallpaper_position`, `wallpaper_brightness`, `wallpaper_wash`, `wallpaper_blur`, `wallpaper_dirs`, `[bindings]`. Displays are matched by connector
name (`eDP-1`, `DP-3`); one without settings gets its preferred mode at scale 1, right of the others.
