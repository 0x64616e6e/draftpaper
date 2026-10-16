//! The layout: every window takes a rectangle of the sheet's zones, so every window edge falls on a zone
//! boundary and lines up with the ticks in the margins, and every window is a zone at least. Windows
//! never overlap; zones nobody takes stay paper.
//!
//! A new window takes the largest free rectangle; with none free, the focused window is cut in half
//! along its longer side and the new one takes the second half (the right or the lower one). With every
//! window a single zone, a new window floats.
//!
//! A window moves a zone at a time, keeping its size, into free zones (Layout::move_dir); against a
//! window that spans the same rows (or columns) it swaps places with it; anything else stops it, and
//! nothing changes. Resizing (Layout::resize) right or down makes it a zone larger that way (into free
//! zones, or taking the zone from the windows there when each keeps one), left or up a zone smaller. A
//! window that closes leaves its zones to the windows on one side of it that line up with it exactly, and
//! to paper when there are none.
//!
//! A parked window leaves a placeholder (Placeholder): its zones and the layout before and after, so it
//! comes back to the same place, and the layout to what it was when nothing changed meanwhile.

use smithay::{
    desktop::{Space, Window},
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Physical, Point, Rectangle, Size},
};

/// The frame's interior and its zones, measured as the sheet is (title.rs): in the output's physical
/// pixels, from the edges of its mode. The logical numbers are those divided by the scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    /// where windows may go: inside the frame's line and their borders, in whole logical pixels (an
    /// exclusive zone of a foreign panel narrows it)
    pub area: Rectangle<i32, Logical>,
    /// left edge of the zone grid (the frame line's outer edge), and the zone sizes
    pub origin: Point<f64, Logical>,
    pub zone: (f64, f64),
    pub zones: (u32, u32),
    /// the output's place (logical), scale, size (physical, its mode) and margin (physical)
    at: Point<i32, Logical>,
    scale: f64,
    mode: (i32, i32),
    margin: i32,
}

/// A tile: where its window goes in the space (whole logical pixels, the size it is told) and the
/// physical pixels it covers on its output, which the renderer moves it onto and cuts it to (render.rs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tile {
    pub logical: Rectangle<i32, Logical>,
    pub pixels: Rectangle<i32, Physical>,
}

impl Grid {
    /// For an output at `at` of `size` logical pixels at `mm` logical pixels per millimetre and `scale`,
    /// `mode` physical pixels, with a margin of `margin_mm`: zones of about 50 mm, at least two.
    pub fn new(at: Point<i32, Logical>, size: Size<i32, Logical>, mm: f64, margin_mm: f64, scale: f64, mode: (i32, i32)) -> Grid {
        let mode = if mode.0 > 0 && mode.1 > 0 { mode } else { ((size.w as f64 * scale).round() as i32, (size.h as f64 * scale).round() as i32) };
        let mg = crate::title::margin(margin_mm, mm, scale);
        let (iw, ih) = ((mode.0 - 2 * mg) as f64, (mode.1 - 2 * mg) as f64);
        let nx = (iw / (50.0 * mm * scale)).round().max(2.0);
        let ny = (ih / (50.0 * mm * scale)).round().max(2.0);
        let mut grid = Grid {
            area: Rectangle::default(),
            origin: (at.x as f64 + mg as f64 / scale, at.y as f64 + mg as f64 / scale).into(),
            zone: (iw / nx / scale, ih / ny / scale),
            zones: (nx as u32, ny as u32),
            at,
            scale,
            mode,
            margin: mg,
        };
        // the whole tile: its logical rectangle holds every tile
        let whole = grid.pixels(0, nx as u32, 0.0, ny);
        let (x0, y0) = ((whole.loc.x as f64 / scale).floor() as i32, (whole.loc.y as f64 / scale).floor() as i32);
        let (x1, y1) = (((whole.loc.x + whole.size.w) as f64 / scale).ceil() as i32, ((whole.loc.y + whole.size.h) as f64 / scale).ceil() as i32);
        grid.area = Rectangle::new((at.x + x0, at.y + y0).into(), (x1 - x0, y1 - y0).into());
        grid
    }

    /// A window's border, in physical pixels: one logical pixel's worth.
    pub fn border(&self) -> i32 {
        (self.scale.round() as i32).max(1)
    }

    /// The physical pixel of the boundary `k` zones in along a side of `len` pixels cut into `n` zones:
    /// the tick there (title.rs's tick()); k may be a fraction (rows outnumbering the zones).
    fn boundary(&self, len: i32, n: f64, k: f64) -> i32 {
        self.margin + (k * (len - 2 * self.margin) as f64 / n).round() as i32
    }

