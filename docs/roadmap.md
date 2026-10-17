# Roadmap

Where Draftpaper is going, in order. Each stage is usable on its own before the next starts.

## 1. The i3 edition (done: 0.1.x, frozen)

Draftpaper as an add-on to i3 on X11: the sheet, the live title block and margins, parking. Released as a
Debian package. Frozen since 2026-10-14: no further releases; 0.1.x stays installable. Draftpaper is now
the compositor.

## 2. The Wayland compositor (the daily session: 0.2.0 on)

Draftpaper as a compositor of its own, on Smithay: the sheet drawn by the compositor, windows tiled on
whole zones (free-form since 0.3.0), parking with exact restore, a launcher of its own (0.3.0), and its
own login screen under greetd with a flicker-free handoff to the session (0.4.0). It replaces the i3
edition in the `draftpaper` package. Design: `docs/wayland-design.md`.

Next: the session restored after a logout or a reboot, the applications back in their places (#38);
settings for the displays and text pages (#36); drag and drop (#18).

## 3. A distribution (vision)

Draftpaper as an operating system, on the model of opinionated distributions that take a base system and
make every choice for the user (kaiseki, Omarchy):

- **Boot on ZFS.** The root file system is ZFS, **encrypted by default** with ZFS native encryption, with
  an unencrypted install as an option. Boot environments: every upgrade is a snapshot, and a bad one is
  rolled back from the boot menu.
- **Its own boot loader**, drawn as a sheet like the desktop. ZFSBootMenu is the starting point to
  evaluate: it boots Linux from ZFS, unlocks native encryption and lists boot environments.
- **An installer** that partitions, creates the pool and datasets, sets up encryption, and installs the
  base system with the Draftpaper session as the default, in a handful of decisions.
- **A plugin ecosystem**: cells for the title block and margins, themes, and layout behaviours that users
  can install and share, with a manifest format and a place to find them.

Open: the base (Debian, which the package already targets, or another), how kaiseki's model carries over,
release and update channels, hardware support policy, and where plugins are published.

This stage starts once the compositor (stage 2) is the daily session.
