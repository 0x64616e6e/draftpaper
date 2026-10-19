//! The surface a margin or the title block is drawn into: a rectangle of physical pixels, in the shape
//! Wayland takes them — ARGB with the colour already multiplied by its alpha, in the machine's own byte
//! order. Everything here is in physical pixels; the millimetres are worked out before (sheet.rs).

/// A colour as the sheet keeps them: red, green and blue, and how much of it there is.
#[derive(Clone, Copy, Debug)]
pub struct Ink {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl Ink {
    /// "#rrggbb" as the theme writes it; black where it is not that.
    pub fn parse(text: &str, alpha: f32) -> Ink {
        let hex = text.trim().trim_start_matches('#');
        let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
        if hex.len() >= 6 {
            Ink { r: byte(0), g: byte(2), b: byte(4), a: alpha }
        } else {
            Ink { r: 0, g: 0, b: 0, a: alpha }
        }
    }

    /// With the colour multiplied by the alpha, which is how a Wayland buffer holds it.
    pub fn premultiplied(self) -> u32 {
        let a = self.a.clamp(0.0, 1.0);
        let c = |v: u8| (v as f32 * a).round() as u32;
        ((a * 255.0).round() as u32) << 24 | c(self.r) << 16 | c(self.g) << 8 | c(self.b)
    }
}

pub struct Canvas<'a> {
    pixels: &'a mut [u8],
    pub width: i32,
    pub height: i32,
}

impl<'a> Canvas<'a> {
    pub fn new(pixels: &'a mut [u8], width: i32, height: i32) -> Canvas<'a> {
        pixels.fill(0);
        Canvas { pixels, width, height }
    }

    fn at(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width && y < self.height).then(|| (y as usize * self.width as usize + x as usize) * 4)
    }

    /// One pixel, `cover` of the way to `colour` (0 none, 255 all), over what is there.
    pub fn blend(&mut self, x: i32, y: i32, colour: u32, cover: u8) {
        let Some(at) = self.at(x, y) else { return };
        let (sa, sr, sg, sb) = (colour >> 24 & 0xff, colour >> 16 & 0xff, colour >> 8 & 0xff, colour & 0xff);
        let k = cover as u32;
        let over = |src: u32, dst: u8| -> u8 {
            // src is already multiplied by its own alpha; `cover` is how much of it lands here
            let s = src * k / 255;
            (s + (dst as u32) * (255 - sa * k / 255) / 255).min(255) as u8
        };
        let p = &mut self.pixels[at..at + 4];
        // the bytes are little-endian: blue, green, red, alpha
        p[0] = over(sb, p[0]);
        p[1] = over(sg, p[1]);
        p[2] = over(sr, p[2]);
        p[3] = over(sa, p[3]);
    }

    /// A line from one point to another, `wide` physical pixels across, drawn by stepping along it: the
    /// sheet's lines are short and few, so nothing cleverer is called for.
    pub fn line(&mut self, from: (f32, f32), to: (f32, f32), wide: f32, colour: u32) {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let steps = dx.abs().max(dy.abs()).ceil().max(1.0);
        let half = (wide / 2.0).max(0.5);
        for i in 0..=steps as i32 {
            let t = i as f32 / steps;
            let (x, y) = (from.0 + dx * t, from.1 + dy * t);
            // across the line, so a thick one is thick
            let (ax, ay) = if dx.abs() > dy.abs() { (0.0, 1.0) } else { (1.0, 0.0) };
            let mut k = -half;
            while k <= half {
                canvas_dot(self, x + ax * k, y + ay * k, colour);
                k += 1.0;
            }
        }
    }

    /// A rectangle, filled.
    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, colour: u32) {
        for row in y..y + h {
            for col in x..x + w {
                self.blend(col, row, colour, 255);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_is_read_and_multiplied() {
        let ink = Ink::parse("#A39C89", 1.0);
        assert_eq!((ink.r, ink.g, ink.b), (0xa3, 0x9c, 0x89));
        assert_eq!(ink.premultiplied() >> 24, 255);
        // half of it is half the colour as well as half the alpha: what a Wayland buffer wants
        let half = Ink { a: 0.5, ..ink }.premultiplied();
        assert_eq!(half >> 24, 128);
        assert_eq!(half >> 16 & 0xff, (0xa3 as f32 * 0.5).round() as u32);
        // nonsense is black rather than a panic
        assert_eq!(Ink::parse("not a colour", 1.0).premultiplied(), 0xff000000);
    }

    #[test]
    fn drawing_lands_where_it_is_told_and_nowhere_else() {
        let mut pixels = vec![0u8; 4 * 4 * 4];
        let mut canvas = Canvas::new(&mut pixels, 4, 4);
        let ink = Ink::parse("#ffffff", 1.0).premultiplied();
        canvas.fill(1, 1, 2, 1, ink);
        let px = |x: usize, y: usize| pixels[(y * 4 + x) * 4 + 3];
        assert_eq!((px(1, 1), px(2, 1)), (255, 255));
        assert_eq!((px(0, 1), px(3, 1), px(1, 0), px(1, 2)), (0, 0, 0, 0));
        // off the edge is dropped, not wrapped round or a panic
        let mut canvas = Canvas::new(&mut pixels, 4, 4);
        canvas.fill(-2, -2, 10, 10, ink);
        assert_eq!(pixels.iter().skip(3).step_by(4).filter(|a| **a == 255).count(), 16);
    }
}

/// One pixel of a line, with the coverage its sub-pixel place deserves, so a line between millimetres
/// does not look ragged.
fn canvas_dot(canvas: &mut Canvas, x: f32, y: f32, colour: u32) {
    let (fx, fy) = (x.floor(), y.floor());
    let (ax, ay) = (x - fx, y - fy);
    for (dx, dy, cover) in [(0.0, 0.0, (1.0 - ax) * (1.0 - ay)), (1.0, 0.0, ax * (1.0 - ay)), (0.0, 1.0, (1.0 - ax) * ay), (1.0, 1.0, ax * ay)] {
        if cover > 0.004 {
            canvas.blend((fx + dx) as i32, (fy + dy) as i32, colour, (cover * 255.0) as u8);
        }
    }
}