    /// The physical pixels of a window spanning zones x0..x1 across and y0..y1 down: inside the frame's
    /// line and its border on the frame's side; between two windows, their borders either side of the
    /// tick's pixel, which the paper shows through, in line with the tick in the margin.
    fn pixels(&self, x0: u32, x1: u32, y0: f64, y1: f64) -> Rectangle<i32, Physical> {
        let (nx, ny) = (self.zones.0 as f64, self.zones.1 as f64);
        let (b, line) = (self.border(), crate::title::LINE);
        let left = if x0 == 0 { self.margin + line + b } else { self.boundary(self.mode.0, nx, x0 as f64) + 1 + b };
        let right = if x1 >= self.zones.0 { self.mode.0 - self.margin - line - b } else { self.boundary(self.mode.0, nx, x1 as f64) - b };
        let top = if y0 <= 0.0 { self.margin + line + b } else { self.boundary(self.mode.1, ny, y0) + 1 + b };
        let bottom = if y1 >= ny { self.mode.1 - self.margin - line - b } else { self.boundary(self.mode.1, ny, y1) - b };
        Rectangle::new((left, top).into(), ((right - left).max(1), (bottom - top).max(1)).into())
    }

    /// The tile of a window spanning zones x0..x1 across and y0..y1 down, kept inside `area`.
    pub fn tile(&self, x0: u32, x1: u32, y0: f64, y1: f64) -> Tile {
        let s = self.scale;
        let mut p = self.pixels(x0, x1, y0, y1);
        // a foreign panel's exclusive zone: the tile stays out of it
        let a = self.area;
        let (ax0, ay0) = (((a.loc.x - self.at.x) as f64 * s).ceil() as i32, ((a.loc.y - self.at.y) as f64 * s).ceil() as i32);
        let (ax1, ay1) = (((a.loc.x + a.size.w - self.at.x) as f64 * s).floor() as i32, ((a.loc.y + a.size.h - self.at.y) as f64 * s).floor() as i32);
        let (l, t) = (p.loc.x.max(ax0), p.loc.y.max(ay0));
        let (r, bt) = ((p.loc.x + p.size.w).min(ax1), (p.loc.y + p.size.h).min(ay1));
        p = Rectangle::new((l, t).into(), ((r - l).max(1), (bt - t).max(1)).into());
        // in whole logical pixels: the nearest place, and a size that covers the pixels (cut to them)
        let loc = ((p.loc.x as f64 / s).round() as i32, (p.loc.y as f64 / s).round() as i32);
        let size = ((p.size.w as f64 / s).ceil() as i32, (p.size.h as f64 / s).ceil() as i32);
        Tile { logical: Rectangle::new((self.at.x + loc.0, self.at.y + loc.1).into(), size.into()), pixels: p }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    /// 0 across, 1 down
    fn axis(self) -> usize {
        match self {
            Direction::Left | Direction::Right => 0,
            Direction::Up | Direction::Down => 1,
        }
    }

    /// right or down
    fn forward(self) -> bool {
        matches!(self, Direction::Right | Direction::Down)
    }
}

/// A rectangle of zones: `[across, down]`, each `[from, to)` in zones from the frame's top left.
pub type Zones = [[u32; 2]; 2];

fn overlaps(a: &Zones, b: &Zones) -> bool {
    (0..2).all(|k| a[k][0] < b[k][1] && b[k][0] < a[k][1])
}

/// Where a parked window was, to put it back there (Layout::park, Layout::unpark).
#[derive(Debug, Clone)]
pub struct Placeholder<W = Window> {
    zones: Zones,
    /// the sheet's zones then
    sheet: (u32, u32),
    /// the layout before it left, and right after; when the layout is still the second, it is the first again
    before: Vec<(W, Zones)>,
    after: Vec<(W, Zones)>,
}

#[derive(Debug)]
pub struct Layout<W = Window> {
    tiles: Vec<(W, Zones)>,
    /// index into `tiles`
    focus: Option<usize>,
    /// the sheet's zones the tiles were placed on (a screen of another size rescales them: fit)
    sheet: (u32, u32),
    /// windows with no zone left after a rescale, for the caller to float (take_evicted)
    evicted: Vec<W>,
}

impl<W> Default for Layout<W> {
    fn default() -> Self {
        Layout { tiles: Vec::new(), focus: None, sheet: (0, 0), evicted: Vec::new() }
    }
}

impl<W: Clone + PartialEq> Layout<W> {
    pub fn windows(&self) -> impl Iterator<Item = &W> {
        self.tiles.iter().map(|(w, _)| w)
    }

    pub fn contains(&self, window: &W) -> bool {
        self.find(window).is_some()
    }

    fn find(&self, window: &W) -> Option<usize> {
        self.tiles.iter().position(|(w, _)| w == window)
    }

