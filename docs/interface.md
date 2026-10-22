# The interface: proportion, lettering, line weight

Status: research and a proposal (2026-10-22). Nothing here is built yet. Against 0.7.5.

The occasion was a defect — the settings' eleventh page card runs 4.7 mm past the frame onto the
bottom margin — but the defect is a symptom. The panel has no proportion system, so it had no way to
know how many cards it could hold, and no way to look composed when it held them.

## The diagnosis, in three numbers

Counted in `qml/Settings.qml`:

| | how many | what they are |
|---|---|---|
| lettering sizes | **10** | 0.62, 0.65, 0.68, 0.70, 0.75, 0.80, 0.85, 0.90, 1.0, 1.1 × 2.8 mm |
| line weights | **6** | 0.15, 0.2, 0.3, 0.6, 0.7, 0.8 mm |
| spacings | **14** | 2, 2.5, 3, 4, 5, 6, 7, 8, 12, 16, 34, 40, 58, 70 mm |

The ten lettering sizes run from 1.74 mm to 3.08 mm. The largest is 1.77× the smallest — **ten steps
inside a single √2 interval**. That is the exact inversion of a scale: many sizes, no contrast. Two
sizes 4% apart are not a hierarchy, they are a wobble, and the eye reads a wobble as a mistake.

This is the documented failure mode. The usability literature on "why does this look amateur" converges
on three causes, and the panel has all three:

