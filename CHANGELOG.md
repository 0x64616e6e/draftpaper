# Changelog

What changed in each release, newest first. The Debian package's own record, with every detail, is
`debian/changelog`.

## Unreleased

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