    pub fn focused(&self) -> Option<W> {
        self.focus.and_then(|i| self.tiles.get(i)).map(|(w, _)| w.clone())
    }

    pub fn focus_window(&mut self, window: &W) {
        if let Some(i) = self.find(window) {
            self.focus = Some(i);
        }
    }

    /// The zones `window` takes.
    pub fn zones_of(&self, window: &W) -> Option<Zones> {
        self.find(window).map(|i| self.tiles[i].1)
    }

    fn side(&self, axis: usize) -> u32 {
        if axis == 0 { self.sheet.0 } else { self.sheet.1 }
    }

    /// Whether `z` is on the sheet and no window but those at `except` takes any of it.
    fn free(&self, z: &Zones, except: &[usize]) -> bool {
        (0..2).all(|k| z[k][0] < z[k][1] && z[k][1] <= self.side(k))
            && self.tiles.iter().enumerate().all(|(i, (_, t))| except.contains(&i) || !overlaps(t, z))
    }

    /// The windows taking any of `z`, but those at `except`.
    fn in_the_way(&self, z: &Zones, except: usize) -> Vec<usize> {
        (0..self.tiles.len()).filter(|&i| i != except && overlaps(&self.tiles[i].1, z)).collect()
    }

    /// The zones a step beyond `a`'s side towards `dir`, as wide as `a` (None at the frame).
    fn strip(&self, a: &Zones, dir: Direction) -> Option<Zones> {
        let k = dir.axis();
        let mut s = *a;
        s[k] = if dir.forward() {
            if a[k][1] >= self.side(k) {
                return None;
            }
            [a[k][1], a[k][1] + 1]
        } else {
            if a[k][0] == 0 {
                return None;
            }
            [a[k][0] - 1, a[k][0]]
        };
        Some(s)
    }

    /// The sheet as `grid` cuts it: after a change of the screen's zones (another screen, another scale)
    /// every edge goes to the same place in proportion; a window left with no zone is placed again as a
    /// new one, or floats (take_evicted).
    pub fn fit(&mut self, grid: &Grid) {
        let (old, new) = (self.sheet, grid.zones);
        if old == new {
            return;
        }
        self.sheet = new;
        if old.0 == 0 || old.1 == 0 {
            return;
        }
        let focused = self.focused();
        let scale = |e: u32, o: u32, n: u32| ((e as u64 * n as u64 + o as u64 / 2) / o as u64) as u32;
        for (_, z) in &mut self.tiles {
            for k in 0..2 {
                let (o, n) = if k == 0 { (old.0, new.0) } else { (old.1, new.1) };
                z[k] = [scale(z[k][0], o, n), scale(z[k][1], o, n)];
            }
        }
        let (kept, lost): (Vec<_>, Vec<_>) = std::mem::take(&mut self.tiles).into_iter().partition(|(_, z)| z[0][0] < z[0][1] && z[1][0] < z[1][1]);
        self.tiles = kept;
        self.focus = focused.as_ref().and_then(|w| self.find(w)).or(if self.tiles.is_empty() { None } else { Some(0) });
        for (w, _) in lost {
            if !self.add(w.clone(), grid) {
                self.evicted.push(w);
            }
        }
        if let Some(w) = &focused {
            self.focus_window(w);
        }
    }

    /// The windows a rescale left without a zone (fit), for the caller to float.
    pub fn take_evicted(&mut self) -> Vec<W> {
        std::mem::take(&mut self.evicted)
    }

    /// The largest free rectangle of zones (the topmost, then leftmost, of equals).
    fn largest_free(&self) -> Option<Zones> {
        let (nx, ny) = self.sheet;
        let mut best: Option<(u32, Zones)> = None;
        for y0 in 0..ny {
            for x0 in 0..nx {
                for y1 in y0 + 1..=ny {
                    for x1 in x0 + 1..=nx {
                        let z = [[x0, x1], [y0, y1]];
                        let area = (x1 - x0) * (y1 - y0);
                        if best.is_none_or(|(b, _)| area > b) && self.free(&z, &[]) {
                            best = Some((area, z));
                        }
                    }
                }
            }
        }
        best.map(|(_, z)| z)
    }

