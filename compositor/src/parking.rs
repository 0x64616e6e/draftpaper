//! Parking: a window goes into a cell of the side margins and comes back to its place in the layout.
//! Cells count down the left margin, then the right, from 0, one per zone down (so a screen has twice
//! its zones down); each workspace has its own. A parked window leaves the space; its workspace keeps it,
//! with its cell and its placeholder (layout.rs), so it comes back where it was.
//!
//! The gestures are the i3 edition's (`draftpaper park`, 0.1):
//! - park: the focused window into the first free cell; on a workspace with no window showing, every
//!   parked window back, last parked first (so the layout comes back exactly);
//! - park_at: the focused window into a given cell (a click on an empty cell);
//! - unpark: one window back (a click on its title), switching to its workspace;
//! - park_next / park_prev: the parked window in the first (last) cell comes back and the focused one
//!   takes the next free cell after the last (before the first) taken one, round the margin; with
//!   nothing parked, the focus moves right (left).

use smithay::{desktop::Window, utils::Rectangle};

use crate::{config::Action, layout::{Direction, Placeholder}, Draftpaper};

/// Where a parked window goes back to.
#[derive(Debug, Clone)]
pub enum Spot {
    Tiled(Placeholder),
    Floating,
}

#[derive(Debug, Clone)]
pub struct Parked {
    pub window: Window,
    pub cell: usize,
    spot: Spot,
}

/// The first free cell after `after` going `step`, round the margin; past the end when all are taken.
fn free(taken: &[usize], total: usize, after: isize, step: isize) -> usize {
    let n = total as isize;
    (1..=n)
        .map(|k| (after + step * k).rem_euclid(n.max(1)) as usize)
        .find(|c| !taken.contains(c))
        .unwrap_or_else(|| taken.iter().copied().max().unwrap_or(0).max(total.saturating_sub(1)) + 1)
}

impl Draftpaper {
    /// The cells of screen `i`: one per zone down each side margin.
    pub fn cells(&self, i: usize) -> usize {
        2 * self.screens[i].grid.zones.1 as usize
    }

    /// The focused window, if it is on the workspace shown on the active screen (tiled, floating or
    /// fullscreen).
    fn focused_here(&self) -> Option<Window> {
        let w = self.focused_window()?;
        self.screens.get(self.active)?.workspace().holds(&w).then_some(w)
    }

    fn park_window(&mut self, window: Window, cell: usize) {
        let i = self.active;
        let grid = self.screens[i].grid;
        let ws = self.screens[i].workspace_mut();
        if ws.fullscreen.as_ref() == Some(&window) {
            ws.fullscreen = None;
        }
        let spot = if let Some(k) = ws.floating.iter().position(|w| w == &window) {
            ws.floating.remove(k);
            Spot::Floating
        } else {
            match ws.layout.park(&window, &grid) {
                Some(p) => Spot::Tiled(p),
                None => return,
            }
        };
        ws.parked.push(Parked { window: window.clone(), cell, spot });
        // from where it was drawn into its cell
        if let Some(loc) = self.space.element_location(&window) {
            self.animate_park(&window, Rectangle::new(loc, window.geometry().size), i, cell);
        }
        self.space.unmap_elem(&window);
        window.set_activated(false);
        if let Some(t) = window.toplevel() {
            t.send_pending_configure();
        }
        self.relayout();
        let next = self.screens[i].layout().focused();
        self.focus(next);
    }

    /// Back into its workspace (screen `i`, workspace `k`, the parked entry `p`); not focused. Returns
    /// the window and the cell it was in.
    fn unpark_window(&mut self, i: usize, k: usize, p: usize) -> (Window, usize) {
        let grid = self.screens[i].grid;
        let ws = &mut self.screens[i].workspaces[k];
        let parked = ws.parked.remove(p);
        match &parked.spot {
            // with no zone left for it (Layout::add), it floats
            Spot::Tiled(placeholder) if ws.layout.unpark(parked.window.clone(), placeholder, &grid) => {}
            _ => ws.floating.push(parked.window.clone()),
        }
        (parked.window, parked.cell)
    }

