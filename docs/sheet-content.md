# The sheet's content: one account of what the drawing says

Status: all five stages built, 2026-10-20. The sheet is lettered by the shell again
(`[sheet] lettering = "shell"`); the client in `sheet/` stays in the tree and still passes its tests.

## Why

The sheet is a drawing with words on it. The lines are the compositor's — the paper, the frame, the
dots, the zone ticks, the cells of the title block, the scale bar's rules — computed per pixel in a
fragment shader and published, in physical pixels, over the socket. Everything else on the sheet is
text set into places the compositor has already measured.

Which words those are is currently decided **twice**: once in QML, for the shell, and once in Rust, for
the client. The two accounts are meant to agree and do not. Over two days of running the client as the
sheet's lettering, fifteen defects were found. Sorting them by where they lived:

| | |
|---|---|
| the Wayland plumbing — surfaces, buffers, scale, redraw bookkeeping | 4 |
| **two accounts of what the sheet says** | **10** |
| the font stack | 1 |

The second row is the one that matters. The battery cell read `-` for two days because the shell takes
the charge from UPower and the client's reading never carried one. The profile read `performance` where
the shell reads `perf`. The network read `-` where the shell reads `offline`. Every reading lost the
glyph in front of it. The clock's size rule was read off the measured cell instead of the table, so
eleven cells were lettered at the clock's size. None of these are rendering faults. They are two
programs disagreeing about what the drawing says, and no test could see the disagreement because each
program was tested against itself.

A renderer is a six-week decision. **What the sheet says is the part that should outlive it.**

## What it is

One JSON document, produced in one place, describing everything lettered on the sheet — and nothing
else. Both renderers read it. Neither decides anything.

Three rules settle what goes in it:

**It carries no positions.** Where a title cell sits, how wide a zone is, where the scale bar's rules
fall — all of that is the compositor's and is already published. The document says *which slot* and
*what it says*. This is what keeps the millimetre grid and the pixel snapping in one place, and what
lets a second renderer exist at all.

**It carries no colours.** An entry names a role — `ink`, `faint`, `strong`, `alert` — and the theme
resolves it. Otherwise a theme change has to find every producer, and an extension can letter something
in a colour that is not on the sheet's palette.

**It carries only what cannot be worked out from the geometry.** The columns' numbers, the rows' letters
and the scale bar's figures are pure functions of the zones and marks the compositor publishes, and they
stay in the renderer. Putting them in the document would be a second source for something that already
has one.

## The shape

```jsonc
{
  "version": 1,
  "displays": {
    "eDP-1": {
      "title": [ Text, ... ],        // twelve, in the order the compositor measured the cells
      "top": {
        "workspaces": [ { "number": 3, "focused": true, "marks": ["f0239"] } ],
        "notice": Text,              // absent when nothing has come in
        "waiting": 2,                // how many are behind it
        "status": [ Text, ... ]      // what wants saying now: a reading past its alarm, a level just moved
      },
      "bottom": {
        "start": Text,               // Bluetooth, in the first zone
        "cells": [ Text, ... ],      // a zone each: the config's [[cell]]s, and later the extensions'
        "end": [ Text, ... ]         // the screenshot, the theme's name
      },
      "sides": [ { "cell": 3, "text": "…", "action": "…" } ],   // the windows parked, by parking cell
      "schedule": Schedule           // only on the display the compositor says carries one
    }
  }
}
```

`Text` is the whole of the sheet's vocabulary:

```jsonc
{
  "label": "POWER",            // the title block's cells; absent elsewhere
  "text": "B mains",     // what is lettered, glyphs and all
  "words": "mains",            // what to letter where they cannot be drawn; absent = `text` is all there is
  "ink": "alert",              // ink, faint, strong, alert; absent = ink
  "large": true,               // the clock, and only the clock
  "action": "graph.power",     // what a click does: `draftpaper graph power`
  "action_right": "theme.next"
}
```

The glyphs sit inline in `text` rather than in a field of their own because two cells carry several of
them — the processor and the memory in one cell, the three temperatures in another — and a single mark
has nowhere to put the rest. One string and one fallback covers both, and the rule is the same for every
entry: **letter `text` where the face has every character in it, else `words`.**

`Schedule` is what the three schedules already are, in both implementations:

```jsonc
{
  "title": "PROCESS SCHEDULE",
  "meta": "BY CPU · LOAD 0.73 · 112 PROCESSES",
  "columns": [ { "name": "NO.", "mm": 8, "right": false } ],
  "rows": [ { "cells": ["01", "firefox", "4210", "4.0", "723 MB", "1:02:50"], "share": 0.04 } ],
  "bar": 3,               // the column ruled as a bar, from `share`; -1 for none
  "note": -1              // the column lettered as a note under its row; -1 for none
}
```

Four things worth saying about that shape, because each of them is a defect from this week:

- **`mark` and `words` are both carried, and the renderer chooses.** Only the renderer knows whether a
  Nerd Font is installed and whether it has that codepoint. The producer cannot know, and the shell and
  the client answered it differently — which is how the client came to letter every reading bare.
- **`large` is stated, not inferred.** The client worked it out from the height the compositor measured
  the cell at; the block's proportions stretch the top row past the threshold, so eleven cells were
  lettered at the clock's size. A fact that is known at the source belongs in the document.