    /// A new window: the largest free rectangle, else the second half of the focused window (cut along
    /// its longer side; when that is a single zone, the largest window that can be cut). False with every
    /// window a single zone: the caller floats it.
    #[must_use]
    pub fn add(&mut self, window: W, grid: &Grid) -> bool {
        self.fit(grid);
        if self.find(&window).is_some() {
            return true;
        }
        if let Some(z) = self.largest_free() {
            self.tiles.push((window, z));
            self.focus = Some(self.tiles.len() - 1);
            return true;
        }
        let len = |z: &Zones, k: usize| z[k][1] - z[k][0];
        let mut order: Vec<usize> = (0..self.tiles.len()).collect();
        order.sort_by_key(|&i| (Some(i) != self.focus, std::cmp::Reverse(len(&self.tiles[i].1, 0) * len(&self.tiles[i].1, 1))));
        let Some(i) = order.into_iter().find(|&i| len(&self.tiles[i].1, 0) > 1 || len(&self.tiles[i].1, 1) > 1) else { return false };
        let z = self.tiles[i].1;
        // the longer side as it shows (zones need not be square), when it can be cut
        let (w, h) = (len(&z, 0) as f64 * grid.zone.0, len(&z, 1) as f64 * grid.zone.1);
        let k = if (w >= h && len(&z, 0) > 1) || len(&z, 1) == 1 { 0 } else { 1 };
        let cut = z[k][0] + len(&z, k).div_ceil(2);
        let (mut first, mut second) = (z, z);
        first[k][1] = cut;
        second[k][0] = cut;
        self.tiles[i].1 = first;
        self.tiles.push((window, second));
        self.focus = Some(self.tiles.len() - 1);
        true
    }

    /// Take `window` out. Its zones go to the windows on one side of it that line up with it exactly
    /// (left, then above, right, below): each grows across the gap. With none, they stay paper.
    pub fn remove(&mut self, window: &W, grid: &Grid) {
        self.fit(grid);
        let Some(i) = self.find(window) else { return };
        let was_focused = self.focus == Some(i);
        let (_, z) = self.tiles.remove(i);
        self.focus = match self.focus {
            Some(f) if f > i => Some(f - 1),
            Some(f) if f == i => None,
            f => f,
        };
        let mut grown = None;
        for dir in [Direction::Left, Direction::Up, Direction::Right, Direction::Down] {
            let (k, o) = (dir.axis(), 1 - dir.axis());
            // the windows touching that side of the gap, all within its span and covering it together
            let edge = if dir.forward() { z[k][1] } else { z[k][0] };
            let side: Vec<usize> = (0..self.tiles.len())
                .filter(|&j| {
                    let t = &self.tiles[j].1;
                    let touches = if dir.forward() { t[k][0] == edge } else { t[k][1] == edge };
                    touches && t[o][0] < z[o][1] && z[o][0] < t[o][1]
                })
                .collect();
            let within = side.iter().all(|&j| self.tiles[j].1[o][0] >= z[o][0] && self.tiles[j].1[o][1] <= z[o][1]);
            let covered: u32 = side.iter().map(|&j| self.tiles[j].1[o][1] - self.tiles[j].1[o][0]).sum();
            if side.is_empty() || !within || covered != z[o][1] - z[o][0] {
                continue;
            }
            for &j in &side {
                let t = &mut self.tiles[j].1;
                if dir.forward() {
                    t[k][0] = z[k][0];
                } else {
                    t[k][1] = z[k][1];
                }
            }
            grown = side.first().copied();
            break;
        }
        if was_focused || self.focus.is_none() {
            self.focus = grown.or_else(|| self.nearest(&z)).or(if self.tiles.is_empty() { None } else { Some(0) });
        }
    }

    /// The window nearest to `z` (centre to centre).
    fn nearest(&self, z: &Zones) -> Option<usize> {
        let centre = |z: &Zones| [z[0][0] + z[0][1], z[1][0] + z[1][1]];
        let c = centre(z);
        (0..self.tiles.len()).min_by_key(|&i| {
            let t = centre(&self.tiles[i].1);
            t[0].abs_diff(c[0]) + t[1].abs_diff(c[1])
        })
    }

    /// Take `window` out (remove), remembering where it was.
    pub fn park(&mut self, window: &W, grid: &Grid) -> Option<Placeholder<W>> {
        self.fit(grid);
        let zones = self.zones_of(window)?;
        let before = self.tiles.clone();
        self.remove(window, grid);
        Some(Placeholder { zones, sheet: self.sheet, before, after: self.tiles.clone() })
    }

    /// Put a parked window back: with the layout as it was left, the layout as it was before; else into
    /// its own zones when they are free, else as a new window (add; false: the caller floats it).
    #[must_use]
    pub fn unpark(&mut self, window: W, placeholder: &Placeholder<W>, grid: &Grid) -> bool {
        self.fit(grid);
        if placeholder.sheet == self.sheet {
            if self.tiles == placeholder.after && placeholder.before.iter().any(|(w, _)| *w == window) {
                self.tiles = placeholder.before.clone();
                self.focus = self.find(&window);
                return true;
            }
            if self.free(&placeholder.zones, &[]) {
                self.tiles.push((window, placeholder.zones));
                self.focus = Some(self.tiles.len() - 1);
                return true;
            }
        }
        self.add(window, grid)
    }