    fn taken_cells(&self) -> Vec<usize> {
        self.screens.get(self.active).map(|s| s.workspace().parked.iter().map(|p| p.cell).collect()).unwrap_or_default()
    }

    pub fn park_toggle(&mut self) {
        let Some(s) = self.screens.get(self.active) else { return };
        if let Some(window) = self.focused_here() {
            let cell = free(&self.taken_cells(), self.cells(self.active), -1, 1);
            self.park_window(window, cell);
        } else if s.workspace().layout.windows().next().is_none() && s.workspace().floating.is_empty() {
            // nothing showing: everything back, last parked first
            let (i, k) = (self.active, s.current);
            let mut back = Vec::new();
            while !self.screens[i].workspaces[k].parked.is_empty() {
                let p = self.screens[i].workspaces[k].parked.len() - 1;
                back.push(self.unpark_window(i, k, p));
            }
            self.relayout();
            for (window, cell) in &back {
                self.animate_unpark(window, i, *cell);
            }
            if let Some((last, _)) = back.pop() {
                self.focus(Some(last));
            }
        }
    }

    pub fn park_at(&mut self, cell: usize, workspace: Option<u32>) {
        let Some(s) = self.screens.get(self.active) else { return };
        if workspace.is_some_and(|n| n != s.workspace().number) || cell >= self.cells(self.active) || self.taken_cells().contains(&cell) {
            return;
        }
        if let Some(window) = self.focused_here() {
            self.park_window(window, cell);
        }
    }

    pub fn unpark(&mut self, id: u64) {
        let found = self.screens.iter().enumerate().find_map(|(i, s)| {
            s.workspaces.iter().enumerate().find_map(|(k, w)| {
                w.parked.iter().position(|p| crate::ipc::window_id(&p.window) == id).map(|p| (i, k, p, w.number))
            })
        });
        let Some((i, k, p, number)) = found else { return };
        let (window, cell) = self.unpark_window(i, k, p);
        if self.screens[i].current != k {
            self.switch_to(number);
        }
        self.relayout();
        self.animate_unpark(&window, i, cell);
        self.focus(Some(window));
    }

    pub fn park_cycle(&mut self, d: i32) {
        let Some(s) = self.screens.get(self.active) else { return };
        let mut rows: Vec<(usize, u64)> = s.workspace().parked.iter().map(|p| (p.cell, crate::ipc::window_id(&p.window))).collect();
        if rows.is_empty() {
            self.act(Action::Focus(if d > 0 { Direction::Right } else { Direction::Left }));
            return;
        }
        rows.sort();
        let (first, last) = (rows[0], rows[rows.len() - 1]);
        let back = if d > 0 { first } else { last };
        if let Some(window) = self.focused_here() {
            let others: Vec<usize> = rows.iter().filter(|r| r.1 != back.1).map(|r| r.0).collect();
            let total = self.cells(self.active);
            let cell = if d > 0 { free(&others, total, last.0 as isize, 1) } else { free(&others, total, first.0 as isize, -1) };
            self.park_window(window, cell);
        }
        self.unpark(back.1);
    }
}

#[cfg(test)]
mod tests {
    use super::free;

    #[test]
    fn free_cells() {
        assert_eq!(free(&[], 8, -1, 1), 0);
        assert_eq!(free(&[0, 1], 8, -1, 1), 2);
        assert_eq!(free(&[2], 8, 2, 1), 3);
        assert_eq!(free(&[7], 8, 7, 1), 0);           // round the margin
        assert_eq!(free(&[0], 8, 0, -1), 7);          // backwards, round
        assert_eq!(free(&[0, 1, 2, 3], 4, -1, 1), 4); // all taken: past the end
    }
}
