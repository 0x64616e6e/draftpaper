# Changelog

What changed in each release, newest first. The Debian package's own record, with every detail, is
`debian/changelog`.

## Unreleased

- A year of readings on the graph, on a logarithmic time scale: now at the right, then the last 5 minutes,
  the hour, the day, the week, the month and the year, each period ticked and named. `draftpaper metrics
  keep` (draftpaper-metrics.service, part of the session) samples every 5 s into six periods of fixed slots
  (5 s, a minute, a quarter hour, an hour, a quarter day, a day): each slot keeps its count, sum, least
  and most, so the year is some 870 slots a reading. One line through all the periods, the bold reading's
  least to most a band under it, broken where nothing was kept. The file is in the runtime directory,
  kept on disk every 5 minutes; `draftpaper metrics` prints it. The shell no longer samples on its own.

## 0.7.2 (2026-10-18)

The graph within its zone, notes that show where they fit, and a quieter shell.

- The graph is no taller than a zone of the sheet (it could reach 72 mm and looked stretched), and by
  default fills the bottom row of zones, from its top line to the frame's: within its zone.
- The shell reads the memory, the CPU line and the backlight in place, and the network comes with
  `draftpaper readings`: no `sh`, `nmcli`, `grep`, `awk`, `ip` or `head` started every second or two.
- A graph or schedule placed where it does not fit on a display (a place dragged to on a larger one)
  shows where it fits instead of not at all: the graph on the panel stayed away.

## 0.7.1 (2026-10-18)

The revisions in the title block's column, the schedules off the frame, and `draftpaper apply` quiet when
nothing changed.

- **The revisions take their turn in the title block's column** with the schedule of equipment and the
  process schedule (Mod+i: equipment, processes, revisions, none), drawn as they are, as many commits
  as fit (up to 30); with `[title] revisions = true` they come by themselves when nothing else is
  chosen. Nothing is drawn above the title block any more.
- The schedules sit 6 mm in from the frame's top and right lines (they sat on them), with no margin of
  their own; one with more rows than fit fills the column, its last row 6 mm above the title block, as
  the plotted load keeps from it (the millimetres left over go to its heading).