    /// The window the focus goes to towards `dir`: of those wholly beyond the focused one's side, one
    /// level with it first, then the nearest, then the most in line.
    fn towards(&self, dir: Direction) -> Option<usize> {
        let a = &self.tiles[self.focus?].1;
        let (k, o) = (dir.axis(), 1 - dir.axis());
        (0..self.tiles.len())
            .filter(|&i| {
                let b = &self.tiles[i].1;
                if dir.forward() { b[k][0] >= a[k][1] } else { b[k][1] <= a[k][0] }
            })
            .min_by_key(|&i| {
                let b = &self.tiles[i].1;
                let level = b[o][0] < a[o][1] && a[o][0] < b[o][1];
                let gap = if dir.forward() { b[k][0] - a[k][1] } else { a[k][0] - b[k][1] };
                (!level, gap, (b[o][0] + b[o][1]).abs_diff(a[o][0] + a[o][1]))
            })
    }

    /// Whether no window lies towards `dir` of the focused one (the focus goes on to the next screen).
    pub fn at_edge(&self, dir: Direction) -> bool {
        self.focus.is_none() || self.towards(dir).is_none()
    }

    /// Whether the focused window is against the frame towards `dir` (a move goes on to the next screen).
    pub fn against_frame(&self, dir: Direction) -> bool {
        self.focus.and_then(|i| self.tiles.get(i)).is_none_or(|(_, a)| self.strip(a, dir).is_none())
    }

    /// Move the focus; returns the newly focused window.
    pub fn focus_dir(&mut self, dir: Direction) -> Option<W> {
        if let Some(i) = self.towards(dir) {
            self.focus = Some(i);
        }
        self.focused()
    }

    /// Move the focused window a zone towards `dir`, keeping its size: into free zones; past a window
    /// spanning the same rows (or columns), swapping places with it; else nowhere, and nothing changes.
    pub fn move_dir(&mut self, dir: Direction, grid: &Grid) {
        self.fit(grid);
        let Some(i) = self.focus else { return };
        let a = self.tiles[i].1;
        let Some(strip) = self.strip(&a, dir) else { return };
        let (k, o) = (dir.axis(), 1 - dir.axis());
        if self.free(&strip, &[i]) {
            let mut z = a;
            z[k] = if dir.forward() { [a[k][0] + 1, a[k][1] + 1] } else { [a[k][0] - 1, a[k][1] - 1] };
            self.tiles[i].1 = z;
            return;
        }
        let way = self.in_the_way(&strip, i);
        let [j] = way[..] else { return };
        let b = self.tiles[j].1;
        if b[o] != a[o] {
            return;
        }
        let (la, lb) = (a[k][1] - a[k][0], b[k][1] - b[k][0]);
        let (mut na, mut nb) = (a, b);
        if dir.forward() {
            nb[k] = [a[k][0], a[k][0] + lb];
            na[k] = [a[k][0] + lb, b[k][1]];
        } else {
            na[k] = [b[k][0], b[k][0] + la];
            nb[k] = [b[k][0] + la, a[k][1]];
        }
        self.tiles[i].1 = na;
        self.tiles[j].1 = nb;
    }

