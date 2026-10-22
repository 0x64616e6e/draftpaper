# The keyboard: the legend, the leader, and binding by pointing

Status: steps 1, 3 and the passing word built — the hold overlay, the leader and the line the sheet
says in answer to a key, in both renderers (2026-10-22); the rest designed. Against 0.7.5.
Where this disagrees with the code, the code has not been written yet — except where it is marked built,
and there the code wins.

## Why

Draftpaper has a documented, mnemonic keymap and a register you can type into, and neither of them is
what the research says makes an operator fast. The research page (draftpaper.este.systems/research)
already carries the finding and the admission:

> It does not yet have the two things this research says matter most: a key shown beside the thing it
> runs, and a hold-to-reveal overlay for what follows a leader key. The cheatsheet is a file you go and
> read, which is exactly the shape the literature says fails.

The shape of the failure is specific. Lane and colleagues watched 251 experienced users who knew their
shortcuts issue commands by menu anyway; Cockburn's survey of novice-to-expert transitions finds expert
accelerators "seldom used", with users failing to adopt faster methods. What moves people is teaching
the fast path **while they use the slow one** — marking menus, where selecting from the menu is
physically the same motion as the expert's mark, and ExposeHK, where holding the modifier overlays each
command with its key and measurably increases hotkey use. A cheatsheet in a file is none of that.

## What is decided

**The overlay mirrors the chord map. It extends past it later.**

Everything below follows from that. The first build invents no vocabulary: it letters the keymap that
already exists onto the things that keymap already acts on. A tree of sequences that no chord can
express comes after, once the overlay has earned its place.

## Two gestures, one legend

They are not alternatives. They do different work, and the difference is the mechanism the research
turns on.

**Holding Super teaches the chords.** You are already holding Super in order to press `Super+h`, so the
overlay arriving is a slower version of the motion you are trying to learn. The novice path and the
expert path are the same movement, which is the whole of why marking menus work, and every slow use
rehearses the fast one. This is the part that makes people graduate.

**A leader composes what a chord cannot.** `Super` tapped, then a key, then a key: a tree, for the
vocabulary that will not fit in one chord without running out of letters. There is no expert motion to
rehearse here, because the sequence *is* the expert motion.

So the hold is for what exists and the leader is for what will not fit, and **wherever a chord exists
the leader route is the same keys**. `Super` tap then `h` does exactly what `Super+h` does. The leader
is a way of typing the vocabulary, not a second vocabulary — Raskin's monotony holds: one way to mean a
thing, even where there are two ways to type it. The legend shows `Super+h`, and you may hold or tap.

Both gestures draw **the same legend**, or we have built two interfaces for one keymap.

## The legend

A mark beside every actionable thing, and the key, the mark and what it does lettered into the sheet's
free area — the way a parts list sits on a drawing. This is the one place the design departs from
qutebrowser's hints, and it departs on purpose:

- One disclosure, not two. A second, longer hold to reveal descriptions is undiscoverable; nobody holds
  a key for a second and a half to find out whether something else happens.
- A crowded sheet gets a **shorter legend**, not a broken overlay. It degrades by saying less.
- It is the sheet's own idiom. A drawing with annotations has a legend; this is that.

