# Contributing

Issues and pull requests are welcome.

## Issues

- **Bugs**: what you did, what you expected, what happened. Include your Debian version, i3 and Quickshell
  versions (`i3 --version`, `qs --version`), your screens (`xrandr --query`), and the shell's log
  (`qs log -p /usr/share/draftpaper/qml/shell.qml`, or the newest file under
  `/run/user/$UID/quickshell/by-id/`).
- **Ideas**: describe the use first, then the change. Draftpaper is opinionated (a drafting sheet, no
  bar), so a proposal that fits that idea has the best chance.
- **Themes**: a new theme is welcome as a pull request with a screenshot; see `themes/softglow.toml`.

## Running from a checkout

    git clone https://github.com/0x64616e6e/draftpaper
    cd draftpaper
    ./bin/draftpaper start          # uses the checkout's qml/ and themes/, not /usr/share

In your i3 config, include the checkout's `i3/draftpaper.conf` instead of the installed one, and change
its `draftpaper` commands to the checkout's `bin/draftpaper`. `./bin/draftpaper stop` stops the shell.

Build the package with `dpkg-buildpackage -us -uc -b` (needs `debhelper`); check it with `lintian`.

## Pull requests

- One change per pull request, with a description of what it changes for the person using it.
- Keep to the existing style: Rust for the compositor and the `draftpaper` command (no Python), QML for
  the shell; comments that say why, at the density of the surrounding code.
- Run `make check`, and try the change in a Draftpaper session (or a nested one); say in the pull request what you tested
  and what you could not (one screen or two, HiDPI or not, laptop or desktop).
- A change to what the sheet draws (the frame, the ticks, the title block, the wallpaper) also runs `make pixels`
  from a Draftpaper session: the sheet at every scale, checked pixel by pixel against `compositor/src/title.rs`.
- Update `README.md` and `docs/draftpaper.1` when you change commands, keys or configuration.
- New dependencies need a reason: every one becomes a Debian package dependency.

## Design

`docs/wayland-design.md` describes where the project is going (a Wayland compositor of its own).
Larger changes should fit that direction; open an issue to discuss them first.

## Licence

By contributing you agree that your contribution is licensed under the MIT licence (see `LICENSE`).
