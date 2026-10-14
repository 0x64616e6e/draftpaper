//! The layout: a workspace is a row of columns, a column a stack of rows, and both are measured in the
//! sheet's zones (widths in the zones across, heights in the zones down), so every window edge falls on
//! a zone boundary and lines up with the ticks in the margins. The last column and the last row end on
//! the frame.
//!
//! A new window opens a column to the right of the focused one and the zones are shared out again; once
//! there are as many columns as zones across, new windows stack as rows in the focused column instead.
//! (Which of the two a new window should do is issue #12.)

use smithay::{
    desktop::{Space, Window},
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Point, Rectangle, Size},
};

/// The frame's interior in logical pixels and its zones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grid {
    pub area: Rectangle<i32, Logical>,
    /// left edge of the zone grid (the frame line's outer edge), and the zone sizes
    pub origin: Point<f64, Logical>,
    pub zone: (f64, f64),
    pub zones: (u32, u32),
}

impl Grid {
    /// For an output of `size` logical pixels at `mm` logical pixels per millimetre, with a margin of
    /// `margin_mm`: the same arithmetic as the shader and render.py (margin rounded to whole pixels,
    /// zones of about 50 mm, at least two).
    pub fn new(size: Size<i32, Logical>, mm: f64, margin_mm: f64) -> Grid {
        let mg = (margin_mm * mm).round();
        let (w, h) = (size.w as f64, size.h as f64);
        let nx = ((w - 2.0 * mg) / (50.0 * mm)).round().max(2.0);
        let ny = ((h - 2.0 * mg) / (50.0 * mm)).round().max(2.0);
        // windows sit inside the 2 px frame line
        let inner = 2.0;
        Grid {
            area: Rectangle::new(
                ((mg + inner) as i32, (mg + inner) as i32).into(),
                ((w - 2.0 * (mg + inner)) as i32, (h - 2.0 * (mg + inner)) as i32).into(),
            ),
            origin: (mg, mg).into(),
            zone: ((w - 2.0 * mg) / nx, (h - 2.0 * mg) / ny),
            zones: (nx as u32, ny as u32),
        }
    }

    /// The x of the boundary `k` zones in, clamped to the frame's interior.
    fn x(&self, k: u32) -> i32 {
        let x = (self.origin.x + k as f64 * self.zone.0).round() as i32;
        x.clamp(self.area.loc.x, self.area.loc.x + self.area.size.w)
    }

    fn y(&self, k: f64) -> i32 {
        let y = (self.origin.y + k * self.zone.1).round() as i32;
        y.clamp(self.area.loc.y, self.area.loc.y + self.area.size.h)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug)]
struct Column {
    windows: Vec<Window>,
    /// width in zones across
    width: u32,
    /// height of each row in zones down (fractions only when rows outnumber the zones)
    heights: Vec<f64>,
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

    /// A new window: a new column to the right of the focused one, or, with every zone across taken
    /// by a column, a new row under the focused window.
    pub fn add(&mut self, window: Window, grid: &Grid) {
        let (fc, fr) = self.focus.unwrap_or((self.columns.len().saturating_sub(1), 0));
        if (self.columns.len() as u32) < grid.zones.0 {
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

    /// Place every window on the grid and tell it its size.
    pub fn arrange(&self, space: &mut Space<Window>, grid: &Grid) {
        let mut x0 = 0u32;
        let last_c = self.columns.len().saturating_sub(1);
        for (ci, col) in self.columns.iter().enumerate() {
            let x1 = if ci == last_c { grid.zones.0 } else { x0 + col.width };
            let (left, right) = (grid.x(x0), grid.x(x1));
            let mut y0 = 0.0;
            let last_r = col.windows.len().saturating_sub(1);
            for (ri, window) in col.windows.iter().enumerate() {
                let y1 = if ri == last_r { grid.zones.1 as f64 } else { y0 + col.heights[ri] };
                let (top, bottom) = (grid.y(y0), grid.y(y1));
                // a 1 px gutter either side of an inner boundary lets the paper show between windows
                let l = if ci == 0 { left } else { left + 1 };
                let r = if ci == last_c { right } else { right - 1 };
                let t = if ri == 0 { top } else { top + 1 };
                let b = if ri == last_r { bottom } else { bottom - 1 };
                let rect = Rectangle::new((l, t).into(), (r - l, b - t).into());
                if let Some(toplevel) = window.toplevel() {
                    toplevel.with_pending_state(|state| {
                        state.size = Some(rect.size);
                        state.states.set(xdg_toplevel::State::TiledLeft);
                        state.states.set(xdg_toplevel::State::TiledRight);
                        state.states.set(xdg_toplevel::State::TiledTop);
                        state.states.set(xdg_toplevel::State::TiledBottom);
                    });
                    if toplevel.is_initial_configure_sent() {
                        toplevel.send_pending_configure();
                    }
                }
                space.map_element(window.clone(), rect.loc, false);
                y0 = y1;
            }
            x0 = x1;
        }
    }
}
