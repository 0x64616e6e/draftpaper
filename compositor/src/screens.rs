//! Screens: every output has its own zone grid and layout, side by side in one coordinate space. New
//! windows open on the active screen (that of the focused window, or the one under the pointer); focus
//! and moves cross to the neighbouring screen at the edge.

use smithay::{
    desktop::Window,
    output::Output,
    utils::{Logical, Point},
};

use crate::{
    layout::{Direction, Grid, Layout},
    state::Draftpaper,
};

pub struct Screen {
    pub output: Output,
    pub grid: Grid,
    pub layout: Layout,
    /// logical pixels per millimetre (for the grid when the output changes size or scale)
    pub mm: f64,
    pub margin_mm: f64,
}

impl Draftpaper {
    /// Add a screen for `output`, already mapped in the space; `mm` in physical pixels per millimetre.
    pub fn add_screen(&mut self, output: Output, mm: f64, margin_mm: f64) {
        let scale = output.current_scale().fractional_scale();
        let geometry = self.space.output_geometry(&output).unwrap();
        let mm = mm / scale;
        let grid = Grid::new(geometry.loc, geometry.size, mm, margin_mm);
        self.screens.push(Screen { output, grid, layout: Layout::default(), mm, margin_mm });
        self.screens.sort_by_key(|s| s.grid.area.loc.x);
    }

    /// Remove the screen of `output`; its windows join the first remaining screen.
    pub fn remove_screen(&mut self, output: &Output) {
        let Some(i) = self.screens.iter().position(|s| &s.output == output) else { return };
        let gone = self.screens.remove(i);
        if let Some(to) = self.screens.first_mut() {
            for w in gone.layout.windows() {
                to.layout.add(w.clone(), &to.grid);
            }
        }
        self.active = self.active.min(self.screens.len().saturating_sub(1));
        self.relayout();
    }

    /// Recompute a screen's grid after its output changed size, scale or position.
    pub fn update_screen(&mut self, output: &Output) {
        let Some(geometry) = self.space.output_geometry(output) else { return };
        if let Some(s) = self.screens.iter_mut().find(|s| &s.output == output) {
            s.grid = Grid::new(geometry.loc, geometry.size, s.mm, s.margin_mm);
        }
        self.relayout();
    }

    pub fn screen_of(&self, window: &Window) -> Option<usize> {
        self.screens.iter().position(|s| s.layout.contains(window))
    }

    pub fn screen_at(&self, at: Point<f64, Logical>) -> Option<usize> {
        self.screens.iter().position(|s| self.space.output_geometry(&s.output).is_some_and(|g| g.to_f64().contains(at)))
    }

    pub fn active_screen(&mut self) -> Option<&mut Screen> {
        self.screens.get_mut(self.active)
    }

    /// The screen beside `i` towards `dir` (screens are kept sorted left to right).
    pub fn neighbour(&self, i: usize, dir: Direction) -> Option<usize> {
        match dir {
            Direction::Left => i.checked_sub(1),
            Direction::Right => (i + 1 < self.screens.len()).then_some(i + 1),
            _ => None,
        }
    }
}
