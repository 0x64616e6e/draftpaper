# Roadmap

Where Draftpaper is going, in order. Each stage is usable on its own before the next starts.

## 1. The i3 edition (done: 0.1.x)

Draftpaper as an add-on to i3 on X11: the sheet, the live title block and margins, parking. Released as a
Debian package.

## 2. The Wayland compositor (in progress)

Draftpaper as a compositor of its own, on Smithay: the sheet drawn by the compositor, windows tiled in
columns and rows aligned to the zones, parking with exact restore. Design: `docs/wayland-design.md`;
plan: milestones M0 to M3 on GitHub. It ships in the same `draftpaper` package as the i3 edition.

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
