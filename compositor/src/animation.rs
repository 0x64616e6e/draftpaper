//! Animations (#27): parking shrinks a window into its cell, unparking grows it back into its place,
//! switching workspaces slides the outgoing windows out and the incoming ones in, across the frame. The
//! layout changes at once; only the drawing catches up (render.rs draws each window's current buffer
//! into the moving rectangle), so a client is never asked for intermediate sizes. `[animations]` in
//! config.toml sets the duration or turns them off (reduced motion).

use std::time::{Duration, Instant};

use smithay::{
    desktop::Window,
    utils::{IsAlive, Logical, Point, Rectangle},
};

use crate::Draftpaper;

/// A window flying between its place and its cell.
pub struct Flight {
    pub window: Window,
    pub output: String,
    pub from: Rectangle<i32, Logical>,
    /// parking: to its cell (the window has left the space); unparking: None, to wherever the window is
    pub to: Option<Rectangle<i32, Logical>>,
    start: Instant,
    duration: Duration,
}

/// A workspace switch on one output: the windows that left, where they were, and the direction
/// (+1: the new workspace comes in from the right).
pub struct Slide {
    pub output: String,
    pub outgoing: Vec<(Window, Point<i32, Logical>)>,
    pub dir: i32,
    start: Instant,
    duration: Duration,
}

#[derive(Default)]
pub struct Animations {
    pub flights: Vec<Flight>,
    pub slides: Vec<Slide>,
}

/// Cubic ease in and out of `t` in 0..1.
fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

fn progress(start: Instant, duration: Duration, now: Instant) -> f64 {
    ease(now.saturating_duration_since(start).as_secs_f64() / duration.as_secs_f64().max(1e-3))
}

/// The rectangle `e` of the way from `a` to `b`.
pub fn lerp(a: Rectangle<i32, Logical>, b: Rectangle<i32, Logical>, e: f64) -> Rectangle<f64, Logical> {
    let l = |x: i32, y: i32| x as f64 + (y - x) as f64 * e;
    Rectangle::new((l(a.loc.x, b.loc.x), l(a.loc.y, b.loc.y)).into(), (l(a.size.w, b.size.w), l(a.size.h, b.size.h)).into())
}

impl Flight {
    pub fn progress(&self, now: Instant) -> f64 {
        progress(self.start, self.duration, now)
    }
}

impl Slide {
    pub fn progress(&self, now: Instant) -> f64 {
        progress(self.start, self.duration, now)
    }
}

impl Animations {
    pub fn flight(&self, window: &Window) -> Option<&Flight> {
        self.flights.iter().find(|f| &f.window == window)
    }

    pub fn slide(&self, output: &str) -> Option<&Slide> {
        self.slides.iter().find(|s| s.output == output)
    }

    /// Drop what has finished (or whose window has gone).
    pub fn prune(&mut self, now: Instant) {
        self.flights.retain(|f| f.window.alive() && now.saturating_duration_since(f.start) < f.duration);
        self.slides.retain(|s| now.saturating_duration_since(s.start) < s.duration);
    }
}

impl Draftpaper {
    /// Where cell `cell` of screen `i` is (global, logical): a zone of the left margin, then the right.
    pub fn cell_rect(&self, i: usize, cell: usize) -> Option<Rectangle<i32, Logical>> {
        let s = self.screens.get(i)?;
        let og = self.space.output_geometry(&s.output)?;
        let margin = (s.margin_mm * s.mm).round() as i32;
        let down = s.grid.zones.1.max(1) as usize;
        let x = if cell < down { og.loc.x } else { og.loc.x + og.size.w - margin };
        let k = (cell % down) as f64;
        let y = (s.grid.origin.y + k * s.grid.zone.1).round() as i32;
        Some(Rectangle::new((x, y).into(), (margin, s.grid.zone.1.round() as i32).into()))
    }

    /// A window that was at `from` (global, logical) flies into cell `cell` of screen `i`.
    pub fn animate_park(&mut self, window: &Window, from: Rectangle<i32, Logical>, i: usize, cell: usize) {
        let (Some(duration), Some(to)) = (self.config.animations, self.cell_rect(i, cell)) else { return };
        let output = self.screens[i].output.name();
        self.anim.flights.retain(|f| &f.window != window);
        self.anim.flights.push(Flight { window: window.clone(), output, from, to: Some(to), start: Instant::now(), duration });
    }

    /// A window back from cell `cell` of screen `i` grows from it into its place.
    pub fn animate_unpark(&mut self, window: &Window, i: usize, cell: usize) {
        let (Some(duration), Some(from)) = (self.config.animations, self.cell_rect(i, cell)) else { return };
        let output = self.screens[i].output.name();
        self.anim.flights.retain(|f| &f.window != window);
        self.anim.flights.push(Flight { window: window.clone(), output, from, to: None, start: Instant::now(), duration });
    }

    /// Screen `i` switched workspaces: `outgoing` slide out, the new ones in, towards `dir`.
    pub fn animate_slide(&mut self, i: usize, outgoing: Vec<(Window, Point<i32, Logical>)>, dir: i32) {
        let Some(duration) = self.config.animations else { return };
        let output = self.screens[i].output.name();
        self.anim.slides.retain(|s| s.output != output);
        self.anim.slides.push(Slide { output, outgoing, dir, start: Instant::now(), duration });
    }
}
