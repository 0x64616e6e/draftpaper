//! The sheet, drawn by the GPU under every window: the same picture as the i3 edition's render.py
//! (paper gradient and blooms, 5 mm dots, the frame, zone ticks across the margins, crop marks, dither),
//! computed per pixel in a fragment shader. Sizes are in the output's pixels per millimetre.

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

float bloom(vec2 uv, vec2 c, float r, float k) {
    vec2 d = vec2((uv.x - c.x) * size.x / size.y, uv.y - c.y);
    float rr = r * strength.y / 2.0;
    return min(1.0, k * strength.x) * exp(-dot(d, d) / (2.0 * rr * rr));
}

// 1.0 within half a pixel of a line at `at`
float near(float p, float at) { return step(abs(p - at), 0.5); }

void main() {
    vec2 uv = v_coords;
    vec2 px = floor(v_coords * size);          // pixel, origin top left
    float w = size.x, h = size.y, mm = strength.w, mg = grid.x;

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
    // two pixels wide, a tick at every zone boundary across the margins, crop marks in the corners
    float m = 0.0;
    float stp = floor(5.0 * mm);
    if (mod(px.x, stp) < 1.0 && mod(h - 1.0 - px.y, stp) < 1.0) m = max(m, 0.6);
    bool inX = px.x >= mg && px.x < w - mg, inY = px.y >= mg && px.y < h - mg;
    if (inY && ((px.x >= mg && px.x < mg + 2.0) || (px.x >= w - mg - 2.0 && px.x < w - mg))) m = max(m, 0.85);
    if (inX && ((px.y >= mg && px.y < mg + 2.0) || (px.y >= h - mg - 2.0 && px.y < h - mg))) m = max(m, 0.85);
    float zw = (w - 2.0 * mg) / grid.y, zh = (h - 2.0 * mg) / grid.z;
    if ((px.y < mg || px.y >= h - mg) && px.x >= mg - 0.5 && px.x <= w - mg + 0.5)
        m = max(m, 0.6 * near(px.x, mg + floor((px.x - mg) / zw + 0.5) * zw));
    if ((px.x < mg || px.x >= w - mg) && px.y >= mg - 0.5 && px.y <= h - mg + 0.5)
        m = max(m, 0.6 * near(px.y, mg + floor((px.y - mg) / zh + 0.5) * zh));
    float arm = min(3.0 * mm, mg * 0.45);
    vec2 cc = vec2(px.x < w / 2.0 ? mg / 2.0 : w - mg / 2.0, px.y < h / 2.0 ? mg / 2.0 : h - mg / 2.0);
    vec2 pc = px + 0.5;                         // pixel centre: the marks sit on half-pixel positions
    if ((abs(pc.y - cc.y) <= 0.5 && abs(pc.x - cc.x) <= arm) || (abs(pc.x - cc.x) <= 0.5 && abs(pc.y - cc.y) <= arm))
        m = max(m, 0.7);
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
    size: Size<i32, Logical>,
    scale: f64,
    pub theme: Theme,
    /// physical pixels per millimetre of the output (from its physical size, or the 160 PPI fallback)
    pub mm: f32,
}

impl SheetRenderer {
    pub fn new(theme: Theme, mm: f32) -> Self {
        SheetRenderer { program: None, element: None, size: (0, 0).into(), scale: 1.0, theme, mm }
    }

    /// The sheet for an output of `size`, built once and rebuilt when the size changes.
    /// `size` is the output's logical size and `scale` its scale: the shader works in the physical pixels
    /// it draws, so the frame and the zones land on the same pixels as the layout's (logical) grid.
    pub fn element(&mut self, renderer: &mut GlesRenderer, size: Size<i32, Logical>, scale: f64) -> PixelShaderElement {
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
            ];
            self.program = Some(renderer.compile_custom_pixel_shader(SHADER, &names).expect("the sheet shader compiles"));
        }
        if self.element.is_none() || self.size != size || self.scale != scale {
            let t = &self.theme;
            let s = &t.sheet;
            // the layout rounds the margin in logical pixels; the shader takes the same edge, scaled
            let logical_mm = self.mm / scale as f32;
            let margin = (t.margin_mm * logical_mm).round() * scale as f32;
            let zones = |len: f32| ((len - 2.0 * margin) / (50.0 * self.mm)).round().max(2.0);
            let physical = (size.w as f32 * scale as f32, size.h as f32 * scale as f32);
            let uniforms = vec![
                Uniform::new("paper", t.paper),
                Uniform::new("paper_light", t.paper_light),
                Uniform::new("paper_shade", t.paper_shade),
                Uniform::new("glow", t.glow),
                Uniform::new("ink", t.ink),
                Uniform::new("shape", [s.angle.to_radians(), s.midpoint, s.blend, s.shading]),
                Uniform::new("strength", [s.glow_strength, s.glow_size, s.pattern_strength, self.mm]),
                Uniform::new("grid", [margin, zones(physical.0), zones(physical.1)]),
            ];
            self.element = Some(PixelShaderElement::new(
                self.program.clone().unwrap(),
                Rectangle::from_size(size),
                Some(vec![Rectangle::from_size(size)]),
                1.0,
                uniforms,
                Kind::Unspecified,
            ));
            self.size = size;
            self.scale = scale;
        }
        self.element.clone().unwrap()
    }
}