- `draftpaper apply` reloads an application (tmux, Alacritty, Neovim, Emacs, X, GTK's settings) only when
  its theme file changed, each file written once with [text]'s fonts in it: an apply that changes
  nothing redraws nothing (tmux was re-sourced, and Alacritty reloaded around it, on every one).

## 0.7.0 (2026-10-18)

One settings tool for the whole desktop, the machine and its load drawn on the sheet, colour correction,
and everything on the sheet on one millimetre grid.

- **Settings (#36).** The settings (Mod+t) gain Displays, Text, Sheet, Rulers, Session, Input and
  Idle · lock pages, each a list of config.toml's keys with their values to pick. A pick shows at once;
  Enter keeps it, Escape puts config.toml back as it was. The Displays page draws the displays as
  arranged, to scale, and sets each one's scale and which side of the one before it it sits. The Text
  page sets one interface face and one monospace face with their sizes for GTK, Qt, the terminals, Emacs
  and the sheet's lettering (`[text]`), each size with its x-height and the angle it makes at the
  viewing distance, lettered red below the legibility target. `draftpaper config set table.key=value`
  writes any key, the file's comments and order kept (quoted names too: `outputs."ASUS MB166C".scale=1`);
  `draftpaper config json` reads them.
- **A display's settings follow it.** `[outputs."ASUS MB166C"]`, by its model, before `[outputs.DP-3]`,
  by its port; a key taken out puts the display back as plugged in.
- **The schedules and the graph (#49).** Mod+i (`draftpaper schedule next`) brings a schedule of
  equipment (machine, processor, graphics, memory, storage, system, each display, the sheet: `draftpaper
  machine`), then a process schedule (`draftpaper processes`: the busiest programs, each one's processes
  counted as one, its share of a processor ruled as a bar), then neither, fading in and out in the title
  block's column from the top of the frame; PROJECT and a new PROCESSES cell in the title block bring
  them too. A click on a reading's cell (CPU / memory, temperature and fan, power, battery) shows its
  last ten minutes on millimetre squares that fade out, each line lettered with its name and value. All
  part of the sheet: no outline or panel; over a wallpaper on the paper's halo. A zone (`[sheet]
  schedule_at`, `load_at`) or a drag moves them, a whole millimetre at a time.
- **Colour correction (#50).** `[outputs."<model>"] icc = "profile.icc"` loads an ICC profile's
  calibration curves (vcgt) into the display's gamma table, as xcalib does on X11; and wlr-gamma-control,
  so gammastep and wlsunset work (a tool's ramps while it runs, the profile's again when it quits).
  `draftpaper-compositor msg gamma`.
- **One millimetre grid.** A design invariant, in docs/wayland-design.md: everything on the sheet sits at
  whole millimetres from the frame's corner, then on whole pixels. The sheet's 5 mm dots are counted from
  the frame's corner, each on its nearest pixel (they drifted a millimetre across a 5.6 px/mm display);
  the rulers' zero too (it was 2 pixels off) and each of their ticks on its pixel, so a level ruler's
  ticks fall on the dots. `make pixels` checks every dot and tick on its pixel.
- **Rulers.** `[rulers]`: the steps, turns, numbered edges, centre line, width and pointer hold.
- **Two displays at different scales.** A window whose shadow reaches over onto the other display
  (Firefox at the left margin of a 4K display beside the panel) is told only its own display's scale and
  drawn only there: it flickered and was drawn whole on the other.
- **The pointer comes along** on a workspace switch to another display, so the focus stays there.
- A clickable cell of the title block takes the click over the whole of it and shows a tint under the
  pointer. Hatching can be set in config.toml's `[sheet]`, over the theme's.
- For scripts and tests: `draftpaper-compositor msg button left down|up`; headless displays take their
  models from DRAFTPAPER_MODELS.

## 0.6.0 (2026-10-18)

The sheet keeps a drawing's conventions: a scale bar true to size, a revision block, hatching, a key
plan; rulers to measure and draw by in true millimetres; and a website with theme and wallpaper
generators.

- **Rulers** (#48, Mod+r): a horizontal and a vertical ruler, frame line to frame line, crossing at a zero
  point that slides anywhere on the sheet, one piece where they cross, both edges graduated in true
  millimetres from the zero both ways. The zero moves 1 mm (10 with Shift), a ruler turns 1° (15°); each
  turns freely or the two are locked at any angle; an inclined ruler shows its degrees, locked ones the
  angle between them; angles can be typed in the panel by the zero. While they are out the compositor
  holds the pointer to their millimetres at any angle (rulers.rs), small movements adding up. Checked by
  `make pixels` (tests/rulers.sh): ticks at true millimetres, the pointer on whole millimetres level and
  turned, free once they are away. `msg pointer` says where the pointer is; `pointer_by DX DY` moves it as
  a mouse does.
  The panel by the crossing hides (p, or its ×) when it covers the angles, a tab bringing it back. Each
  ruler has its own zero, a red line that slides along it (drag it, double-click the ruler, Ctrl+arrows),
  the graduations counting from it both ways: measured from anywhere, clear of the crossing. Ticks on
  both edges, numbers by one only: the top, the right, as measuring is done in the top right quadrant.
- **A key plan** with several screens (#47), as a drawing set shows which part of the whole a sheet is: in
  the bottom margin left of the scale bar, every screen's outline in their arrangement, this one filled,
  the focused one twice as heavy; drawn by the compositor on whole pixels. Left of it a north mark, a
  drawing's north arrow drawn square: a square, a thin cross running a little past it, and the arm toward
  each other screen a solid bar from the centre to the square's edge. `[title] key_plan = false` takes both away.
- The headless backend drives several outputs side by side (`--headless 1920x1200,2560x1440`, a
  `DRAFTPAPER_MM` each), so `make pixels` checks the multi-screen sheet (tests/keyplan.sh).
- **Hatching** for parked windows' cells (#46), as a drawing hatches a cut: one-pixel lines at 45°, 1.5 mm
  apart, laid from the screen's corner so they run on from cell to cell, in the margins' ink at a low
  strength; broken around the zone letter and the title, which the shell reports to the compositor.
  `[sheet] hatch_strength` and `hatch_spacing` (0: none) in a theme. Checked by `make pixels`
  (tests/hatch.sh), and by hand at scales 1, 1.25 and 1.75.
- **A revision block** above the title block (#45), as drawings keep one: the latest commits of the git
  repository the focused window works in, REV / DESCRIPTION / DATE / BY, the repository named in the
  header; a terminal's directory is its shell's. Off by default (`[title] revisions = true`): commit
  subjects are then on screen. `draftpaper revisions` prints what it letters; git runs there with nothing of
  the repository's own configuration able to start a program. Checked by `make pixels` (tests/revisions.sh).
- `make` always hands the programs to cargo: once built they were never rebuilt, and `make pixels` could
  test a stale build.
- **A scale bar** in the bottom margin (#44): 0 to 50 mm (as many whole centimetres as its zone holds),
  true to size on each screen, the first centimetre ticked by the millimetre, drawn by the compositor
  on whole pixels and lettered by the shell. A display that reports no size letters NTS (not to scale)
  instead. `[title] scale = false` takes it away. Checked headless by `make pixels` (tests/scale.sh).
- **A website**, https://draftpaper.este.systems/, with a theme generator (the settings' scheme builder,
  in the browser) and a wallpaper generator (the sheet of any theme at any screen's size). Linked from the
  README and the man page.
- toml 1.1 and png 0.18 (#40). Screenshots are written with fast compression, png 0.18's default being slower,
  and under a hidden name until whole, so nothing reads half a picture.

## 0.5.1 (2026-10-17)

A security fix (#41), and the login screen's fingerprint glyph lit.

- **Security: tmux restore no longer types control characters.** A program saved in a tmux pane is typed
  back into its shell, and a file name holding control characters (Ctrl+C, Enter) could run a command of
  its own the next time the server started. A word with one is no longer typed: the pane gets its shell.
- The dependencies are checked against the RustSec advisories on every change and weekly (GitHub
  Actions); Dependabot proposes updates. SECURITY.md says how to report a vulnerability privately.
- **The login screen's fingerprint glyph lights up** while the reader listens: it stayed faint (Qt's curve
  renderer does not repaint a stroke colour changed once drawn; the glyph is now the ink, faint by its
  opacity).
- The README's pictures are new, in Soft Glow, with the settings' wallpaper page among them.

## 0.5.0 (2026-10-17)

The session comes back as it was left, and a lock screen of Draftpaper's own.

- **The session comes back** after a logout or a reboot (#38). Every application open at the end starts
  again at login, by its desktop entry or else its command line, and each window goes back to its
  workspace and zones, floating, fullscreen or parked in its cell. Terminals start in the directory their
  shell was in. The session is written to `~/.local/state/draftpaper/session.json` every few seconds, once
  it has held (the windows that close on the way out are never written), and by Mod+Shift+e. The opening
  waits for the windows a moment before it reveals the desktop. `[session] restore = false` turns it off;
  `skip` leaves applications out.
- **The applications as they were left** (#38). A terminal runs again what it ran (an editor, a pager, a
  monitor, tmux; never a build, a script or ssh), its shell still there when it ends. An application
  started with documents opens them again. `draftpaper apply` switches on each application's own restore
  (`[session] apps`, true by default): Firefox and Chromium their windows and tabs; Emacs its files, each
  at its place (desktop-save-mode, saved as it goes, so a logout loses nothing); Neovim the cursor in every
  file and, started with no file, the session it last had in that directory; tmux its sessions, windows
  and panes, each pane in its directory and running its program again, across a reboot
  (`draftpaper tmux`). `apps = false` takes these files back out.
- **The fingerprint reader keeps listening** on the login and lock screens: when it gives up (a finger
  not recognised, or its timeout) and nothing is typed, it listens again, as often as it takes; a finger
  not recognised shakes the field. Typing a password still works at any time. Escape on the login screen
  while the reader listens goes back to the name once it gives up.
- **The lock screen looks like the login screen** (Mod+Shift+x, idle, before sleep): the sheet blurred,
  nothing of the windows, and one field with the fingerprint reader listening. Start typing and the
  password's dots follow its glyph. It is Draftpaper's own
  (`draftpaper lock`, `qml/lock.qml`, PAM service `draftpaper-lock`) in place of swaylock, which stays a
  fallback should it not lock within 5 seconds.
- **The login and lock screens** are one field in the middle of the sheet and no words (no heading, no
  messages): the glyph lit while the reader listens, the dots, a shake for a refusal
  (`qml/SignInField.qml`). The sheet is blurred inside its frame only; the margins stay sharp.
- **The login screen no longer gets stuck** when Escape is pressed while the fingerprint reader listens:
  greetd answers nothing until PAM is back, so nothing is sent to it then; and should greetd fail or
  not start the session once the name slides, the field comes back for another try.
- With a wallpaper, the frost under the title block shows only once the shell letters the block (as its
  rules): no frosted patch on the login and lock screens.
- **The settings' wallpaper page scrolls**: every picture in the folders, not only the first few rows; a
  wheel (or a touchpad) scrolls a row at a time, the arrows keep the picked one in view, and the heading
  says which show ("1–10 OF 41").
- The session no longer exports RUST_LOG to every program started in it: the compositors' log level is
  DRAFTPAPER_LOG. The compositor prints its Wayland socket's name whatever the level.
- **A better fingerprint glyph** on both screens: Tabler Icons' fingerprint (MIT), drawn as strokes at the
  weight of the sheet's rules, in place of the ellipses drawn before.

## 0.4.0 (2026-10-17)

A login screen of Draftpaper's own, and no flicker between it and the session.

- **Login screen** (experimental, under greetd; #37). `draftpaper-greeter` draws your sheet, blurred, with
  a sign-in card on whole zones in the middle of it. With your name known, a fingerprint reader
  (pam_fprintd) listens from the start; a password typed meanwhile is given when PAM asks for it. Signed
  in, your name slides from the card into the title block's DRAWN cell, and the session starts on the
  same VT. Setup: the README's *Login screen*.
- **No flicker, either way.** The compositor takes the displays over as they are, in their mode, instead
  of resetting them; leaving, it keeps its last frame on screen. The next one opens on that very
  picture, read back from the display, and reveals the desktop under it once the shell has lettered
  everything, the blur fading to nothing. After any display manager the session opens the same way.
- **Hardware-decoded video** (Firefox on Intel) that the display refuses to show directly no longer
  blanks or freezes the screen: the compositor composes everything instead.
- **Sound after logging in again** is checked: when only the dummy output is left, WirePlumber is
  restarted and the volume restored.
- The session no longer opens a terminal at the start. The compositors log to the journal
  (`journalctl -t draftpaper -t draftpaper-greeter`). `draftpaper-compositor grab` (as root) reads back
  what each display shows. The IPC's outputs list their layer surfaces; new requests `ready` and `blur`.
- `make screenshots` draws the README's pictures headless.

## 0.3.0

Windows on the zones, a launcher, the sheet to the pixel. Not released on its own: its changes come with
0.4.0.

- **Free-form tiling.** Every window takes a rectangle of whole zones of its own, its edges on the ticks.
  It moves a zone at a time keeping its size (swapping places with a window of the same rows or
  columns) and resizes a zone at a time (Mod+Ctrl+arrows or h/j/k/l). A new window takes the largest
  free rectangle, else half of the focused one; a closed one leaves its zones to the windows that line up
  with it. A window alone moves and resizes in zones too.
- **A launcher of its own** (Mod+d; #34): the applications as a register in the frame's top-left corner,
  fuzzy, the most used first. `draftpaper menu` offers any list of lines in it, for scripts. fuzzel stays
  an alternative.
- **The sheet to the pixel**, measured once in the display's own pixels: the frame, the zones, the title
  block (on whole zones now) and the tiles agree at every scale (1, 1.25, 1.5, 1.75, 2), checked pixel
  by pixel (`make pixels`, headless).
- The top and bottom margins number their zones, as the sides letter theirs.
- **A magnifier** on the pointer (Mod+z): each pixel a block, the one under the pointer outlined.
- **Settings on the board** (Mod+t): the themes as drawings, a colour scheme built from eight choices
  with every word legible (4.5:1 or more; `draftpaper theme check`), and a wallpaper page: how a picture
  is mounted, its brightness, a wash of paper, blur.
- **Displays**: kanshi, wlr-randr and wdisplays arrange them (wlr-output-management); a change of scale
  redraws the zones at once.
- **Window rules** (`[[rule]]`): float, sticky, placed in a corner, no focus, by app id, title or a
  browser's picture-in-picture. Windows of one fixed size float at their size.
- Touchscreens; clicks on the shell's surfaces focus them.
- `draftpaper-as USER [COMMAND]`: a program, or a shell, as another user on the session.
- The `draftpaper` command is Rust, no longer Python.
- A cheatsheet: every key, click, command and file on one page (`docs/cheatsheet.md`).

## 0.2.0

Draftpaper becomes a Wayland compositor of its own (Smithay), started as the "Draftpaper" session from
the display manager; the i3 edition is dropped (#31). Not released on its own.

- The sheet's margins, title block, side cells and notifications are drawn by the shell over the
  compositor's IPC; merged cells, Nerd Font glyphs.
- Parking in the side margins, animated; focus follows the mouse.
- A wallpaper in the frame, with the paper's or the picture's colours in the margins.
- Screenshots, screen sharing (xdg-desktop-portal-wlr), clipboard managers, fractional scaling,
  linux-dmabuf, Xwayland.
- Locking (swaylock) and idle (swayidle): the screens off after 15 minutes, locking before sleep on
  request.
- Volume and brightness keys; the volume kept from one session to the next.
- A theme picker with a live preview; a theme also colours alacritty, foot, tmux, Neovim, Emacs, GTK,
  Qt and Firefox (#35).

## 0.1.1 (2026-10-14)

The i3 edition, frozen since.

- The sheet redrawn when screens are added, removed or change mode (#4).
- Parked windows kept in step with i3 (#5).
- Several custom cells: the first in the title block, the others in the bottom margin (#6).
- Blueprint, a dark theme (#7).
- Super+Left/Right cycling in its own include, `draftpaper-cycle.conf` (#8).
- IBM Plex Mono recommended; DejaVu Sans Mono without it.

## 0.1.0 (2026-10-14)

The first release: the drafting sheet behind i3 on X11 (paper, frame, zones), live margins and title
block, parking in the side margins, the Soft Glow theme.
