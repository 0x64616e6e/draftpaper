//! The layout: a workspace is a row of columns, a column a stack of rows, and both are measured in the
//! sheet's zones (widths in the zones across, heights in the zones down), so every window edge falls on
//! a zone boundary and lines up with the ticks in the margins, and every window is a zone at least. The
//! last column and the last row end on the frame, except a window alone across or down: resized in from
//! it (Layout::resize), it can be moved about the empty zones in zone steps (Layout::move_dir).
//!
//! The first window takes the frame and the second opens a column beside it. After that, config.toml's
//! `[layout] new_window` decides (issue #12): "column", a new column to the right of the focused one
//! (rows in the focused column once there are as many columns as zones across); or "row", a new row
//! under the focused window (a new column once the column has as many rows as zones down). With every
//! zone taken, a new window floats.
//!
//! A parked window leaves the layout and leaves a placeholder (Placeholder): its neighbours and the
//! layout's sizes then, so it comes back to the same place, and to the same sizes when nothing else
//! changed meanwhile.

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

/// Where a new window goes once there are two (config.toml, `[layout] new_window`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NewWindow {
    #[default]
    Column,
    Row,
}

#[derive(Debug)]
struct Column {
    windows: Vec<Window>,
    /// width in zones across
    width: u32,
    /// height of each row in zones down (fractions only when rows outnumber the zones)
    heights: Vec<f64>,
    /// zones down left empty above and below a window alone in its column (resized or moved in zones)
    gap: (u32, u32),
}

impl Column {
    fn new(window: Window) -> Column {
        Column { windows: vec![window], width: 1, heights: vec![], gap: (0, 0) }
    }
}

/// A column as Layout::snapshot keeps it: its windows, width, rows' heights and empty zones.
type Snapshot = (Vec<Window>, u32, Vec<f64>, (u32, u32));

/// Where a parked window was in the layout, to put it back there (Layout::park, Layout::unpark).
#[derive(Debug, Clone)]
pub struct Placeholder {
    /// in a column with other windows: those windows, and its neighbours above and below
    column: Vec<Window>,
    above: Option<Window>,
    below: Option<Window>,
    /// the windows of the columns left and right of its column (for a column of its own)
    left: Vec<Window>,
    right: Vec<Window>,
    /// its column and row then: the last resort
    index: (usize, usize),
    /// the layout before it left; when the layout is that again once it is back, its sizes are too
    before: Vec<Snapshot>,
    gap: (u32, u32),
}

#[derive(Debug, Default)]
pub struct Layout {
    columns: Vec<Column>,
    /// focused (column, row)
    focus: Option<(usize, usize)>,
    /// zones across left empty left and right of a column alone on the sheet (resized or moved in zones)
    gap: (u32, u32),
}

/// `total` zones shared among `n`: whole zones, the extra ones to the leftmost (or topmost).
fn share(total: u32, n: usize) -> Vec<u32> {
    let n = n.max(1) as u32;
    (0..n).map(|i| total / n + u32::from(i < total % n)).collect()
}

/// A window alone along a side of `zones` zones, with `gap` empty zones before and after it, resized by
/// a zone: shrinking moves its far edge in (it keeps a zone at least); growing takes the room after it
/// back, then the room before.
fn resize_gap(gap: &mut (u32, u32), zones: u32, grow: bool) {
    if grow {
        if gap.1 > 0 {
            gap.1 -= 1;
        } else if gap.0 > 0 {
            gap.0 -= 1;
        }
    } else if zones.saturating_sub(gap.0 + gap.1) > 1 {
        gap.1 += 1;
    }
}

/// The same window moved a zone forward (right, down) or back, keeping its size; false at the frame.
fn shift_gap(gap: &mut (u32, u32), forward: bool) -> bool {
    match forward {
        true if gap.1 > 0 => {
            gap.1 -= 1;
            gap.0 += 1;
            true
        }
        false if gap.0 > 0 => {
            gap.0 -= 1;
            gap.1 += 1;
            true
        }
        _ => false,
    }
}

