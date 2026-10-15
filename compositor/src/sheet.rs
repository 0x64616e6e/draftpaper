//! The sheet, drawn by the GPU under every window: the same picture as the i3 edition's render.py
//! (paper gradient and blooms, 5 mm dots, the frame, zone ticks across the margins, dither; the i3 edition also draws crop marks in the corners),
//! computed per pixel in a fragment shader. Sizes are in the output's pixels per millimetre.
//!
//! The margins' zones are cells, and the shell may merge neighbouring ones into one (a notification
//! that takes two zones): the ticks inside a merged cell are not drawn. `hidden` holds, per side, a mask
//! of the boundaries left out (bit k: the boundary k zones from the frame's corner).

use smithay::{
    backend::renderer::{
        element::Kind,
        gles::{element::PixelShaderElement, GlesPixelProgram, GlesRenderer, Uniform, UniformName, UniformType},
    },
    utils::{Logical, Rectangle, Size},
};

use crate::theme::Theme;

const SHADER: &str = r#"
precision highp float;
varying vec2 v_coords;
uniform vec2 size;
uniform float alpha;

uniform vec3 paper;
uniform vec3 paper_light;
uniform vec3 paper_shade;
uniform vec3 glow;
uniform vec3 ink;
// angle (radians), midpoint, blend, shading
uniform vec4 shape;
// glow strength, glow size, pattern strength, pixels per millimetre
uniform vec4 strength;
// margin (px), zones across, zones down
uniform vec3 grid;
// the boundaries not ticked, per side (top, bottom, left, right): bit k is boundary k
uniform vec4 hidden;
// a wallpaper (wallpaper.rs): 0 none; 1 a picture in the frame, paper margins; 2 ambient margins (only the
// lines are drawn here, in margin_ink, over the picture's colours)
uniform float wall;
uniform vec3 margin_ink;
// the element's extent in physical pixels (Smithay's `size` is its logical size), then the output's own
// (its mode): at a scale that does not divide the mode, the logical size is rounded up, and the element
// runs past the output's edge
uniform vec4 physical;

float bloom(vec2 uv, vec2 c, float r, float k) {
    vec2 d = vec2((uv.x - c.x) * size.x / size.y, uv.y - c.y);
    float rr = r * strength.y / 2.0;
    return min(1.0, k * strength.x) * exp(-dot(d, d) / (2.0 * rr * rr));
}

// 1.0 on the tick of boundary k (of n) along a side len pixels long, as title.rs's tick() places it: at
// the frame's corners the frame's line (2 pixels), between them one pixel at mg + round(k * inside / n),
// in whole numbers (a float division may round the wrong way; the products are exact)
float ticked(float p, float len, float mg, float n, float k) {
    if (k < 0.5) return step(mg, p) * step(p, mg + 1.0);
    if (k > n - 0.5) return step(len - mg - 2.0, p) * step(p, len - mg - 1.0);
    float a = 2.0 * k * (len - 2.0 * mg) + n, b = 2.0 * n;
    float at = floor(a / b);
    if (at * b > a) at -= 1.0;
    if ((at + 1.0) * b <= a) at += 1.0;
    return step(abs(p - (mg + at)), 0.25);
}

// bit k of a mask held in a float (no bit operations in GLSL ES 2; exact up to 2^24)
float bit(float mask, float k) { return mod(floor(mask / exp2(k) + 0.25), 2.0); }

