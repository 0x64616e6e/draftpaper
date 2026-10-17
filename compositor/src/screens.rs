//! Screens and workspaces. Every output has its own zone grid, side by side in one coordinate space, and
//! shows one of its workspaces; each workspace has its own layout. Workspaces are numbered globally, as
//! in i3: a number belongs to one screen, switching to it shows it there and makes that screen active.
//! New windows open on the active screen's workspace; focus and moves cross to the neighbouring screen
//! at the edge.

use smithay::{
    desktop::Window,
    output::Output,
    utils::{Logical, Point},
};

use crate::{
    layout::{Direction, Grid, Layout},
    state::Draftpaper,
};

pub struct Workspace {
    pub number: u32,
    pub layout: Layout,
    /// a window covering the whole output, sheet included
    pub fullscreen: Option<Window>,
    /// dialogs (windows with a parent): centred inside the frame, above the tiles
    pub floating: Vec<Window>,
    /// parked windows, in the order they were parked (parking.rs)
    pub parked: Vec<crate::parking::Parked>,
}

impl Workspace {
    pub fn new(number: u32) -> Workspace {
        Workspace { number, layout: Layout::default(), fullscreen: None, floating: Vec::new(), parked: Vec::new() }
    }

    pub fn holds(&self, window: &Window) -> bool {
        self.layout.contains(window) || self.floating.contains(window) || self.parked.iter().any(|p| &p.window == window)
    }

    /// Nothing in it, parked windows included: it may go away.
    pub fn is_empty(&self) -> bool {
        self.layout.windows().next().is_none() && self.floating.is_empty() && self.fullscreen.is_none() && self.parked.is_empty()
    }
}

pub struct Screen {
    pub output: Output,
    pub grid: Grid,
    pub workspaces: Vec<Workspace>,
    /// index of the workspace shown
    pub current: usize,
    /// logical pixels per millimetre (for the grid when the output changes size or scale)
    pub mm: f64,
    /// physical pixels per millimetre: `mm` again when the scale changes
    pub phys_mm: f64,
    pub margin_mm: f64,
    /// whether `phys_mm` is the display's own (its reported size, or config.toml's): else a guess, and
    /// nothing on the sheet is to scale (the scale bar letters NTS)
    pub measured: bool,
    /// the margins' ticks the shell's merged cells leave out (sheet.rs), top, bottom, left, right
    pub hidden_ticks: [u32; 4],
}

impl Draftpaper {
    /// Add a screen for `output`, already mapped in the space; `mm` in physical pixels per millimetre,
    /// `measured` when it comes from the display's size (or config.toml), not the 160 PPI guess.
    pub fn add_screen(&mut self, output: Output, mm: f64, margin_mm: f64, measured: bool) {
        let scale = output.current_scale().fractional_scale();
        let geometry = self.space.output_geometry(&output).unwrap();
        let phys_mm = mm;
        let mm = mm / scale;
        let grid = Grid::new(geometry.loc, geometry.size, mm, margin_mm, scale, crate::render::mode_size(&output));
        // the lowest number not yet in use
        let number = (1..).find(|n| !self.screens.iter().any(|s| s.workspaces.iter().any(|w| w.number == *n))).unwrap();
        self.screens.push(Screen { output, grid, workspaces: vec![Workspace::new(number)], current: 0, mm, phys_mm, margin_mm, measured, hidden_ticks: [0; 4] });
        self.screens.sort_by_key(|s| s.grid.area.loc.x);
    }

    /// Remove the screen of `output`; its windows join the first remaining screen.
    pub fn remove_screen(&mut self, output: &Output) {
        let Some(i) = self.screens.iter().position(|s| &s.output == output) else { return };
        let gone = self.screens.remove(i);
        if let Some(to) = self.screens.first_mut() {
            // its workspaces move over whole, keeping their numbers
            to.workspaces.extend(gone.workspaces);
            to.workspaces.sort_by_key(|w| w.number);
        }
        self.active = self.active.min(self.screens.len().saturating_sub(1));
        self.relayout();
    }

    /// Recompute a screen's grid after its output changed size, scale or position.
    pub fn update_screen(&mut self, output: &Output) {
        let Some(geometry) = self.space.output_geometry(output) else { return };
        if let Some(s) = self.screens.iter_mut().find(|s| &s.output == output) {
            s.mm = s.phys_mm / output.current_scale().fractional_scale();
            s.grid = Grid::new(geometry.loc, geometry.size, s.mm, s.margin_mm, output.current_scale().fractional_scale(), crate::render::mode_size(output));
        }
        self.relayout();
    }

