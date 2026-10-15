//! The layout: a workspace is a row of columns, a column a stack of rows, and both are measured in the
//! sheet's zones (widths in the zones across, heights in the zones down), so every window edge falls on
//! a zone boundary and lines up with the ticks in the margins. The last column and the last row end on
//! the frame.
//!
//! The first window takes the frame and the second opens a column beside it. After that, config.toml's
//! `[layout] new_window` decides (issue #12): "column", a new column to the right of the focused one
//! (rows in the focused column once there are as many columns as zones across); or "row", a new row
//! under the focused window (a new column once the column has as many rows as zones down).
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
}

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
    before: Vec<(Vec<Window>, u32, Vec<f64>)>,
}

#[derive(Debug, Default)]
pub struct Layout {
    columns: Vec<Column>,
    /// focused (column, row)
    focus: Option<(usize, usize)>,
}

/// `total` zones shared among `n`: whole zones, the extra ones to the leftmost (or topmost).
fn share(total: u32, n: usize) -> Vec<u32> {
    let n = n.max(1) as u32;
    (0..n).map(|i| total / n + u32::from(i < total % n)).collect()
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
        let widths = share(grid.zones.0, self.columns.len());
        for (col, w) in self.columns.iter_mut().zip(widths) {
            col.width = w;
        }
    }

    fn reshare_rows(col: &mut Column, grid: &Grid) {
        let n = col.windows.len();
        col.heights = if n as u32 <= grid.zones.1 {
            share(grid.zones.1, n).into_iter().map(f64::from).collect()
        } else {
            vec![grid.zones.1 as f64 / n as f64; n]
        };
    }

    /// A new window: a new column to the right of the focused one, or a new row under the focused
    /// window, as `mode` says (see the top of this file).
    pub fn add(&mut self, window: Window, grid: &Grid, mode: NewWindow) {
        let (fc, fr) = self.focus.unwrap_or((self.columns.len().saturating_sub(1), 0));
        let room_across = (self.columns.len() as u32) < grid.zones.0;
        let room_down = self.columns.get(fc).is_some_and(|c| (c.windows.len() as u32) < grid.zones.1);
        let as_column = self.columns.is_empty()
            || match mode {
                NewWindow::Column => room_across,
                NewWindow::Row => room_across && (self.columns.len() < 2 || !room_down),
            };
        if as_column {
            let at = if self.columns.is_empty() { 0 } else { fc + 1 };
            self.columns.insert(at, Column { windows: vec![window], width: 1, heights: vec![] });
            Self::reshare_rows(&mut self.columns[at], grid);
            self.reshare_columns(grid);
            self.focus = Some((at, 0));
        } else {
            let col = &mut self.columns[fc];
            col.windows.insert(fr + 1, window);
            Self::reshare_rows(col, grid);
            self.focus = Some((fc, fr + 1));
        }
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
    fn snapshot(&self) -> Vec<(Vec<Window>, u32, Vec<f64>)> {
        self.columns.iter().map(|c| (c.windows.clone(), c.width, c.heights.clone())).collect()
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
        };
        self.remove(window, grid);
        Some(placeholder)
    }

    /// Put a parked window back where its placeholder says: into its column between the same
    /// neighbours, or as its own column between the same columns (at the nearest place when those have
    /// gone). With the layout as it was before, the sizes are as they were too.
    pub fn unpark(&mut self, window: Window, placeholder: &Placeholder, grid: &Grid, mode: NewWindow) {
        let column_of = |ws: &[Window], cols: &[Column]| cols.iter().position(|c| c.windows.iter().any(|w| ws.contains(w)));
        if let Some(c) = (!placeholder.column.is_empty()).then(|| column_of(&placeholder.column, &self.columns)).flatten() {
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
            self.columns.insert(c, Column { windows: vec![window], width: 1, heights: vec![] });
            Self::reshare_rows(&mut self.columns[c], grid);
            self.reshare_columns(grid);
            self.focus = Some((c, 0));
        } else {
            // no room for a column of its own: placed like a new window
            self.add(window, grid, mode);
        }
        let windows = |s: &[(Vec<Window>, u32, Vec<f64>)]| s.iter().map(|c| c.0.clone()).collect::<Vec<_>>();
        if windows(&self.snapshot()) == windows(&placeholder.before) {
            for (col, (_, width, heights)) in self.columns.iter_mut().zip(&placeholder.before) {
                col.width = *width;
                col.heights = heights.clone();
            }
        }
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
    /// column at the edge), up/down within its column.
    pub fn move_dir(&mut self, dir: Direction, grid: &Grid) {
        let Some((c, r)) = self.focus else { return };
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
                    None if !alone && (self.columns.len() as u32) < grid.zones.0 => {
                        let at = if dir == Direction::Left { 0 } else { self.columns.len() };
                        Self::reshare_rows(&mut self.columns[c], grid);
                        self.columns.insert(at, Column { windows: vec![window], width: 1, heights: vec![] });
                        Self::reshare_rows(&mut self.columns[at], grid);
                        self.focus = Some((at, 0));
                    }
                    None => self.columns[c].windows.insert(r, window),   // nowhere to go
                }
                self.reshare_columns(grid);
            }
        }
    }

    /// Resize by one zone: left/right moves the boundary on the focused column's right (on its left for
    /// the last column) one zone that way, up/down the boundary under the focused row (over it for the
    /// last row); the window on the other side gives or takes the zone.
    pub fn resize(&mut self, dir: Direction) {
        let Some((c, r)) = self.focus else { return };
        let grow = matches!(dir, Direction::Right | Direction::Down);
        match dir {
            Direction::Left | Direction::Right => {
                let n = self.columns.len();
                if n < 2 {
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
        let mut x0 = 0u32;
        let last_c = self.columns.len().saturating_sub(1);
        for (ci, col) in self.columns.iter().enumerate() {
            let x1 = if ci == last_c { grid.zones.0 } else { x0 + col.width };
            let mut y0 = 0.0;
            let last_r = col.windows.len().saturating_sub(1);
            for (ri, window) in col.windows.iter().enumerate() {
                let y1 = if ri == last_r { grid.zones.1 as f64 } else { y0 + col.heights[ri] };
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
