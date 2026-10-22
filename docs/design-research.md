# Prior art: what the design traditions actually hold, and which side Draftpaper is on

Status: research (2026-10-22). Nothing here is built. Companion to docs/interface.md, which sets out
the proportion system; this is the part that system could not supply.

## Why there is a second document

docs/interface.md fixed an arithmetic problem — ten lettering sizes, six line weights, fourteen
spacings — with a proportion system, and the first mockup drawn from it was still not good. That is
the honest lesson and it is worth writing down:

**A proportion system is hygiene. It removes the obviously wrong. It does not produce beauty.**

What it cannot decide: where the weight of a composition sits, what to leave out, whether empty space
is active or residual, which of two defensible answers feels inevitable. Those decisions are what the
traditions below argue about — and they *do* argue. They are not a single body of wisdom to average
into a formula. They disagree, sharply, and the useful work is knowing which side a product is on.

Draftpaper has already picked sides without saying so. This document says so.

---

## I. The traditions

### The technical drawing — covered in docs/interface.md

One ratio (√2) across sheet sizes, lettering heights and line widths; three lettering tiers assigned
by role; line weights in a group of two at 2:1; lists as lettering and rules, never a box per row.
This is the product's own idiom and the baseline everything below is measured against.

### Ulm, Braun, Rams — reduction as a method, not a style