The legend is also what makes a leader admissible at all. We published Raskin's preference for
quasimodes over modes, and a leader key *is* a mode. His argument is against modes you can **forget you
are in**, and his own remedy is a mode that is impossible to miss and always escapable — which is the
ground on which the rulers were already accepted ("two lines across the whole screen is not a state you
can forget you are in"). So: **no silent leader.** The leader shows the legend or the leader does not
ship. `Esc` always leaves, an unbound key does nothing and says so, and there is no inter-key timeout —
once the legend is on screen a timeout only punishes you for reading it.

## Which key, and why there is no choice

A desktop has no normal mode. Focus is nearly always inside an application, which receives every plain
keystroke, so `Space` or `,` — the leader of Helix and Spacemacs — is unavailable unless it is stolen
globally, which breaks the application it is stolen from.

That leaves a modifier. A **Super tap** — pressed and released with nothing else — is unambiguous:
press `Super+h` and Super is never released alone, so the leader never arms. This removes the timing
threshold that the hold gesture needs, and with it the false overlays that a threshold fires while
someone types quickly.

## Where the targets come from

Both registries already exist, and the overlay is a join between them on the action's name.

| | holds | lives in |
|---|---|---|
| key → action | `("Mod+h", "focus left")`, the table of defaults and whatever the config overrides | `compositor/src/config.rs` |
| thing → action | every cell, row and parked title's `action` and `action_right` | the content document (`docs/sheet-content.md`) |

Nothing has to be enumerated for the first build. For each thing the document says is actionable, look
up the key bound to its action and letter that key on it. Where the document gives both an `action` and
an `action_right`, the mark pressed plainly runs the first and with Shift the second — two verbs, already
populated, no new vocabulary.

**The overlay shows the real binding.** It does not generate throwaway labels. A label that means
nothing tomorrow teaches nothing, and the point of the exercise is that you stop needing the overlay.

## Zones need no hints

The sheet already prints a coordinate system: the columns numbered across the frame, the rows lettered
down it. For every spatial target — move this window there, split there, park here — the hints are
already drawn, in the product's own hand, and they do not move. `Super`-hold then `C3` is not a hint
system; it is reading the drawing.

This is the largest and most frequent class of target, and it costs nothing and teaches a coordinate
you can see with the overlay closed. A tiling compositor with arbitrary geometry could not do it.

## Stable and positional

The distinction decides what may be bound, and the architecture already draws it.

- **Stable**: a thing whose action has a name in the content document — `profile.next`, `ext-ci.left`,
  `schedule.trains`. The name means the same thing tomorrow.
- **Positional**: this window, the third parked title, that notification. Reachable by coordinate,
  never by a name that survives the afternoon.

Anything that still needs a *generated* mark after the keymap has settled is a missing binding, and the
overlay is then a list of them.

## Binding by pointing

The reason a keymap never grows is that the config file asks you to know an action's name before you can
bind it: you have to find `park_prev` in a list to discover that it exists. Inverted — point at the
thing on the sheet and press the key you want it on — the keymap grows by use, which is what designing
against non-adoption actually requires.

Two rules keep it from filling the config with bindings that rot:

1. **Only stable things may be bound.** The prompt never appears over a positional target, so a binding
   cannot be attached to a window that will not exist tomorrow. The system can tell which is which
   without guessing, because the document names one kind and not the other.
2. **The tree places it.** A flat "press any free key" degenerates into assignments nobody remembers.
   Once the leader tree exists the prompt can say *this is a schedule; schedules live under `s`; `c`,
   `d` and `f` are free* — so a new binding lands somewhere mnemonic by construction. Norman's mapping
   argument, which the research page already cites: a meaningful binding is one chunk, an arbitrary one
   is not.

Draftpaper writes the result. The user's `config.toml` is the user's file, so the bindings it accepts go
in a block or a file Draftpaper owns, and the config always wins over it.

## Not settled

1. **How far the leader tree extends past the chord map.** Mirroring keeps one language and caps the
   vocabulary at what chords can express; extending is where the power is and where a second language
   creeps back in. Decided when the mirror has been lived with, not before.
2. **Whether a Super tap is acceptable as the leader**, given it is muscle memory for an overview
   elsewhere and that some applications grab Super. The fallback is `Super+Space`, which is immune and
   gives the timing threshold back.
3. **What the overlay costs to draw** on a sheet whose whole doctrine is that nothing moves. The defence
   is that it is asked for — the objection in the literature is to unbidden motion, not to a response to
   your own key — but it should arrive without animation and leave without one.

## Order

1. **The hold overlay over the existing keymap.** ✅ Built 2026-10-20; in both renderers
   2026-10-21. Read-only, nothing generated, no new actions. Mod held alone past
   `[keys] legend_after` (500 ms) and the keymap is lettered on the sheet's free side, as many
   columns of it as the sheet will take; let Mod go, or press anything at all, and it is gone.

   It stands on the theme's paper, opaque, `pad` millimetres out past the table on every side: it is
   over the windows, and a legend you can read the window through is not a legend. The paper, like
   everything else it is set out on, is measured in the document (`content::legend()`).

   The compositor keeps the gesture, being the only thing that sees every key, and says so with
   the outputs (`legend`); it draws no text, as it draws none of the sheet's lettering. The keys
   are its own, asked for once over the IPC (`keys`), so neither renderer keeps a copy of the
   keymap. The millimetres they set it out on are the document's (`content::legend()`), for the
   same reason, and `compositor/tests/lettering.sh` compares the two legends like everything else.

   It marks as well as lettering (2026-10-21): each window a focus key would reach carries that
   key, and the window the keys act *from* carries a quiet mark of its own. Which window each
   key reaches is the compositor's (`focus_to` in the outputs, from the same `peek_dir` that
   `focus left` runs, so a mark cannot promise what the key would not do); which key that is
   comes off the keymap. The table gives way to the marks, taking half the sheet at most, and
   says in its heading how many bindings it had no room for.

   Still not done: nothing is marked but windows. The title block's cells and the schedules'
   rows carry actions in the document but no keys in the keymap, and by this design that makes
   them missing bindings rather than things to label — which is what step 4 is for.

   `msg legend [on|off]` puts it out without the gesture, which is what the tests drive it with and what
   a binding of one's own would run.