void main() {
    vec2 uv = v_coords;
    vec2 px = floor(v_coords * physical.xy);   // physical pixel, origin top left
    float w = physical.z, h = physical.w, mm = strength.w, mg = grid.x;

    // the paper: light -> paper -> shade along the angle, smoothstepped, then three blooms of glow
    float ca = cos(shape.x), sa = sin(shape.x);
    float t = (ca * uv.x + sa * uv.y) / (ca + sa);
    float p = clamp(0.5 + (t - shape.y) / shape.z, 0.0, 1.0);
    p = p * p * (3.0 - 2.0 * p);
    vec3 light = paper + (paper_light - paper) * shape.w;
    vec3 shade = paper + (paper_shade - paper) * shape.w;
    vec3 c = p < 0.5 ? mix(light, paper, p / 0.5) : mix(paper, shade, (p - 0.5) / 0.5);
    c += (glow - c) * bloom(uv, vec2(0.16, 0.20), 0.55, 0.50);
    c += (glow - c) * bloom(uv, vec2(0.80, 0.30), 0.40, 0.26);
    c += (glow - c) * bloom(uv, vec2(0.42, 0.88), 0.48, 0.30);

    // the pattern, as an alpha mask: dots every 5 mm (counted from the bottom-left corner), the frame
    // two pixels wide, a tick at every zone boundary across the margins
    float m = 0.0;
    float stp = floor(5.0 * mm);
    if (mod(px.x, stp) < 1.0 && mod(h - 1.0 - px.y, stp) < 1.0) m = max(m, 0.6);
    bool inX = px.x >= mg && px.x < w - mg, inY = px.y >= mg && px.y < h - mg;
    if (inY && ((px.x >= mg && px.x < mg + 2.0) || (px.x >= w - mg - 2.0 && px.x < w - mg))) m = max(m, 0.85);
    if (inX && ((px.y >= mg && px.y < mg + 2.0) || (px.y >= h - mg - 2.0 && px.y < h - mg))) m = max(m, 0.85);
    float zw = (w - 2.0 * mg) / grid.y, zh = (h - 2.0 * mg) / grid.z;
    if ((px.y < mg || px.y >= h - mg) && px.x >= mg && px.x < w - mg) {
        float k = clamp(floor((px.x - mg) / zw + 0.5), 0.0, grid.y);
        float off = bit(px.y < mg ? hidden.x : hidden.y, k);
        m = max(m, 0.6 * ticked(px.x, w, mg, grid.y, k) * (1.0 - off));
    }
    if ((px.x < mg || px.x >= w - mg) && px.y >= mg && px.y < h - mg) {
        float k = clamp(floor((px.y - mg) / zh + 0.5), 0.0, grid.z);
        float off = bit(px.x < mg ? hidden.z : hidden.w, k);
        m = max(m, 0.6 * ticked(px.y, h, mg, grid.z, k) * (1.0 - off));
    }
    if (wall > 0.5) {
        // the picture shows in the frame's interior (no dots over it)
        if (px.x >= mg + 2.0 && px.x < w - mg - 2.0 && px.y >= mg + 2.0 && px.y < h - mg - 2.0) {
            gl_FragColor = vec4(0.0);
            return;
        }
        if (wall > 1.5) {
            float a = clamp(m * strength.z * 1.6, 0.0, 1.0);
            gl_FragColor = vec4(margin_ink * a, a) * alpha;
            return;
        }
    }
    c += (ink - c) * m * strength.z;

    // half a level of noise so the 8-bit steps of the gradient do not band
    float n = fract(sin(dot(px, vec2(12.9898, 78.233))) * 43758.5453) - 0.5;
    c += n / 255.0;
    gl_FragColor = vec4(c, 1.0) * alpha;
}
"#;

pub struct SheetRenderer {
    program: Option<GlesPixelProgram>,
    element: Option<PixelShaderElement>,
    /// what the element was built for: the size, the scale, the mode, the margin and the zones
    key: (Size<i32, Logical>, u64, (i32, i32), i32, (u32, u32)),
    pub theme: Theme,
    /// physical pixels per millimetre of the output (from its physical size, or the 160 PPI fallback)
    pub mm: f32,
    /// the ticks not drawn, per side (top, bottom, left, right): see the module's comment
    hidden: [u32; 4],
    /// with a wallpaper: the mode (see the shader) and the margins' ink
    wall: (u32, [u32; 3]),
}

