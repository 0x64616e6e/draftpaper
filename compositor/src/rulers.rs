//! The rulers (issue #48): while the shell has them out (qml/Rulers.qml), the pointer moves in whole
//! steps on their grid, a millimetre (ten with Shift) along each ruler from their zero point, at whatever
//! angles they stand, so what is drawn under it lands on the millimetre. The shell sends where the zero
//! is and how the rulers stand (the IPC's "rulers" request); the pointer's own position is kept unsnapped
//! underneath (input.rs), so small movements add up to whole steps instead of being lost to rounding.

use smithay::utils::{Logical, Point};

/// The rulers as the shell has them out.
#[derive(Debug, Clone, PartialEq)]
pub struct Rulers {
    /// the output they are on
    pub output: String,
    /// the zero point, in the layout's logical pixels
    pub zero: Point<f64, Logical>,
    /// each ruler's direction, degrees counter-clockwise from the horizontal: the horizontal one's, then
    /// the vertical one's (90 when level)
    pub angles: (f64, f64),
    /// logical pixels per millimetre on that output
    pub mm: f64,
    /// whether the pointer is held to the steps
    pub snap: bool,
    /// the steps in millimetres: without Shift, and with it (config.toml's [rulers] step, step_shift)
    pub steps: (f64, f64),
}

/// `raw` held to the rulers' grid: its offset from the zero point, along each ruler, a whole number of
/// steps (`steps`: 1 mm, or 10 with `coarse`, unless configured). Unchanged when the rulers stand (nearly) along one line.
pub fn snap(raw: Point<f64, Logical>, r: &Rulers, coarse: bool) -> Point<f64, Logical> {
    let dir = |deg: f64| (deg.to_radians().cos(), -deg.to_radians().sin());
    let (u, w) = (dir(r.angles.0), dir(r.angles.1));
    let det = u.0 * w.1 - u.1 * w.0;
    if det.abs() < 0.05 || r.mm <= 0.0 {
        return raw;
    }
    let (dx, dy) = (raw.x - r.zero.x, raw.y - r.zero.y);
    let a = (dx * w.1 - dy * w.0) / det;
    let b = (u.0 * dy - u.1 * dx) / det;
    let step = r.mm * if coarse { r.steps.1 } else { r.steps.0 };
    let (a, b) = ((a / step).round() * step, (b / step).round() * step);
    Point::from((r.zero.x + a * u.0 + b * w.0, r.zero.y + a * u.1 + b * w.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rulers(h: f64, v: f64) -> Rulers {
        Rulers { output: "x".into(), zero: Point::from((400.0, 300.0)), angles: (h, v), mm: 6.4 / 1.5, snap: true, steps: (1.0, 10.0) }
    }

    /// along each ruler, a whole number of steps from the zero
    fn steps(p: Point<f64, Logical>, r: &Rulers, step: f64) -> (f64, f64) {
        let dir = |deg: f64| (deg.to_radians().cos(), -deg.to_radians().sin());
        let (u, w) = (dir(r.angles.0), dir(r.angles.1));
        let det = u.0 * w.1 - u.1 * w.0;
        let (dx, dy) = (p.x - r.zero.x, p.y - r.zero.y);
        (((dx * w.1 - dy * w.0) / det) / (r.mm * step), ((u.0 * dy - u.1 * dx) / det) / (r.mm * step))
    }

    #[test]
    fn the_pointer_keeps_to_the_millimetre() {
        for (h, v) in [(0.0, 90.0), (30.0, 120.0), (15.0, 90.0), (-20.0, 55.0), (45.0, 135.0)] {
            let r = rulers(h, v);
            for (x, y) in [(400.3, 300.2), (433.7, 287.1), (12.0, 790.5), (1270.9, 3.3), (401.0, 299.0)] {
                for (coarse, step) in [(false, 1.0), (true, 10.0)] {
                    let p = snap(Point::from((x, y)), &r, coarse);
                    let (a, b) = steps(p, &r, step);
                    assert!((a - a.round()).abs() < 1e-9 && (b - b.round()).abs() < 1e-9, "{h}/{v}: ({x}, {y}) to {p:?}: {a}, {b}");
                    // the nearest step: never more than half a step away along either ruler
                    let (a0, b0) = steps(Point::from((x, y)), &r, step);
                    assert!((a - a0).abs() <= 0.5 + 1e-9 && (b - b0).abs() <= 0.5 + 1e-9, "{h}/{v}: ({x}, {y}): not the nearest");
                }
            }
        }
        // level: the grid of whole millimetres from the zero, in logical pixels
        let r = rulers(0.0, 90.0);
        let p = snap(Point::from((400.0 + 3.4 * r.mm, 300.0 - 7.6 * r.mm)), &r, false);
        assert!((p.x - (400.0 + 3.0 * r.mm)).abs() < 1e-9 && (p.y - (300.0 - 8.0 * r.mm)).abs() < 1e-9, "{p:?}");
        // along one line: no grid, the pointer as it is
        let r = rulers(10.0, 10.0);
        assert_eq!(snap(Point::from((123.4, 56.7)), &r, false), Point::from((123.4, 56.7)));
    }
}