2. **The legend**, once there is more to say than a mark can carry.
3. **The leader**, mirroring the chord map exactly. ✅ Built 2026-10-21, in the compositor; both
   renderers letter it.

   Mod **tapped** — pressed and let go with nothing in between, before `legend_after` is up — arms
   it. That is the same gesture the hold already starts, told apart by whether the wait is still
   running when Mod comes up, so there is no second threshold and no second key. The legend comes up
   and **stays**: no silent leader, which is the whole condition for admitting a mode at all.

   The next key is looked up in the chord map with Mod held (`config::led`), so `Mod` tap then `h`
   runs what `Mod+h` runs by construction rather than by a second table that could disagree. Whatever
   else is held still counts, and a modifier on its own does not spend it, so the leader reaches
   `Mod+Shift+Left`. `Esc` leaves having run nothing; a click leaves; a key with no binding leaves.
   While it is armed the keys it takes do not reach the focused window — the one place any of this
   takes a key away.

   The compositor keeps it (`leader` in the outputs, beside `legend`), as it keeps the hold. What the
   heading is *called* in each state is the document's (`content::legend()`: `title`, `held`,
   `leader`, `more`), because that is lettering and lettering is the document's — the two renderers
   had a copy of the words each until now. `msg leader [on|off]` arms it without the gesture, which is
   what the tests drive it with, a key being untappable in a headless compositor.

   A key with no binding **says so**: the passing word, below.
3a. **The passing word.** ✅ Built 2026-10-22, in the compositor and both renderers.

   One line the sheet says in answer to a key and then takes back. It stands on the **legend's
   heading**, on its right, where `MOD HELD` and `LEADER` already say what state the keyboard is in —
   a word about a key belongs in the keyboard's own register, and that register already has a place on
   the sheet. It is not a notification: those belong to whoever took the name on the session bus
   (docs/sheet-content.md), and mixing the two would be a second voice saying the same kind of thing.

   The compositor keeps it, as it keeps the legend and the leader, and publishes it in the outputs as
   `word`: `{"say": "unbound", "about": "q"}` — **what happened, and what about**. It never carries a
   phrasing. How `unbound` reads is `content::legend()`'s `unbound` ("NOTHING ON"), so the word is
   lettered out of the document like every other word on the legend and neither renderer keeps one of
   its own. A case the document has no words for is lettered by its name rather than dropped, so a
   word added to one and not the other is visible rather than silent.

   Saying a word holds the legend up under it, because that is the paper it stands on; when the word's
   time is up (`[keys] word_for`, 1200 ms) both go — unless Mod is being held, which takes the legend
   over. Anything **pressed** supersedes it; the release of the key it is answering does not, that
   release being the tail of the same keystroke. A click takes it back: it was an answer to the
   keyboard. `msg word unbound q` and `msg word off` say one without a key, which is what the tests
   drive it with.

   The first and only thing it says is that a key had nothing on it — the gap step 3 left. The second
   will be step 4's prompt, which is why this is one channel and not two.

   Still not done: a word says what happened, not what to do about it. "NOTHING ON q" is the whole of
   it; *binding* q to something is step 4.
4. **Binding by pointing**, which wants the tree to place things in — and the passing word, which it
   now has.
5. **Extending the tree** past what a chord can express — if by then it is still wanted.