- **Everything at the same visual volume.** Nothing reads as primary when nothing differs enough in
  size, weight or space ([NN/G](https://www.nngroup.com/articles/visual-hierarchy-ux-definition/)).
- **Spacing decided one element at a time** rather than from a system — named repeatedly as the main
  reason an interface looks amateur
  ([spacing systems](https://mohitphogat.medium.com/the-spacing-system-that-makes-every-ui-look-more-intentional-52251ae61c2f)).
- **Borders used as a crutch for hierarchy.** NN/G's own guidance is that where whitespace is not cue
  enough you *may* add a border, but it clutters, so use it sparingly. The settings use eleven of them
  for one list.

And the common advice on the other side: hierarchy lives in the **contrast between levels**, not in any
level's absolute size, and three tiers of type is usually the most a dense interface should carry
([typography hierarchy](https://medium.com/@lokaydouglas/typography-hierarchy-in-ui-design-ee7f4f2581d0)).

## Why a drafting sheet does not have to invent any of this

Draftpaper draws a technical drawing. Technical drawing is a design system that has been refined for a
century, is written down as standards, and is already the product's idiom. We have been ignoring it and
hand-rolling a worse one.

**One ratio governs the whole system, and it is √2.**

- **Sheet sizes.** ISO 216's A series has a √2 aspect, which is why halving a sheet preserves its shape.
- **Lettering heights.** ISO 3098 nominal heights are 1.8, 2.5, 3.5, 5, 7, 10, 14, 20 mm — a 1:√2
  progression, "the same formula used for paper sizes and line weights"
  ([lettering heights](https://cadsetterout.com/drawing-standards/lettering-heights/)).
- **Line widths.** The ISO 128-20 series is 0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1, 1.4, 2 mm — again √2
  steps ([line weights](https://cadsetterout.com/?p=316),
  [IS 10714-20](https://law.resource.org/pub/in/bis/S01/is.10714.20.2001.pdf)).

So the three things an interface needs a scale for — type, rule weight, spacing — are the three things
a drawing already scales, by one ratio, in millimetres. The sheet is measured in whole millimetres from
the frame's corner already (docs/wayland-design.md); this is the same doctrine carried into the panel.

**A drawing's hierarchy is three tiers and it is assigned by role.**

Lettering is not sized by taste but by what it is. The recommended minimums are given per sheet size in
three columns — *notes and dimensions*, *titles*, *headings* — 2.5 / 3.5 / 5 mm on an A1 through A3
sheet, 1.8 / 2.5 / 3.5 on A4. Three roles, three sizes, a √2 apart each. Exactly the "at most three
tiers" the interface literature arrives at independently.

**Line weights come in groups of two, in a 2:1 ratio.**

A drawing picks a *line group* and uses two weights from it: wide for visible outlines, narrow for
dimension lines, projection lines and leaders. ISO describes wide, narrow and extra-wide in a **4:2:1**
relationship. Six weights, as the panel has, is not a drawing — it is six people's opinions.

**A drawing's lists are not boxes.**

A parts list, a revision table, a legend: these are lettering in columns, separated by *rules*, usually
one heavy rule under the heading and thin rules between groups — not a border around every row. The
legend we built for the keyboard already does this correctly (`qml/Legend.qml`): a heading, one heavy
rule, then rows with no boxes at all. The settings panel is the one place that went the other way.

## The system

Three ladders, each a √2 step, each landing on whole millimetres so the existing snapping is unchanged.

### Lettering — three sizes, assigned by role

| mm | role | where |
|---|---|---|
| **5** | heading | the page's name, once, at the top |
| **3.5** | title | a row's name; a page's name in the list |
| **2.5** | note | values, units, the note under a row, the page numbers |

Nothing else. A size is chosen by asking what the text *is*, never by eye.

### Line weight — one group, 2:1, with a third for the frame

| mm | role |
|---|---|
| **0.5** | the rule under a heading; the mark on the current row |
| **0.25** | every other rule: column rules, row separators |
| (1.0) | reserved — the frame itself, which the compositor already draws |

Two weights in the panel. A third exists only because the sheet's frame already uses it.

### Spacing — one ladder

**2, 3, 4, 6, 8, 12, 16 mm** — each step ×1.4, every value a whole millimetre.

| mm | role |
|---|---|
| 2 | between a label and its value |
| 3 | between rows in a group |
| 4 | a row's own height padding |
| 6 | between groups |
| 8 | the gutter in from the frame |
| 12 | between the page list and the page |
| 16 | reserved for the heading's own space |

Fourteen spacings become seven, and a gap is chosen by what it separates.

## What this fixes, concretely

**The overflow has a cause and the cause goes away.** The page list is eleven bordered cards at a 16 mm
pitch = 176 mm, against a frame 177.5 mm deep with 8 mm of gutter — it was always going to run off, and
nothing in the code knew. As lettering with a mark instead of boxes, the pitch is **8 mm**: eleven pages
take 88 mm, half the frame, and the list has room to roughly double before anyone has to think again.
A bound still belongs in the code, but it stops being load-bearing.

**Eleven borders become one rule.** The current page is marked the way a drawing marks the thing it is
pointing at: a short heavy tick in the gutter and full ink on the word, against faint ink for the rest.
Contrast does the work that eleven rectangles were doing badly.

**"SETTINGS · 1" eleven times becomes "SETTINGS" once.** The word is repeated on every card today. A
drawing letters its title once, in the title block.

## Open questions

1. **Whether the page numbers stay at all.** They are not bound to anything — no key reaches page 7.
   If they stay they should become real: the digits as the keys that reach the pages, which is the
   keyboard work's business (docs/keyboard.md) and an argument for keeping them.
2. **Whether the client ever draws the settings.** It does not today (`sheet/` has no settings), so
   this is the one part of the sheet with a single renderer. Everything proposed here is rectangles
   and text, so the client *could*, and the ladders above are the document's kind of numbers — they
   would belong in `content::` if it ever does.
3. **The rest of the panel.** This covers the page list and a row. The colour-scheme cards, the
   wallpaper grid and the displays diagram each have their own geometry and are not yet measured
   against these ladders.

## Related

docs/design-research.md surveys the prior art — Ulm, Swiss, Constructivist, Japanese, Isotype, Tufte,
NeXT and Apple, the automotive glance literature — and the perception layer underneath them
(preattentive channels, Gestalt grouping, optical alignment, colour relativity). It exists because the
system below turned out to be necessary and not sufficient: the first mockup drawn from it was
arithmetically right and badly composed.

## Sources

- [NN/G — Visual Hierarchy in UX](https://www.nngroup.com/articles/visual-hierarchy-ux-definition/)
- [Typography hierarchy in UI design](https://medium.com/@lokaydouglas/typography-hierarchy-in-ui-design-ee7f4f2581d0)
- [The spacing system that makes a UI look intentional](https://mohitphogat.medium.com/the-spacing-system-that-makes-every-ui-look-more-intentional-52251ae61c2f)
- [Technical drawing lettering heights (BS EN ISO 3098)](https://cadsetterout.com/drawing-standards/lettering-heights/)
- [Line weights and line groups (ISO 128-20)](https://cadsetterout.com/?p=316)
- [IS 10714 (Part 20) — identical to ISO 128-20](https://law.resource.org/pub/in/bis/S01/is.10714.20.2001.pdf)
- [Technical lettering](https://en.wikipedia.org/wiki/Technical_lettering)
- [Modular scale and vertical rhythm](https://www.bounteous.com/insights/2018/03/26/what-font-are-vertical-rhythm-and-modular-scale/)
