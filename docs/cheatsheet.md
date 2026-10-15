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
(`[layout] new_window`). Windows of one fixed size, and dialogs, float centred.

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

## Themes

| Keys | |
|---|---|
| Mod+t | The theme picker: the themes as drawings on the board |
| ← → ↑ ↓ · h j k l · wheel | Browse (the sheet previews the picked one) |
| Enter · click the picked one | Keep it (the applications too) |
| Esc · click elsewhere | Back to the theme you had |
| E | Edit the picked theme |
| Mod+Shift+t | The theme editor on the current theme (again: close it, dropping the changes) |

In the theme editor (E in the picker, or `draftpaper theme edit [NAME]`):

| Keys | |
|---|---|
| ↑ ↓ · k j | Pick a value |
| ← → · h l | Adjust it; with Shift, ten times as far |
| Tab · Shift+Tab | Hue, saturation, lightness |
| # | Type the colour as hex (Enter uses it, Esc leaves it) |
| Enter | Save your copy (`~/.config/draftpaper/themes/NAME.toml`) and switch to it |
| Esc | Drop the changes, back to where you came from |
| R twice | A shipped theme as shipped again (your copy removed) |

## The sheet's margins and title block

| Click | |
|---|---|
| A workspace number (top left) | Switch to it |
| A notification (top) | Left: its action, then gone; right: gone |
| Volume (top right, while it shows) | Mute; scroll to change |
| Tray icons (top right) | The application's own; right click: its menu |
| BT (bottom left) | blueman-manager |
| SHOT (bottom right) | A region into swappy (`[commands] screenshot`) |
| The theme name (bottom right) | Left: the picker; right: the next theme |
| PROFILE (title block) | Cycle power-saver, balanced, performance |
| A cell of your own | Its `left` / `right` commands |

## Commands

| | |
|---|---|
| `draftpaper theme [list]` | The themes, the current one marked |
| `draftpaper theme NAME [--preview]` | Switch (the sheet only, with `--preview`) |
| `draftpaper theme next` | The next theme |
| `draftpaper theme edit [NAME]` | The theme editor |
| `draftpaper theme revert NAME` | A shipped theme as shipped again |
| `draftpaper picker` | Open or close the theme picker |
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
variant, options, repeat, tap, natural_scroll, focus_follows_mouse), `[layout]` (new_window),
`[animations]`, `[outputs.NAME]` (scale, position, mm), `[bindings]`. Displays are matched by connector
name (`eDP-1`, `DP-3`); one without settings gets its preferred mode at scale 1, right of the others.