- **`action` is a name, not a function.** QML held closures, the client held an enum, and the two drifted
  — the client's PROJECT cell toggled a schedule the shell opened differently. A name the renderer hands
  back to `draftpaper` is the same in both.
- **The schedule is in the document even when it has no rows.** A schedule with nothing in it is still a
  schedule, with a heading and columns. The client dropped it entirely and the sheet looked ignored.

## Who makes it

`draftpaper sheet` prints the document; `draftpaper::content::document()` builds it. The formatting that
lives in `sheet/src/title.rs` and the three builders in `sheet/src/schedule.rs` **move** into
`cli/src/content.rs` — they are not copied, and when the move is done neither renderer formats anything.

The client calls the function. The shell polls the command on the cadence it already polls
`draftpaper readings`, and that poll replaces it rather than adding to it: the same one process every
five seconds, carrying the whole sheet instead of the sensors alone.

What the shell gets back is smaller than what it has now. `TitleBlock.qml`'s twelve-row table of
labels, values, closures and alert flags becomes `model: doc.title`, and `shell.qml`'s dozen text
properties — `powerText`, `batteryText`, `profileText`, `loadText`, `networkText`, `tempText`, the ramps
and the glyph helper — go away entirely.

### What it does not cover yet

- **The graph.** It is a plot, not lettering: lines from the metrics store. It stays a renderer feature.
- **The tray.** Icons from `SystemTray`, which has no text to carry. It stays the shell's, as it is now.
- **The notifications.** They belong to whichever process took the name on the session bus, which is
  the renderer, and a polled snapshot cannot see them. The formatting is the one place a disagreement
  could still hide; closing it means a daemon that holds them, which is a bigger decision than this.
- ~~The config's own `[[cell]]`s.~~ In it since stage 5, with the extensions: each is run no oftener than
  its own interval asks, because what it last printed is kept beside the theme with the second it was
  printed in. That is also what let the extensions in at all.
- **Levels that are lit for four seconds.** The volume and the brightness are lettered for a moment
  after they move, which is faster than a five-second poll. For now the renderer keeps watching them and
  overlays them on `top.status`. If that proves to be where the next disagreement lives, the document
  gets pushed over the socket instead of polled, and this goes in it.

## Stages

Each one leaves the sheet working and the tests green.

0. **The document, and the client reading it.** ✅ Built 2026-10-20. `cli/src/content.rs`, `draftpaper sheet`, tests against a
   known reading. The title block only — twelve cells is enough to settle the shape, and the formatting
   moves out of `sheet/src/title.rs` rather than being written again.
1. **The margins and the schedules.** ✅ Built 2026-10-20. The rest of the shape; the three schedule
   builders move out of `sheet/src/schedule.rs`, and the marks a workspace carries — which the client
   never had — come with them.
2. **The client letters from it.** ✅ Built 2026-10-20. `sheet/src` keeps the font, the drawing and the
   placing, and decides nothing about words. Its content tests moved to the CLI with the code, and the
   `Theme` it reads lost `project`, `drawn` and `label`: nothing in the client reads them any more.
3. **The shell letters from it.** ✅ Built 2026-10-20. The poll changed, and the twelve-row table, the
   dozen text properties and the three schedule components went. The second account no longer exists:
   the ten defects above are impossible rather than fixed, and PROJECT now reads the same on both.
4. **The diff harness as a test.** ✅ Built 2026-10-20, compositor/tests/lettering.sh. One document, both
   renderers, headless, screenshots compared band by band and cell by cell — not pixel for pixel, which
   would fail on every glyph edge, but on where the ink falls across each region. It found, on its first
   run, that the Nerd Font glyphs come out smaller in the client than in the shell: the same character,
   asked for at the same size, out of the same file, whose metrics the two agree on exactly. That is
   still open.
5. **Extensions.** ✅ Built 2026-10-20. A program in `extensions/NAME.toml`, or a `[[cell]]` of the
   config, contributing entries to the title block's last cell and the foot's zones. The shape was
   settled by then, and an extension that can only supply words cannot break the grid, the face, the ink
   or the pixel snapping. An extension that prints a schedule rather than a line is one, called up by
   the name of its file.

The three schedules the sheet comes with stay in process and are not extensions. They want what an
extension has no way to ask for — the displays as the compositor publishes them, the rows last read when
every window is parked — one of them samples the processes over half a second, and they are what the
sheet is rather than what is added to it: a mistake in a file should not be able to empty the title
block, and the second an extension is given before it is killed would do exactly that.

## Testing

The document is plain JSON built from a reading, so it is tested the way `readings` is: a fixed input
and an expected document, including the awkward cases that cost a day each this week — no battery, no
network at all, a profile with a glyph and a profile without, a schedule with no rows.

Stage 4 is the one that catches what unit tests cannot. Until the shell and the client letter the same
document there is nothing to diff; after stage 3 there is, and the comparison is exact rather than a
matter of someone noticing that the lettering looks wrong.

## What this is not

It is not an architecture change. The compositor keeps its shader, the renderers keep their surfaces,
Quickshell keeps the sheet. It is one account of what the drawing says, in the place where both
renderers can read it, so that the choice of renderer stops being a choice about correctness.

If the sheet later moves into the compositor, the document is what it renders. If it stays with
Quickshell for good, the document is what the shell renders and what extensions write. That is the
point: it is the part that does not have to be decided now.