impl SheetRenderer {
    pub fn new(theme: Theme, mm: f32) -> Self {
        SheetRenderer { program: None, element: None, key: ((0, 0).into(), 0, (0, 0), 0, (0, 0)), theme, mm, hidden: [0; 4], wall: (0, [0; 3]) }
    }

    /// The sheet for an output of `size`, built once and rebuilt when the size changes.
    /// A new theme: the shader's uniforms are rebuilt on the next frame.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.element = None;
    }

    /// The ticks to leave out (Screen::hidden_ticks); the sheet is drawn again when they change.
    pub fn set_hidden(&mut self, hidden: [u32; 4]) {
        if self.hidden != hidden {
            self.hidden = hidden;
            self.element = None;
        }
    }

    /// The wallpaper's mode (0 none, 1 paper margins, 2 ambient margins) and the margins' ink.
    pub fn set_wall(&mut self, mode: u32, ink: [f32; 3]) {
        let wall = (mode, ink.map(f32::to_bits));
        if self.wall != wall {
            self.wall = wall;
            self.element = None;
        }
    }

    /// `size` is the output's logical size and `scale` its scale: the shader works in the physical pixels
    /// it draws, the margin (title.rs's margin()) and the zones the layout's, so the frame and the zones land
    /// on the pixels the rest of the sheet is measured from.
    pub fn element(&mut self, renderer: &mut GlesRenderer, size: Size<i32, Logical>, scale: f64, mode: (i32, i32), margin: i32, zones: (u32, u32)) -> PixelShaderElement {
        if self.program.is_none() {
            let names = [
                UniformName::new("paper", UniformType::_3f),
                UniformName::new("paper_light", UniformType::_3f),
                UniformName::new("paper_shade", UniformType::_3f),
                UniformName::new("glow", UniformType::_3f),
                UniformName::new("ink", UniformType::_3f),
                UniformName::new("shape", UniformType::_4f),
                UniformName::new("strength", UniformType::_4f),
                UniformName::new("grid", UniformType::_3f),
                UniformName::new("physical", UniformType::_4f),
                UniformName::new("hidden", UniformType::_4f),
                UniformName::new("wall", UniformType::_1f),
                UniformName::new("margin_ink", UniformType::_3f),
            ];
            self.program = Some(renderer.compile_custom_pixel_shader(SHADER, &names).expect("the sheet shader compiles"));
        }
        let key = (size, scale.to_bits(), mode, margin, zones);
        if self.element.is_none() || self.key != key {
            let t = &self.theme;
            let s = &t.sheet;
            let physical = ((size.w as f64 * scale).round() as f32, (size.h as f64 * scale).round() as f32);
            let uniforms = vec![
                Uniform::new("paper", t.paper),
                Uniform::new("paper_light", t.paper_light),
                Uniform::new("paper_shade", t.paper_shade),
                Uniform::new("glow", t.glow),
                Uniform::new("ink", t.ink),
                Uniform::new("shape", [s.angle.to_radians(), s.midpoint, s.blend, s.shading]),
                Uniform::new("strength", [s.glow_strength, s.glow_size, s.pattern_strength, self.mm]),
                Uniform::new("grid", [margin as f32, zones.0.max(1) as f32, zones.1.max(1) as f32]),
                Uniform::new("physical", [physical.0, physical.1, mode.0 as f32, mode.1 as f32]),
                Uniform::new("hidden", self.hidden.map(|m| m as f32)),
                Uniform::new("wall", self.wall.0 as f32),
                Uniform::new("margin_ink", self.wall.1.map(f32::from_bits)),
            ];
            self.element = Some(PixelShaderElement::new(
                self.program.clone().unwrap(),
                Rectangle::from_size(size),
                // opaque, unless a wallpaper shows through
                (self.wall.0 == 0).then(|| vec![Rectangle::from_size(size)]),
                1.0,
                uniforms,
                Kind::Unspecified,
            ));
            self.key = key;
        }
        self.element.clone().unwrap()
    }
}