/// The zones a window alone spans along a side of `zones`, given its `gap`: kept inside the side, a zone
/// at least, when the side has fewer zones than when the gap was made.
fn span(zones: u32, gap: (u32, u32)) -> (u32, u32) {
    let from = gap.0.min(zones.saturating_sub(1));
    (from, zones.saturating_sub(gap.1).max(from + 1).min(zones.max(1)))
}

impl Layout {
    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.columns.iter().flat_map(|c| c.windows.iter())
    }

    pub fn contains(&self, window: &Window) -> bool {
        self.find(window).is_some()
    }

    /// Whether the focus is already as far as it goes towards `dir` (the next step leaves the screen).
    pub fn at_edge(&self, dir: Direction) -> bool {
        match (self.focus, dir) {
            (None, _) => true,
            (Some((c, _)), Direction::Left) => c == 0,
            (Some((c, _)), Direction::Right) => c + 1 >= self.columns.len(),
            _ => false,
        }
    }

    /// Whether the focused window is alone that way (on the sheet across, in its column down) with empty
    /// zones beside it to move into (move_dir).
    pub fn can_shift(&self, dir: Direction) -> bool {
        let Some((c, _)) = self.focus else { return false };
        match dir {
            Direction::Left | Direction::Right if self.columns.len() == 1 && self.columns[0].windows.len() == 1 => {
                if dir == Direction::Left { self.gap.0 > 0 } else { self.gap.1 > 0 }
            }
            Direction::Up | Direction::Down if self.columns[c].windows.len() == 1 => {
                let gap = self.columns[c].gap;
                if dir == Direction::Up { gap.0 > 0 } else { gap.1 > 0 }
            }
            _ => false,
        }
    }

    pub fn focused(&self) -> Option<Window> {
        self.focus.and_then(|(c, r)| self.columns.get(c)?.windows.get(r).cloned())
    }

    fn find(&self, window: &Window) -> Option<(usize, usize)> {
        self.columns.iter().enumerate().find_map(|(c, col)| col.windows.iter().position(|w| w == window).map(|r| (c, r)))
    }

    pub fn focus_window(&mut self, window: &Window) {
        if let Some(at) = self.find(window) {
            self.focus = Some(at);
        }
    }

    fn reshare_columns(&mut self, grid: &Grid) {
        self.gap = (0, 0);
        let widths = share(grid.zones.0, self.columns.len());
        for (col, w) in self.columns.iter_mut().zip(widths) {
            col.width = w;
        }
    }

    fn reshare_rows(col: &mut Column, grid: &Grid) {
        let n = col.windows.len();
        col.gap = (0, 0);
        col.heights = if n as u32 <= grid.zones.1 {
            share(grid.zones.1, n).into_iter().map(f64::from).collect()
        } else {
            vec![grid.zones.1 as f64 / n as f64; n]
        };
    }

    /// A new window: a new column to the right of the focused one, or a new row under the focused
    /// window, as `mode` says (see the top of this file).
    /// Every window keeps a zone at least: with every zone taken (as many columns as zones across, each
    /// with as many rows as zones down) there is no room, and the window is not added (false; the caller
    /// floats it).
    #[must_use]
    pub fn add(&mut self, window: Window, grid: &Grid, mode: NewWindow) -> bool {
        let (fc, fr) = self.focus.unwrap_or((self.columns.len().saturating_sub(1), 0));
        let room_across = (self.columns.len() as u32) < grid.zones.0;
        let room = |c: &Column| (c.windows.len() as u32) < grid.zones.1;
        let room_down = self.columns.get(fc).is_some_and(room);
        let as_column = self.columns.is_empty()
            || match mode {
                NewWindow::Column => room_across,
                NewWindow::Row => room_across && (self.columns.len() < 2 || !room_down),
            };
        if as_column {
            let at = if self.columns.is_empty() { 0 } else { fc + 1 };
            self.columns.insert(at, Column::new(window));
            Self::reshare_rows(&mut self.columns[at], grid);
            self.reshare_columns(grid);
            self.focus = Some((at, 0));
            return true;
        }
        // a row: under the focused window, else at the foot of the nearest column with room
        let near = (0..self.columns.len()).filter(|&c| room(&self.columns[c])).min_by_key(|&c| c.abs_diff(fc));
        let Some(c) = (if room_down { Some(fc) } else { near }) else { return false };
        let r = if c == fc { fr + 1 } else { self.columns[c].windows.len() };
        let col = &mut self.columns[c];
        col.windows.insert(r, window);
        Self::reshare_rows(col, grid);
        self.focus = Some((c, r));
        true
    }

    pub fn remove(&mut self, window: &Window, grid: &Grid) {
        let Some((c, r)) = self.find(window) else { return };
        self.columns[c].windows.remove(r);
        if self.columns[c].windows.is_empty() {
            self.columns.remove(c);
            self.reshare_columns(grid);
            self.focus = if self.columns.is_empty() { None } else { Some((c.min(self.columns.len() - 1), 0)) };
        } else {
            Self::reshare_rows(&mut self.columns[c], grid);
            self.focus = Some((c, r.min(self.columns[c].windows.len() - 1)));
        }
    }

    /// The windows column by column, with the columns' widths and the rows' heights.
    fn snapshot(&self) -> Vec<Snapshot> {
        self.columns.iter().map(|c| (c.windows.clone(), c.width, c.heights.clone(), c.gap)).collect()
    }

    /// Take `window` out of the layout, remembering where it was.
    pub fn park(&mut self, window: &Window, grid: &Grid) -> Option<Placeholder> {
        let (c, r) = self.find(window)?;
        let col = &self.columns[c].windows;
        let others: Vec<Window> = col.iter().filter(|w| *w != window).cloned().collect();
        let placeholder = Placeholder {
            above: if others.is_empty() { None } else { r.checked_sub(1).map(|i| col[i].clone()) },
            below: if others.is_empty() { None } else { col.get(r + 1).cloned() },
            column: others,
            left: c.checked_sub(1).map(|i| self.columns[i].windows.clone()).unwrap_or_default(),
            right: self.columns.get(c + 1).map(|col| col.windows.clone()).unwrap_or_default(),
            index: (c, r),
            before: self.snapshot(),
            gap: self.gap,
        };
        self.remove(window, grid);
        Some(placeholder)
    }

    /// Put a parked window back where its placeholder says: into its column between the same
    /// neighbours, or as its own column between the same columns (at the nearest place when those have
    /// gone). With the layout as it was before, the sizes are as they were too. False when there is no
    /// zone for it (Layout::add): the caller floats it.
    #[must_use]
    pub fn unpark(&mut self, window: Window, placeholder: &Placeholder, grid: &Grid, mode: NewWindow) -> bool {
        let column_of = |ws: &[Window], cols: &[Column]| cols.iter().position(|c| c.windows.iter().any(|w| ws.contains(w)));
        let column = (!placeholder.column.is_empty()).then(|| column_of(&placeholder.column, &self.columns)).flatten();
        if let Some(c) = column.filter(|&c| (self.columns[c].windows.len() as u32) < grid.zones.1) {
            let col = &self.columns[c].windows;
            let r = placeholder
                .above
                .as_ref()
                .and_then(|a| col.iter().position(|w| w == a))
                .map(|i| i + 1)
                .or_else(|| placeholder.below.as_ref().and_then(|b| col.iter().position(|w| w == b)))
                .unwrap_or(placeholder.index.1.min(col.len()));
            self.columns[c].windows.insert(r, window);
            Self::reshare_rows(&mut self.columns[c], grid);
            self.focus = Some((c, r));
        } else if (self.columns.len() as u32) < grid.zones.0 {
            let c = column_of(&placeholder.left, &self.columns)
                .map(|i| i + 1)
                .or_else(|| column_of(&placeholder.right, &self.columns))
                .unwrap_or(placeholder.index.0.min(self.columns.len()));
            self.columns.insert(c, Column::new(window));
            Self::reshare_rows(&mut self.columns[c], grid);
            self.reshare_columns(grid);
            self.focus = Some((c, 0));
        } else {
            // no room for a column of its own: placed like a new window, or nowhere
            if !self.add(window, grid, mode) {
                return false;
            }
        }
        let windows = |s: &[Snapshot]| s.iter().map(|c| c.0.clone()).collect::<Vec<_>>();
        if windows(&self.snapshot()) == windows(&placeholder.before) {
            for (col, (_, width, heights, gap)) in self.columns.iter_mut().zip(&placeholder.before) {
                col.width = *width;
                col.heights = heights.clone();
                col.gap = *gap;
            }
            self.gap = placeholder.gap;
        }
        true
    }

    /// Move the focus; returns the newly focused window.
    pub fn focus_dir(&mut self, dir: Direction) -> Option<Window> {
        let (c, r) = self.focus?;
        let (nc, nr) = match dir {
            Direction::Left if c > 0 => (c - 1, r.min(self.columns[c - 1].windows.len() - 1)),
            Direction::Right if c + 1 < self.columns.len() => (c + 1, r.min(self.columns[c + 1].windows.len() - 1)),
            Direction::Up if r > 0 => (c, r - 1),
            Direction::Down if r + 1 < self.columns[c].windows.len() => (c, r + 1),
            _ => (c, r),
        };
        self.focus = Some((nc, nr));
        self.focused()
    }

    /// Move the focused window: left/right into the neighbouring column as a row (or out to a new
    /// column at the edge), up/down within its column; a window alone that way (on the sheet across, in its
    /// column down) a zone into the empty zones beside it. Into a full column, or nowhere, it stays.
    pub fn move_dir(&mut self, dir: Direction, grid: &Grid) {
        let Some((c, r)) = self.focus else { return };
        // a window alone that way: a zone step into the empty zones beside it, keeping its size
        if self.can_shift(dir) {
            let forward = matches!(dir, Direction::Right | Direction::Down);
            let gap = if matches!(dir, Direction::Left | Direction::Right) { &mut self.gap } else { &mut self.columns[c].gap };
            shift_gap(gap, forward);
            return;
        }
        match dir {
            Direction::Up | Direction::Down => {
                let col = &mut self.columns[c];
                let to = if dir == Direction::Up { r.checked_sub(1) } else { (r + 1 < col.windows.len()).then_some(r + 1) };
                if let Some(to) = to {
                    col.windows.swap(r, to);
                    self.focus = Some((c, to));
                }
            }
            Direction::Left | Direction::Right => {
                let alone = self.columns[c].windows.len() == 1;
                let target = if dir == Direction::Left { c.checked_sub(1) } else { (c + 1 < self.columns.len()).then_some(c + 1) };
                // nowhere to go: a full column beside it (a row there would be less than a zone), or the
                // frame with no room for a column of its own; then nothing changes, sizes included
                let room = |col: &Column| (col.windows.len() as u32) < grid.zones.1;
                let goes = match target {
                    Some(t) => room(&self.columns[t]),
                    None => !alone && (self.columns.len() as u32) < grid.zones.0,
                };
                if !goes {
                    return;
                }
                let window = self.columns[c].windows.remove(r);
                match target {
                    Some(t) => {
                        self.columns[t].windows.push(window);
                        let at = (t, self.columns[t].windows.len() - 1);
                        Self::reshare_rows(&mut self.columns[t], grid);
                        if alone {
                            self.columns.remove(c);
                            self.focus = Some((if t > c { t - 1 } else { t }, at.1));
                        } else {
                            Self::reshare_rows(&mut self.columns[c], grid);
                            self.focus = Some(at);
                        }
                    }
                    None => {
                        let at = if dir == Direction::Left { 0 } else { self.columns.len() };
                        Self::reshare_rows(&mut self.columns[c], grid);
                        self.columns.insert(at, Column::new(window));
                        Self::reshare_rows(&mut self.columns[at], grid);
                        self.focus = Some((at, 0));
                    }
                }
                self.reshare_columns(grid);
            }
        }
    }

    /// Resize by one zone: left/right moves the boundary on the focused column's right (on its left for
    /// the last column) one zone that way, up/down the boundary under the focused row (over it for the
    /// last row); the window on the other side gives or takes the zone. A window alone across (or down)
    /// moves its right (or bottom) edge instead, in from the frame (to a zone) and back out, then its
    /// left (or top) edge out to the frame; the zones it leaves stay empty until a neighbour comes.
    pub fn resize(&mut self, dir: Direction, grid: &Grid) {
        let Some((c, r)) = self.focus else { return };
        let grow = matches!(dir, Direction::Right | Direction::Down);
        match dir {
            Direction::Left | Direction::Right => {
                let n = self.columns.len();
                if n < 2 {
                    resize_gap(&mut self.gap, grid.zones.0, grow);
                    return;
                }
                let (a, b) = if c + 1 < n { (c, c + 1) } else { (c - 1, c) };
                let (from, to) = if grow { (b, a) } else { (a, b) };   // the boundary moves right: the left side grows
                if self.columns[from].width > 1 {
                    self.columns[from].width -= 1;
                    self.columns[to].width += 1;
                }
            }
            Direction::Up | Direction::Down => {
                let col = &mut self.columns[c];
                let n = col.windows.len();
                if n < 2 {
                    resize_gap(&mut col.gap, grid.zones.1, grow);
                    return;
                }
                let (a, b) = if r + 1 < n { (r, r + 1) } else { (r - 1, r) };
                let (from, to) = if grow { (b, a) } else { (a, b) };   // the boundary moves down: the upper side grows
                if col.heights[from] >= 2.0 {
                    col.heights[from] -= 1.0;
                    col.heights[to] += 1.0;
                }
            }
        }
    }

    /// Place every window on the grid and tell it its size. Each keeps its tile (tile_of), for the renderer.
    /// `skip` is a fullscreen window, placed by the caller.
    pub fn arrange(&self, space: &mut Space<Window>, grid: &Grid, skip: Option<&Window>) {
        // a column alone on the sheet spans what its gap leaves, and a window alone in its column likewise
        let lone = self.columns.len() == 1;
        let mut x0 = if lone { span(grid.zones.0, self.gap).0 } else { 0 };
        let last_c = self.columns.len().saturating_sub(1);
        for (ci, col) in self.columns.iter().enumerate() {
            let x1 = match (lone, ci == last_c) {
                (true, _) => span(grid.zones.0, self.gap).1,
                (false, true) => grid.zones.0.max(x0 + 1),
                (false, false) => x0 + col.width,
            };
            let single = col.windows.len() == 1;
            let mut y0 = if single { span(grid.zones.1, col.gap).0 as f64 } else { 0.0 };
            let last_r = col.windows.len().saturating_sub(1);
            for (ri, window) in col.windows.iter().enumerate() {
                let y1 = match (single, ri == last_r) {
                    (true, _) => span(grid.zones.1, col.gap).1 as f64,
                    (false, true) => (grid.zones.1 as f64).max(y0 + 1.0),
                    (false, false) => y0 + col.heights[ri],
                };
                let tile = grid.tile(x0, x1, y0, y1);
                let rect = tile.logical;
                set_tile(window, Some(tile));
                if Some(window) == skip {
                    y0 = y1;
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
                y0 = y1;
            }
            x0 = x1;
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

    /// A window alone, resized and moved in zones along a side of 6: a zone at least, never past the frame,
    /// its size kept as it moves; and its span inside a side that has fewer zones since.
    #[test]
    fn a_window_alone_steps_in_zones() {
        let mut gap = (0, 0);
        for _ in 0..10 {
            resize_gap(&mut gap, 6, false);
        }
        assert_eq!(gap, (0, 5), "shrinks to one zone, no further");
        assert!(!shift_gap(&mut gap, false), "at the frame, no step back");
        for _ in 0..3 {
            assert!(shift_gap(&mut gap, true));
        }
        assert_eq!((gap, span(6, gap)), ((3, 2), (3, 4)), "three zones on, still one zone wide");
        resize_gap(&mut gap, 6, true);
        resize_gap(&mut gap, 6, true);
        assert_eq!(gap, (3, 0), "grows to the frame on its far side first");
        resize_gap(&mut gap, 6, true);
        assert_eq!(gap, (2, 0), "then back towards the near side");
        assert!(!shift_gap(&mut gap, true), "at the frame, no step on");
        // a side with fewer zones (another screen): still inside it, a zone at least
        assert_eq!(span(2, (3, 0)), (1, 2));
        assert_eq!(span(4, (0, 5)), (0, 1));
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
