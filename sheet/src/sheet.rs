//! The sheet's geometry, as the compositor works it out: the client never computes any of it. One
//! request on the compositor's socket (`{"request": "outputs"}`, compositor/src/ipc.rs) gives every
//! display its frame, its margin, its zones and the pixels a millimetre comes to, all in that display's
//! logical pixels.
//!
//! Stage 1 subscribes to the socket instead, so a change arrives on its own; stage 0 asks again whenever
//! Wayland says the displays have changed, which is what moves the sheet.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use serde_json::Value;

/// One display's sheet, in that display's logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub name: String,
    /// the display, from the whole layout's corner
    pub width: f64,
    pub height: f64,
    pub scale: f64,
    /// logical pixels a millimetre
    pub mm: f64,
    /// the margin, from the display's edge to the frame's line
    pub margin: f64,
    /// the frame's interior, from the display's corner
    pub frame: (f64, f64, f64, f64),
}

impl Screen {
    /// `k` millimetres from the frame's corner, across or down: the display's logical pixels, put on the
    /// nearest physical pixel — the sheet's invariant (qml/sheet.js `at`, docs/wayland-design.md).
    pub fn at(&self, k: f64, across: bool) -> f64 {
        let from = if across { self.frame.0 } else { self.frame.1 };
        ((from + k * self.mm) * self.scale).round() / self.scale
    }
}

/// Every display's sheet, asked of the compositor; empty where it cannot be reached.
pub fn screens() -> Vec<Screen> {
    let Some(reply) = ask("{\"request\": \"outputs\"}") else { return Vec::new() };
    let Some(outputs) = reply.get("reply").and_then(Value::as_array) else { return Vec::new() };
    outputs
        .iter()
        .filter_map(|o| {
            let n = |k: &str| o.get(k)?.as_f64();
            let frame = o.get("frame")?;
            let f = |k: &str| frame.get(k)?.as_f64();
            Some(Screen {
                name: o.get("name")?.as_str()?.to_string(),
                width: n("width")?,
                height: n("height")?,
                scale: n("scale").filter(|s| *s > 0.0)?,
                mm: n("px_per_mm").filter(|m| *m > 0.0)?,
                margin: n("margin")?,
                // the frame is given from the layout's corner; the sheet is drawn from the display's
                frame: (f("x")? - n("x")?, f("y")? - n("y")?, f("width")?, f("height")?),
            })
        })
        .collect()
}

/// One request on the compositor's socket, one reply.
fn ask(request: &str) -> Option<Value> {
    let path = std::env::var_os("DRAFTPAPER_SOCKET")?;
    let mut socket = UnixStream::connect(path).ok()?;
    let timeout = Some(std::time::Duration::from_secs(2));
    socket.set_read_timeout(timeout).ok()?;
    socket.set_write_timeout(timeout).ok()?;
    socket.write_all(format!("{request}\n").as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(socket).read_line(&mut line).ok()?;
    serde_json::from_str(&line).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(scale: f64, mm: f64, frame_x: f64) -> Screen {
        Screen { name: "test".into(), width: 1920.0, height: 1200.0, scale, mm, margin: 38.0, frame: (frame_x, frame_x, 1000.0, 800.0) }
    }

    #[test]
    fn a_millimetre_lands_on_a_physical_pixel() {
        // whatever the scale, what `at` gives is a whole physical pixel: the invariant the sheet rests on
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let s = screen(scale, 6.4 / scale, 30.4);
            for k in 0..200 {
                let v = s.at(k as f64, true) * scale;
                assert!((v - v.round()).abs() < 1e-9, "scale {scale}, {k} mm landed at {v}");
            }
        }
    }

    #[test]
    fn millimetres_keep_their_distance() {
        // 50 mm from the corner is 50 mm away, to within half a pixel: it is the rounding of one edge,
        // never a drift that grows with the distance
        let s = screen(1.25, 5.12, 30.4);
        for k in 0..200 {
            let want = s.frame.0 + k as f64 * s.mm;
            assert!((s.at(k as f64, true) - want).abs() <= 0.5 / s.scale + 1e-9, "{k} mm drifted");
        }
    }
}