Rams formed at the intellectual edge of the **Hochschule für Gestaltung Ulm**, the Bauhaus's successor,
among Hans Gugelot, Fritz Eichler and Otl Aicher, whose emphasis was on *systems, clarity and social
responsibility* ([Rams](https://en.wikipedia.org/wiki/Dieter_Rams)). The method was systematic design
tied to form and function; products were **reduced to the essentials and stripped of distracting
elements**. "Weniger, aber besser."

The thing usually missed: reduction at Braun was a *consequence of systematising*, not an aesthetic
preference for sparseness. You reduce because the system tells you which parts are redundant. That is
the same order of operations docs/interface.md proposes — count first, then remove.

**For Draftpaper:** the ladders come before the taste. Every mark must answer "what is this for",
and the answer "it looks better" is not one.

### Swiss / Müller-Brockmann — objectivity, and the cost of it

Müller-Brockmann's grid is a deliberate **commitment to cultivating objectivity instead of
subjectivity**, with proportions and spacing balanced by strict mathematical ratios; *Grid Systems in
Graphic Design* gives exact instructions for systems of 8 to 32 fields
([review](https://theartbog.com/book-review-grid-systems-in-graphic-design-by-josef-muller-brockmann/)).

And the part worth quoting against ourselves: the strict emphasis on objectivity **later came to be
seen as cold**, which is why the style fell out of favour. That is precisely the failure of the first
mockup. A correct grid, coldly executed, is not neutral — it reads as indifferent.

**For Draftpaper:** we are a Swiss-grid product by construction (whole millimetres, a frame, zones).
The risk is not disorder, it is coldness. Warmth has to come from somewhere else — see Constructivism
and Japan below.

### Russian Constructivism — asymmetry and dynamism as the counterweight

Rodchenko and Lissitzky worked in **bold lettering, stark planes of colour and diagonal elements**, on a
**stripped-down palette**, with serif and sans combined into letterform compositions
([RA](https://www.royalacademy.org.uk/article/five-things-graphic-designers-owe-to-russia),
[Art Story](https://www.theartstory.org/movement/constructivism/)). Lissitzky's engineering and
architecture training gave the work a *linear, logical quality* — he is the bridge: the same rigour as
Swiss, arranged for tension rather than repose.

The lesson is not to put diagonals on a drafting sheet. It is that **a composition needs a dominant
element and an off-centre weight**, and that a restricted palette makes one accent enormously powerful.

**For Draftpaper:** the sheet has exactly one accent colour (the frame's red) and otherwise one ink at
varying opacity. That is already a Constructivist palette discipline. What the first mockup lacked was
the other half — a focal point. Everything was evenly weighted and nothing was *the* thing.

### Japan — emptiness as presence, and design for the body

**Ma (間)** is routinely mistranslated as negative space. The distinction the sources draw is that
negative space is a compositional leftover, while **ma treats emptiness as an active presence** — space
with intent ([Kadir Kara](http://kadirkara.com.tr/blog/japanese-design-philosophy-ma-wabi-sabi-mono-no-aware)).

Kenya Hara's **emptiness (空)** sharpens it: an empty vessel is not a negative state but *a capacity to
be filled*; MUJI's framing is "emptiness, not minimalism"
([Hara](https://medium.com/@chenxidu/complexity-to-simplicity-ccae63ad5678)). Minimalism removes until
little is left; emptiness arranges so that what is absent invites.

Naoto Fukasawa's **"without thought"** is design aimed at the body rather than the mind — built on
movements people already make without deciding to
([Fukasawa](https://blakecrosley.com/blog/design-philosophy-naoto-fukasawa)).

**For Draftpaper:** this is the exact diagnosis of the second mockup's lower half. The empty band below
the rows was *residue*, and I only made it a shape by putting the title block under it. The test for
every empty region on the sheet: is it holding something open, or did the content just run out?

"Without thought" is also the strongest existing argument for the keyboard work — the hold overlay
teaches the fast path *through the slow motion you already make* (docs/keyboard.md), which is the same
idea arrived at from ergonomics rather than from Lane and Cockburn.

### Isotype — Neurath and Arntz, and the rule about glances

Isotype's symbols were **simple and abstract as far as possible**, made repeatable, with a
"transformer" whose job was turning data into a sketch
([Isotype](https://en.wikipedia.org/wiki/Isotype_(picture_language)),
[Arntz](https://www.gerdarntz.org/content/gerd-arntz.html)). Two rules transfer directly:

- **Quantity is shown by repetition, not by scale.** Rows of identical icons, never one icon drawn
  bigger. Scaling a symbol makes it a different symbol.
- **The fourth-look rule.** Neurath: a picture should give its main point at a glance, with detail only
  on closer inspection — *"if you see more than the main points by the fourth look, the teaching-picture
  is bad"* ([Eye](https://www.eyemagazine.com/feature/article/talking-pictures1)).

**For Draftpaper:** the sheet is read in glances all day. The fourth-look rule is a usable acceptance
test for every screen we draw: name the one thing it says at a glance. If the answer is "it depends
where you look", it has failed before any measurement.

### Tufte — the ink, and how to layer without space

- **Data-ink ratio:** maximise the share of ink carrying non-redundant information; *erase non-data ink
  — frame boxes, gridlines, minor tick marks.*
- **Chartjunk:** ink that tells the viewer nothing new.
- **Layering and separation:** elements coexist in one space when separated by **value, weight, hue or
  transparency** rather than by spacing.
- **1 + 1 = 3:** two heavy elements placed together generate a third, unintended, visual noise.
- **Micro/macro:** the whole structure at a glance, detail rewarded on inspection — Neurath's rule
  again, from a different direction.
- Tufte qualifies much of this "within reason", and explicitly allows a place for *complexity,
  structure, density and even beauty* ([course notes](https://faculty.cc.gatech.edu/~stasko/7450/16/Notes/tufte.pdf)).

**For Draftpaper:** 1+1=3 names a bug in the current panel precisely — a 0.6 mm border *and* a bold
label *and* a filled background, all saying "this one is selected", producing mush instead of emphasis.
One channel per message.

### NeXT and Apple — depth as hierarchy, and its opposite

NeXTSTEP's look came partly from constraint: **1120×832 at 2 bits per pixel, four-level greyscale**
([NeXTcube](https://en.wikipedia.org/wiki/NeXTcube)). Four greys, and the interface's entire hierarchy
had to live in them. That is a useful provocation: our sheet has one ink and a handful of opacities, and
that is not a limitation, it is the NeXT budget.

Apple's current position is the opposite pole. A **material** is a translucent blurred layer separating
foreground from background; **vibrancy** pulls colour forward from behind it to enhance depth; materials
are to be chosen *by semantic meaning, never by the colour they appear to produce*
([HIG](https://developer.apple.com/design/human-interface-guidelines/materials)). And from WWDC25, the
line that matters most here: **hierarchy should come from layout and grouping rather than decoration.**

**For Draftpaper this is a fork, and we are already on one side.** Translucency, blur and shadow are
depth cues for a world of stacked panes. A drawing has no stack — it is one sheet, and everything on it
is *on* it. The legend's paper is opaque for exactly this reason (docs/keyboard.md: "a legend you can
read the window through is not a legend"). So: **no shadows, no blur, no elevation.** Separation comes
from value, weight and position, as Tufte describes and as a drawing has always done. The wallpaper's
blur is the one exception and it is a photograph being pushed behind the paper, not a UI material.

### Automotive instrument clusters — the glance budget

The HMI literature measures what Neurath asserted. Total eyes-off-road time is significantly longer for
visual-manual interfaces than voice
([Tandfonline](https://www.tandfonline.com/doi/full/10.1080/00140139.2021.1930197)); head-up displays
cause shorter distraction than instrument clusters or centre panels; better-grouped HMIs produce
**fewer glances**, though not shorter ones
([AAA](https://aaafoundation.org/research/human-machine-interfaces-and-vehicle-automation-the-effect-of-hmi-design-on-driver-performance-and-behavior/)).

That last finding is the interesting one: layout changed *how often* people had to look, not how long
each look took. Glance *count* is the thing design moves.

**ISO 15008** sets minimum legibility requirements for changeable visual information shown to a driver
of a moving car, deliberately independent of display technology
([ISO 15008:2017](https://www.iso.org/standard/62784.html)). It is the closest standard in existence to
what a drafting-sheet desktop is: an instrument surface read in glances while attending to something
else. It is paywalled; the numbers are not quoted here because we have not bought it.

**For Draftpaper:** the sheet's title block *is* an instrument cluster. The design goal is fewer
glances, not prettier ones — which argues for stable positions above all. Something that never moves can
be read with a saccade and no search. This is already doctrine ("nothing moves") and the HMI literature
is the evidence for it.

---

## II. The perception layer — the part that *is* quantifiable

This is the honest answer to "can any of this be expressed mathematically". Some of it can, and it is
the layer below taste rather than taste itself.

### Preattentive attributes — what the eye resolves before you look

Some channels are processed before focused attention, in roughly 200 ms and independent of the number
of distractors, at around 10 ms per item or better
([IAT 814](http://www.sfu.ca/siatclass/IAT814/Spring2009/Lectures/IAT814-week5A.pdf)). But the channels
are **not interchangeable**:

| channel | pops out | reads quantitatively |
|---|---|---|
| 2D position | yes | **yes** |
| length | yes | **yes** |
| hue | **strongly** | no |
| intensity / value | strongly | poorly |
| orientation | yes | no |
| shape, texture | weakly | no |

Hue has *higher priority than shape and texture* for popout, yet is poor for judging magnitude
([Iowa](https://homepage.divms.uiowa.edu/~luke/classes/STAT4580/percep.html),
[arXiv](https://arxiv.org/pdf/2301.00002)). The classic Treisman model is now treated as too clean —
these features keep guiding attention during exploration, not only in the first glance.

**For Draftpaper:** use position and length for anything that means *how much* (the load graph, the
schedule bars, the scale bar). Use hue only to mean *look here* — which is what the single red accent
already does. Never use hue to encode a magnitude.

### Gestalt — and why borders are the lazy answer

**Common region**: items inside a boundary are perceived as a group, and the boundary **overrides
proximity** ([NN/g](https://www.nngroup.com/articles/common-region/)). That is exactly why a box is so
tempting — it is the strongest grouping cue available, which is why it gets reached for first and why
interfaces end up full of boxes. But the boundary can be a border, a background, *or* a container, and
**too many borders is clutter; whitespace and proximity are usually enough**.

So the design rule is an ordering, not a prohibition: **proximity → alignment → value → common region.**
Use the weakest cue that works. Reach for a border only when the three cheaper cues have failed.

The second mockup is an application of exactly this: three columns held by alignment alone, no rules
between rows.

### Optical versus mathematical alignment

The eye centres **visual mass**; layout engines centre **extents**. A square fills its bounding box; a
circle touches it at four tangent points, so a mathematically aligned circle reads as small and high.
Hence **overshoot**: round forms are drawn 1–3% of cap height past the line, up to 5% on some serifs and
on pointed forms like A ([Pangram](https://pangrampangram.com/blogs/journal/optical-vs-mathematical-alignment),
[overshoot](https://learn.scannerlicker.net/2014/09/03/the-art-of-eyeballing-part-3-overshooting/)).
Also **irradiation**: a light shape on a dark ground reads thicker than the same shape reversed.

**For Draftpaper this is a genuine tension with doctrine.** Everything on the sheet sits at whole
millimetres snapped to physical pixels, which is mathematical alignment by construction. Optical
correction means deliberately *breaking* the grid by a fraction of a millimetre — and our grid is the
thing that keeps two renderers identical. The resolution is probably: optical corrections belong in the
*document* as measured numbers (a glyph's own offset, stated once), never as a renderer's judgement.
Unresolved.

### Colour — relativity, and why a palette cannot be checked one colour at a time

Albers' whole argument is that colour is **relative and interactive**, not fixed: the same grey on two
grounds is two greys ([Artforum](https://www.artforum.com/columns/josef-alberss-interaction-of-color-193938/)).
**Simultaneous contrast** is the mechanism — a colour induces its opposite in hue, value and intensity
in its neighbour, and both are changed ([handout](http://facweb.cs.depaul.edu/sgrais/color_context.htm)).
Itten's seven contrasts (hue, light-dark, cold-warm, complementary, simultaneous, saturation, extension)
are a vocabulary for naming which contrast a design is using
([Itten](https://en.wikipedia.org/wiki/Johannes_Itten)).

**For Draftpaper:** `draftpaper theme check` already measures contrast ratios pairwise, which is the
right instinct and an incomplete one — it checks legibility, not interaction. A theme can pass every
ratio and still have an accent that shifts hue against the paper it sits on. Worth knowing; not worth
building until a theme actually fails this way.

---

## III. On neuro-linguistic programming

Asked for, so answered plainly: **NLP is not a usable source.** Its central claims — eye-accessing cues
indicating internal representation, matching a person's "representational system" to build rapport, and
the VAK learning-styles taxonomy it popularised — have been tested repeatedly since the early 1980s and
have not replicated. It is categorised as pseudoscience in the research literature. There is no
extractable principle here, and borrowing its vocabulary would make everything next to it less credible.

What it gestures at is real and is covered above by fields that have evidence: preattentive processing
for what the eye grabs first, Gestalt for how things group, Norman for how an interface says what it
affords, and the glance literature for attention under load.

### Norman — the vocabulary we should actually use

An **affordance** is a possibility of action; a **signifier** is how that possibility is perceived.
Designers control signifiers, not affordances — an affordance nobody perceives is useless. Discoverability
rests on six: affordances, signifiers, constraints, mappings, feedback, conceptual model
([summary](https://uxmag.com/articles/understanding-don-normans-principles-of-interaction)).

**For Draftpaper this names the whole keyboard project.** The bindings were always affordances; the
cheatsheet was not a signifier because you had to go and find it. The hold overlay is a signifier. The
passing word is **feedback**. The leader's visible legend is Raskin's argument and Norman's at once.

It also names an unfixed gap: the title block's cells and the schedule rows have `action` and
`action_right` in the document — real affordances — with **no signifier at all**. Nothing on the sheet
says a cell can be clicked, or that right-clicking does something else.

---

## IV. What this gives Draftpaper

The traditions disagree. Here is where we stand on each fork, and why.

| fork | sides | Draftpaper |
|---|---|---|
| hierarchy by **depth** or by **layout** | Apple materials / vibrancy vs Tufte layering | **layout.** A drawing is one sheet; nothing is above anything. No shadow, no blur, no elevation. |
| grouping by **boundary** or by **alignment** | common region vs proximity | **weakest cue that works**: proximity → alignment → value → border. |
| **objectivity** or **tension** | Swiss grid vs Constructivist composition | grid for structure, Constructivist discipline for the accent: one focal point, one accent colour. |
| emptiness **residual** or **active** | (the failure) vs ma / Hara | **active.** Every empty region must be holding something open. |
| magnitude by **scale** or **repetition** | — vs Isotype | **repetition.** A glyph is one size; quantity is more of them or a bar. |
| colour for **magnitude** or **attention** | — vs preattentive evidence | **attention only.** Hue says look here; position and length say how much. |
| detail **at once** or **on inspection** | — vs Neurath / Tufte micro-macro | **micro/macro.** One thing at a glance, detail on the fourth look. |

Five things that follow immediately and are testable:

1. **The fourth-look test.** For every screen: name the one thing it says at a glance. The settings
   panel currently fails — a page list, a page name and nine rows all arrive at once.
2. **One channel per message.** The selected row says "selected" three times (border, weight, fill).
   That is Tufte's 1+1=3. Pick one.
3. **Glance count, not glance beauty.** Stable position is worth more than any refinement, and the sheet
   already has it. Don't spend it.
4. **Every affordance needs a signifier.** Clickable title-block cells currently have none.
5. **Empty space must be answerable.** If it is not holding anything open, the layout stopped early.

## Open questions

1. **Optical correction versus the whole-millimetre grid.** The grid is how two renderers stay
   identical; optical corrections are sub-millimetre and judgement-based. Proposal: corrections live in
   the document as stated numbers, never as a renderer's judgement. Unresolved and worth resolving
   before any of it is built.
2. **Where the warmth comes from.** We are a Swiss-grid product, and the recorded failure of that style
   is coldness. The candidates on this sheet are the paper's own colour, the single accent, and the
   lettering's rhythm. Not yet decided, and this is the question the mockups keep failing.
3. **Whether the glyph set is a system or a collection.** Isotype's symbols were one designer's
   consistent language. Ours are Nerd Font glyphs chosen per cell with text fallbacks — a collection.
   Whether that matters is untested.
4. **ISO 15008's actual numbers.** The nearest thing to a standard for an instrument surface read in
   glances. Paywalled; not bought.