    /// The screen whose shown workspace holds `window`.
    pub fn screen_of(&self, window: &Window) -> Option<usize> {
        self.screens.iter().position(|s| s.layout().contains(window))
    }

    /// The screen and workspace (index) holding `window`, shown or not.
    pub fn workspace_of(&self, window: &Window) -> Option<(usize, usize)> {
        self.screens.iter().enumerate().find_map(|(i, s)| s.workspaces.iter().position(|w| w.holds(window)).map(|k| (i, k)))
    }

    /// Show workspace `number`: where it is, or new on the active screen.
    pub fn switch_to(&mut self, number: u32) {
        let at = self.screens.iter().enumerate().find_map(|(i, s)| s.workspaces.iter().position(|w| w.number == number).map(|k| (i, k)));
        let (i, k) = match at {
            Some(at) => at,
            None => {
                let Some(s) = self.screens.get_mut(self.active) else { return };
                s.workspaces.push(Workspace::new(number));
                s.workspaces.sort_by_key(|w| w.number);
                (self.active, s.workspaces.iter().position(|w| w.number == number).unwrap())
            }
        };
        // the windows leaving the screen, where they are, to slide them out
        let outgoing: Vec<(Window, Point<i32, Logical>)> = if self.screens[i].current != k {
            let ws = self.screens[i].workspace();
            ws.layout.windows().chain(ws.floating.iter().filter(|w| !crate::rules::is_sticky(w))).filter_map(|w| Some((w.clone(), self.space.element_location(w)?))).collect()
        } else {
            Vec::new()
        };
        let from = self.screens[i].workspace().number;
        let s = &mut self.screens[i];
        let old = s.current;
        s.current = k;
        // sticky windows (picture-in-picture) come along to the workspace shown
        if old != k {
            let sticky: Vec<Window> = s.workspaces[old].floating.iter().filter(|w| crate::rules::is_sticky(w)).cloned().collect();
            s.workspaces[old].floating.retain(|w| !crate::rules::is_sticky(w));
            s.workspaces[k].floating.extend(sticky);
        }
        // an empty workspace that is left behind goes away
        if old != k && s.workspaces[old].is_empty() {
            s.workspaces.remove(old);
            if old < k {
                s.current -= 1;
            }
        }
        self.active = i;
        self.relayout();
        if old != k {
            self.animate_slide(i, outgoing, if number > from { 1 } else { -1 });
        }
        let w = self.screens[i].layout().focused();
        self.focus(w);
    }

    /// Send the focused window of the active screen to workspace `number` (created where it does not exist).
    pub fn send_to(&mut self, number: u32) {
        let Some(s) = self.screens.get(self.active) else { return };
        let Some(window) = s.layout().focused() else { return };
        if s.workspaces[s.current].number == number {
            return;
        }
        let (i, grid) = (self.active, s.grid);
        self.screens[i].layout_mut().remove(&window, &grid);
        let at = self.screens.iter().enumerate().find_map(|(j, s)| s.workspaces.iter().position(|w| w.number == number).map(|k| (j, k)));
        let (j, k) = at.unwrap_or_else(|| {
            let s = &mut self.screens[i];
            s.workspaces.push(Workspace::new(number));
            s.workspaces.sort_by_key(|w| w.number);
            (i, s.workspaces.iter().position(|w| w.number == number).unwrap())
        });
        let g = self.screens[j].grid;
        let ws = &mut self.screens[j].workspaces[k];
        // with no zone left for it there (Layout::add), it floats
        if !ws.layout.add(window.clone(), &g) {
            ws.floating.push(window);
        }
        self.relayout();
        let next = self.screens[i].layout().focused();
        self.focus(next);
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

impl Screen {
    pub fn workspace(&self) -> &Workspace {
        &self.workspaces[self.current]
    }

    pub fn workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.current]
    }

    /// The layout of the workspace shown.
    pub fn layout(&self) -> &Layout {
        &self.workspaces[self.current].layout
    }

    pub fn layout_mut(&mut self) -> &mut Layout {
        &mut self.workspaces[self.current].layout
    }
}