    /// Resize the focused window by a zone. Right or down: a zone larger, on its right (bottom) side, or
    /// on its left (top) when that cannot be: into free zones, or taking the zone from the windows there
    /// when each of them keeps one. Left or up: a zone smaller, from its right (bottom) side, or its left
    /// (top) when the other is on the frame; the windows beside that side and within its span take the
    /// zone, else it stays paper.
    pub fn resize(&mut self, dir: Direction, grid: &Grid) {
        self.fit(grid);
        let Some(i) = self.focus else { return };
        let a = self.tiles[i].1;
        let (k, o) = (dir.axis(), 1 - dir.axis());
        let len = |z: &Zones| z[k][1] - z[k][0];
        if dir.forward() {
            for side in [true, false] {
                let towards = match (k, side) {
                    (0, true) => Direction::Right,
                    (0, false) => Direction::Left,
                    (_, true) => Direction::Down,
                    (_, false) => Direction::Up,
                };
                let Some(strip) = self.strip(&a, towards) else { continue };
                let way = self.in_the_way(&strip, i);
                if !way.iter().all(|&j| len(&self.tiles[j].1) > 1) {
                    continue;
                }
                for &j in &way {
                    let t = &mut self.tiles[j].1;
                    if side { t[k][0] += 1 } else { t[k][1] -= 1 }
                }
                let t = &mut self.tiles[i].1;
                if side { t[k][1] += 1 } else { t[k][0] -= 1 }
                return;
            }
        } else {
            if len(&a) < 2 {
                return;
            }
            // the far side, unless it is on the frame and the near one is not
            let far = !(a[k][1] == self.side(k) && a[k][0] > 0);
            let edge = if far { a[k][1] } else { a[k][0] };
            for j in 0..self.tiles.len() {
                let t = &mut self.tiles[j].1;
                let beside = if far { t[k][0] == edge } else { t[k][1] == edge };
                if j != i && beside && t[o][0] >= a[o][0] && t[o][1] <= a[o][1] {
                    if far { t[k][0] -= 1 } else { t[k][1] += 1 }
                }
            }
            let t = &mut self.tiles[i].1;
            if far { t[k][1] -= 1 } else { t[k][0] += 1 }
        }
    }
}

impl Layout<Window> {
    /// Place every window on the grid and tell it its size. Each keeps its tile (tile_of), for the renderer.
    /// `skip` is a fullscreen window, placed by the caller.
    pub fn arrange(&self, space: &mut Space<Window>, grid: &Grid, skip: Option<&Window>) {
        for (window, z) in &self.tiles {
            let tile = grid.tile(z[0][0], z[0][1], z[1][0] as f64, z[1][1] as f64);
            let rect = tile.logical;
            set_tile(window, Some(tile));
            if Some(window) == skip {
                continue;
            }
            if let Some(toplevel) = window.toplevel() {
                toplevel.with_pending_state(|state| {
                    state.size = Some(rect.size);
                    state.states.set(xdg_toplevel::State::TiledLeft);
                    state.states.set(xdg_toplevel::State::TiledRight);
                    state.states.set(xdg_toplevel::State::TiledTop);
                    state.states.set(xdg_toplevel::State::TiledBottom);
                    state.states.unset(xdg_toplevel::State::Fullscreen);
                });
                if toplevel.is_initial_configure_sent() {
                    toplevel.send_pending_configure();
                }
            } else if let Some(x11) = window.x11_surface() {
                // an X11 window is told its place and size directly (in the space's coordinates)
                let _ = x11.configure(rect);
            }
            space.map_element(window.clone(), rect.loc, false);
        }
    }
}

/// A tiled window's tile, kept with the window by Layout::arrange.
struct TileOf(std::cell::Cell<Option<Tile>>);

pub fn set_tile(window: &Window, tile: Option<Tile>) {
    window.user_data().get_or_insert(|| TileOf(std::cell::Cell::new(None))).0.set(tile);
}

/// The tile `window` was last given, while it is still where and as large as that tile says (floating,
/// or a client still catching up with its new size, it is drawn as it is).
pub fn tile_of(window: &Window, at: Rectangle<i32, Logical>) -> Option<Tile> {
    window.user_data().get::<TileOf>()?.0.get().filter(|t| t.logical == at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::title::{margin, tick, LINE};
    use Direction::*;

    /// A sheet of `nx` by `ny` square zones.
    fn sheet(nx: u32, ny: u32) -> Grid {
        Grid {
            area: Rectangle::default(),
            origin: (0.0, 0.0).into(),
            zone: (50.0, 50.0),
            zones: (nx, ny),
            at: (0, 0).into(),
            scale: 1.0,
            mode: (nx as i32 * 50, ny as i32 * 50),
            margin: 0,
        }
    }

    fn z(x0: u32, x1: u32, y0: u32, y1: u32) -> Zones {
        [[x0, x1], [y0, y1]]
    }

    fn at(l: &Layout<u32>, w: u32) -> Zones {
        l.zones_of(&w).unwrap()
    }

    /// Two windows, then halves of halves; with every window a zone, the next is refused (it floats).
    #[test]
    fn new_windows_take_free_zones_then_halves() {
        let g = sheet(6, 4);
        let mut l = Layout::default();
        assert!(l.add(1, &g));
        assert_eq!(at(&l, 1), z(0, 6, 0, 4), "the first takes the frame");
        assert!(l.add(2, &g));
        assert_eq!((at(&l, 1), at(&l, 2)), (z(0, 3, 0, 4), z(3, 6, 0, 4)), "the second the right half");
        assert!(l.add(3, &g));
        assert_eq!((at(&l, 2), at(&l, 3)), (z(3, 6, 0, 2), z(3, 6, 2, 4)), "the third the focused one's lower half");
        let mut l = Layout::default();
        for w in 0..24 {
            assert!(l.add(w, &g), "window {w}");
        }
        assert!(!l.add(24, &g), "every zone taken");
        // a closed window's zone goes to a neighbour, which the next one halves again
        l.remove(&5, &g);
        let area = |t: &Zones| (t[0][1] - t[0][0]) * (t[1][1] - t[1][0]);
        assert_eq!(l.tiles.iter().filter(|(_, t)| area(t) == 2).count(), 1);
        assert!(l.add(24, &g));
    }

    /// The case that started this: a window moves left past rows that are not its own, into free zones,
    /// and nothing else changes size; against a window of the same rows it swaps; else it stops.
    #[test]
    fn moves_keep_sizes() {
        let g = sheet(12, 7);
        let mut l = Layout::default();
        l.tiles = vec![(1, z(0, 4, 0, 3)), (2, z(6, 10, 4, 7))];
        l.sheet = g.zones;
        l.focus = Some(1);
        for _ in 0..6 {
            l.move_dir(Left, &g);
        }
        assert_eq!((at(&l, 1), at(&l, 2)), (z(0, 4, 0, 3), z(0, 4, 4, 7)), "under the other, sizes kept");
        l.move_dir(Up, &g);
        assert_eq!(at(&l, 2), z(0, 4, 3, 6), "up into the free row");
        l.move_dir(Up, &g);
        assert_eq!((at(&l, 1), at(&l, 2)), (z(0, 4, 3, 6), z(0, 4, 0, 3)), "then swaps with the one above");
        // different spans: it stops
        l.tiles = vec![(1, z(0, 4, 0, 3)), (2, z(4, 8, 1, 4))];
        l.focus = Some(1);
        let before = l.tiles.clone();
        l.move_dir(Left, &g);
        assert_eq!(l.tiles, before, "half in the way: nothing changes");
        // side by side, different widths: they swap, each its own width
        l.tiles = vec![(1, z(0, 8, 0, 7)), (2, z(8, 12, 0, 7))];
        l.focus = Some(1);
        l.move_dir(Left, &g);
        assert_eq!((at(&l, 1), at(&l, 2)), (z(4, 12, 0, 7), z(0, 4, 0, 7)));
        assert!(l.against_frame(Left));
        l.move_dir(Left, &g);
        assert_eq!(at(&l, 2), z(0, 4, 0, 7), "at the frame, no further");
    }

    /// Right and down grow, left and up shrink, a zone at least; a neighbour gives or takes the zone.
    #[test]
    fn resizing() {
        let g = sheet(6, 4);
        let mut l = Layout::default();
        assert!(l.add(1, &g));
        for _ in 0..8 {
            l.resize(Left, &g);
        }
        assert_eq!(at(&l, 1), z(0, 1, 0, 4), "shrinks from the right, to a zone");
        for _ in 0..3 {
            l.move_dir(Right, &g);
        }
        for _ in 0..8 {
            l.resize(Right, &g);
        }
        assert_eq!(at(&l, 1), z(0, 6, 0, 4), "grows right to the frame, then left");
        assert!(l.add(2, &g));
        l.focus_window(&1);
        l.resize(Right, &g);
        assert_eq!((at(&l, 1), at(&l, 2)), (z(0, 4, 0, 4), z(4, 6, 0, 4)), "takes a zone from its neighbour");
        l.resize(Right, &g);
        l.resize(Right, &g);
        assert_eq!((at(&l, 1), at(&l, 2)), (z(0, 5, 0, 4), z(5, 6, 0, 4)), "which keeps one");
        l.focus_window(&2);
        l.resize(Left, &g);
        assert_eq!(at(&l, 2), z(5, 6, 0, 4), "a zone at least");
        l.resize(Right, &g);
        assert_eq!((at(&l, 1), at(&l, 2)), (z(0, 4, 0, 4), z(4, 6, 0, 4)), "on the frame, it grows on its left");
        l.resize(Left, &g);
        assert_eq!((at(&l, 1), at(&l, 2)), (z(0, 5, 0, 4), z(5, 6, 0, 4)), "and gives it back");
        l.resize(Up, &g);
        assert_eq!((at(&l, 1), at(&l, 2)), (z(0, 5, 0, 4), z(5, 6, 0, 3)), "a zone shorter: paper under it");
    }

    /// A closed window's zones go to the windows that line up with it; parked and back, all as before.
    #[test]
    fn closing_and_parking() {
        let g = sheet(6, 4);
        let mut l = Layout::default();
        for w in 1..=3 {
            assert!(l.add(w, &g));
        }
        let three = l.tiles.clone();
        l.remove(&1, &g);
        assert_eq!((at(&l, 2), at(&l, 3)), (z(0, 6, 0, 2), z(0, 6, 2, 4)), "both on the right grow left");
        assert!(l.add(1, &g));
        l.tiles = three.clone();
        l.remove(&2, &g);
        assert_eq!(at(&l, 3), z(3, 6, 0, 4), "the one below grows up");
        // two that do not line up: paper
        l.tiles = vec![(1, z(0, 3, 0, 3)), (2, z(3, 6, 0, 4)), (3, z(0, 2, 3, 4))];
        l.remove(&2, &g);
        assert_eq!(l.tiles, vec![(1, z(0, 3, 0, 3)), (3, z(0, 2, 3, 4))]);
        // parked, then back: the layout as before
        l.tiles = three.clone();
        l.focus = Some(1);
        let p = l.park(&2, &g).unwrap();
        assert!(!l.contains(&2));
        assert!(l.unpark(2, &p, &g));
        assert_eq!(l.tiles, three);
        // the others changed meanwhile: back into its zones when they are free
        let p = l.park(&2, &g).unwrap();
        l.tiles = vec![(1, z(0, 3, 0, 4)), (3, z(3, 6, 2, 4))];
        assert!(l.unpark(2, &p, &g));
        assert_eq!(at(&l, 2), z(3, 6, 0, 2));
    }

    /// Focus goes to the window beyond the side, level with the focused one first.
    #[test]
    fn focus_moves_by_place() {
        let g = sheet(6, 4);
        let mut l = Layout::default();
        l.sheet = g.zones;
        l.tiles = vec![(1, z(0, 3, 0, 4)), (2, z(3, 6, 0, 2)), (3, z(3, 6, 2, 4))];
        l.focus = Some(2);
        assert_eq!(l.focus_dir(Left), Some(1));
        assert_eq!(l.focus_dir(Right), Some(2), "level, then the upper one");
        assert_eq!(l.focus_dir(Down), Some(3));
        assert!(l.at_edge(Right) && l.at_edge(Down) && !l.at_edge(Up));
    }

    /// A smaller sheet (another screen): every edge in proportion; a window left with nothing is placed again.
    #[test]
    fn fits_another_sheet() {
        let mut l = Layout::default();
        l.sheet = (12, 7);
        l.tiles = vec![(1, z(0, 6, 0, 7)), (2, z(6, 12, 0, 4)), (3, z(6, 12, 4, 5))];
        l.focus = Some(0);
        l.fit(&sheet(6, 4));
        assert_eq!((at(&l, 1), at(&l, 2)), (z(0, 3, 0, 4), z(3, 6, 0, 2)));
        // 3 was one row of seven: rows 4..5 become 2..3 (rounded), still a zone
        assert_eq!(at(&l, 3), z(3, 6, 2, 3));
        assert!(l.take_evicted().is_empty());
        assert_eq!(l.focused(), Some(1));
    }

    /// At every scale, tiles land on whole physical pixels symmetric about the frame: each border abuts
    /// the frame's line, or the tick's pixel between two windows, and the window's logical place and size
    /// cover its pixels within the one pixel the renderer moves and cuts.
    #[test]
    fn tiles_are_exact() {
        for &(w, h, phys_mm) in &[(1920, 1200, 6.4), (3840, 2160, 6.1935), (2560, 1440, 4.13), (2880, 1800, 9.1)] {
            for &scale in &[1.0, 1.25, 1.5, 1.75, 2.0] {
                let size = Size::from(((w as f64 / scale).ceil() as i32, (h as f64 / scale).ceil() as i32));
                let at = Point::from((100, 0));
                let g = Grid::new(at, size, phys_mm / scale, 6.0, scale, (w, h));
                let (b, mg) = (g.border(), margin(6.0, phys_mm / scale, scale));
                let case = format!("{w}x{h} at {scale}");
                let whole = g.tile(0, g.zones.0, 0.0, g.zones.1 as f64).pixels;
                assert_eq!(whole.loc.x - b, mg + LINE, "{case}");
                assert_eq!(w - (whole.loc.x + whole.size.w + b), mg + LINE, "{case}");
                assert_eq!(h - (whole.loc.y + whole.size.h + b), mg + LINE, "{case}");
                for k in 1..g.zones.0 {
                    let (l, r) = (g.tile(0, k, 0.0, 1.0).pixels, g.tile(k, g.zones.0, 0.0, 1.0).pixels);
                    let (t, _) = tick(w, mg, g.zones.0, k);
                    assert_eq!(l.loc.x + l.size.w + b, t, "{case}: left border ends at the tick {k}");
                    assert_eq!(r.loc.x - b, t + 1, "{case}: right border starts after the tick {k}");
                }
                for x0 in 0..g.zones.0 {
                    for x1 in x0 + 1..=g.zones.0 {
                        let tile = g.tile(x0, x1, 0.0, g.zones.1 as f64);
                        let (p, l) = (tile.pixels, tile.logical);
                        let lx = ((l.loc.x - at.x) as f64 * scale).round() as i32;
                        assert!((lx - p.loc.x).abs() <= 1, "{case}: logical place {lx} off its pixels {}", p.loc.x);
                        assert!((l.size.w as f64 * scale).round() as i32 >= p.size.w, "{case}: logical size short of its pixels");
                        assert!(g.area.contains_rect(l) || (l.size.w as f64 * scale) - p.size.w as f64 <= scale, "{case}");
                    }
                }
            }
        }
    }
}
