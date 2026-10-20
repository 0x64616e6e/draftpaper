# Extensions

Things to put on the sheet that are yours rather than the desktop's. Copy one into
`~/.config/draftpaper/extensions/`, edit the few lines at the top of it, and call it up by the name of
its file:

```sh
cp extensions/disks.toml ~/.config/draftpaper/extensions/
draftpaper schedule disks       # up, where the equipment schedule goes
draftpaper schedule hide        # away again
```

Or reach for it without typing: **Mod+i** goes through the schedules in turn — the three the sheet comes
with, then yours, then none — and **Mod+Shift+i** picks one straight out of the launcher's register,
which is the way to do it once you have more than a couple. `draftpaper schedule list` names them all.
A new extension joins both the moment its file appears. `cycle = false` in its file keeps it out of
Mod+i's turn while leaving it in the register and answering to its name, which is what to reach for once
the turn has more stops in it than you want to go past.

Nothing here is installed or runs by itself. They are files to take a copy of.

| | what it says | wants |
|---|---|---|
| `disks.toml` | the filesystems, fullest first, each one's use ruled as a bar | nothing |
| `ci.toml` | the repository's last runs on GitHub, and how many failed | `gh`, logged in |
| `departures.toml` | aircraft off an airport in the last six hours | the network |
| `calendar.toml` | today, from any calendar you can fetch as iCalendar | the network, a feed address |
| `containers.toml` | what is running under Docker or Podman | one of them |
| `updates.toml` | the packages waiting, and what they are coming from | Debian or a relation |

`disks.toml` is the one to read first. It needs nothing installed, and it is the shortest example of
every part of the shape: the columns, the one that takes what is left, and a bar ruled from each row's
share.

## What an extension is

A program that prints what it has to say, and a file saying how to run it:

```toml
interval = 60                      # seconds; it is not run oftener than this
cycle = false                      # optional: keep it out of Mod+i's turn (still picked, still by name)
command = ["sh", "-c", '''
  ... whatever prints the JSON ...
''']
```

Use `'''` rather than `"""` for the command. A TOML string in double quotes reads backslashes as escapes,
which fights with nearly every shell and with `jq`'s `\(...)`; in single quotes the script is passed
through exactly as written.

Print **a line** and it is a cell — the first takes the title block's last cell, the rest a zone each
along the foot. Print **an object with `text` in it** and it is still a cell, with everything a slot can
say: a glyph, the words to fall back on where no Nerd Font is installed, the alert ink. Print **an
object with `rows` in it** and it is a schedule, called up by name.

```json
{"title": "DEPARTURES", "meta": "PLATFORM 4",
 "columns": [{"name": "TIME", "mm": 18}, {"name": "FOR", "mm": 0}, {"name": "DUE", "mm": 24, "right": true}],
 "rows": [{"cells": ["09:14", "Cluj-Napoca", "on time"]},
          {"cells": ["09:52", "Brasov", "4 min late"], "note": "awaiting a platform"}]}
```

A column of `"mm": 0` takes whatever is left over, and there should be exactly one of them. `"note"` is
lettered small under its row. `"bar": N` rules column N from each row's `"share"`, 0 to 1 — `disks.toml`
does this. `"heavy": true` on a row puts a heavier rule above it, for grouping.

`"ink"` on a row says how it should read, not what colour to use:

| | for |
|---|---|
| `"alert"` | something wrong: a run that failed, a disk nearly full, a train cancelled |
| `"faint"` | something said quietly: a row that is over, or stale, or does not matter yet |
| `"strong"` | the one row to look at first |

Leave it out and the row is lettered in the sheet's own ink. Which colour each of those is belongs to
the theme, and changes with it; an extension that named a colour would be the one thing on the sheet
that did not follow the paper.

An extension supplies words and the sheet sets them. It cannot letter in a face of its own, an ink off
the theme's palette, or anywhere but the slot it was given, because all it hands over is an entry of the
document both renderers already read (`docs/sheet-content.md`).

## How it is run

Never while the sheet waits. When an extension's interval is up the sheet asks again **in the
background** and goes on being drawn from what the last asking left behind, so the answer appears one
reading later — a few seconds on something read every five, nothing at all on something read every
minute. That is why `ci.toml` and `departures.toml` can talk to the network at all: nothing you put on
the sheet is allowed to be the reason it stops being drawn.

Two things follow from it. The first time you call an extension up it will be empty, and right the time
after. And a command that takes longer than twenty seconds is killed, with the last answer left standing.

A schedule is only asked while it is the one on the sheet. A cell is asked whenever the sheet is drawn,
because a cell is always on it; a schedule that is not out is not asked at all, so the ones here that
talk to the network cost nothing while they are put away.

What it printed is kept in `~/.cache/draftpaper/extensions/`. Editing a command does not take effect
until its interval is up; delete the file to see it at once.

## Writing one

Print a whole schedule even when there is nothing to say — `{"title": "...", "rows": []}` — rather than
nothing at all. An empty schedule stands on the sheet with its heading and its columns, which reads as
"nothing today"; printing nothing reads as the sheet having ignored you. Each of these does that when
the tool it wants is missing.

Try it without the sheet first:

```sh
sh -c '...your command...' | jq .
draftpaper sheet NAME eDP-1 | jq '.displays."eDP-1".schedule'
```
