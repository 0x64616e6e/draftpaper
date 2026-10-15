//! The magnifier (Mod+z, the `zoom` action): a square lens on the pointer showing what is under it,
//! magnified, each pixel a block, with a faint grid between them once they are large enough to tell
//! apart and the pixel under the pointer's hotspot outlined. Scrolling zooms (2 to 64 times), Shift with
//! it changes the lens's size (20 to 200 mm); Escape or Mod+z puts it away. Clicks and keys go on to
//! the windows as ever; the pointer itself is hidden, the outlined pixel being where it points.
//!
//! The lens's output is drawn offscreen first, without the pointer or the lens (kept between frames, so
//! only what changed is drawn again), and the lens samples that picture.

use std::cell::RefCell;

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            damage::OutputDamageTracker,
            element::{Element, Id, Kind, RenderElement},
            gles::{GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformName, UniformType},
            utils::{CommitCounter, DamageSet, OpaqueRegions},
            Bind, Offscreen,
        },
    },
    output::Output,
    utils::{Buffer, Physical, Point, Rectangle, Scale, Transform},
};

/// The zoom levels a notch of the wheel steps through.
const LEVELS: [f64; 11] = [2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0];

#[derive(Debug, Clone)]
pub struct Lens {
    /// index into LEVELS
    level: usize,
    /// the lens's side, in millimetres
    pub size_mm: f64,
    /// scrolling not yet a whole notch (a touchpad's)
    acc: f64,
}

impl Default for Lens {
    fn default() -> Lens {
        Lens { level: 4, size_mm: 50.0, acc: 0.0 }
    }
}

impl Lens {
    pub fn level(&self) -> f64 {
        LEVELS[self.level]
    }

    /// Scrolling by `notches` (up negative): zoom, or with `resize` the lens's size. True when the lens
    /// changed.
    pub fn scroll(&mut self, notches: f64, resize: bool) -> bool {
        self.acc += notches;
        let steps = self.acc.trunc();
        if steps == 0.0 {
            return false;
        }
        self.acc -= steps;
        if resize {
            self.size_mm = (self.size_mm * 1.15f64.powf(-steps)).clamp(20.0, 200.0);
        } else {
            self.level = (self.level as i64 - steps as i64).clamp(0, LEVELS.len() as i64 - 1) as usize;
        }
        true
    }
}

const SHADER: &str = r#"#version 100
//_DEFINES_
#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif
precision highp float;
#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif
uniform float alpha;
varying vec2 v_coords;
// the picture's size in pixels, the zoom, the pixel under the hotspot
uniform vec2 texsize;
uniform float level;
uniform vec2 centre;

float luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }

void main() {
    vec2 t = v_coords * texsize;
    vec2 cell = floor(t);
    // the pixel's own colour, whole: sampled at its centre
    vec4 color = texture2D(tex, (cell + 0.5) / texsize);
    color.a = 1.0;
    // where in its block, in screen pixels
    vec2 f = (t - cell) * level;
    // a grid between the pixels once they are large enough to tell apart
    if (level >= 6.0 && (f.x < 1.0 || f.y < 1.0)) color.rgb = mix(color.rgb, vec3(0.5), 0.3);
    // the pixel under the hotspot outlined, in black or white, whichever it is not
    float w = level >= 12.0 ? 2.0 : 1.0;
    if (all(equal(cell, centre)) && (f.x < w || f.y < w || f.x >= level - w || f.y >= level - w))
        color.rgb = luma(color.rgb) > 0.5 ? vec3(0.0) : vec3(1.0);
    gl_FragColor = color * alpha;
}
"#;

/// Where the lens goes on an output of `size` pixels, the pointer's hotspot at `pointer`, `level` times,
/// about `side` pixels wide: the region it shows (an odd number of pixels across, the hotspot's in the
/// middle, kept inside the output), where it shows it (the hotspot's block under the hotspot, or near an
/// edge as close as the output allows, its frame on it), and the hotspot's pixel.
pub fn place(size: (i32, i32), pointer: Point<f64, Physical>, level: f64, side: f64) -> (Rectangle<i32, Buffer>, Rectangle<i32, Physical>, (i32, i32)) {
    let n = (2.0 * (side.max(level * 3.0) / level / 2.0).floor() + 1.0) as i32;
    let c = (pointer.x.floor() as i32, pointer.y.floor() as i32);
    let half = (n - 1) / 2;
    let sx = (c.0 - half).clamp(0, (size.0 - n).max(0));
    let sy = (c.1 - half).clamp(0, (size.1 - n).max(0));
    let src = Rectangle::<i32, Buffer>::new((sx, sy).into(), (n.min(size.0), n.min(size.1)).into());
    let l = level as i32;
    let (dw, dh) = (src.size.w * l, src.size.h * l);
    let x = (c.0 - (c.0 - sx) * l - l / 2).clamp(2, (size.0 - dw - 2).max(2));
    let y = (c.1 - (c.1 - sy) * l - l / 2).clamp(2, (size.1 - dh - 2).max(2));
    (src, Rectangle::new((x, y).into(), (dw, dh).into()), c)
}

/// Kept with each output: the offscreen picture of it, its damage tracker, and the lens's program,
/// element id and commit.
#[derive(Default)]
pub struct LensTarget(RefCell<Option<Target>>);

struct Target {
    texture: GlesTexture,
    size: (i32, i32),
    damage: OutputDamageTracker,
    program: GlesTexProgram,
    id: Id,
    commit: CommitCounter,
    /// what the lens showed last: its place, size, level and the pixel under the hotspot
    last: (Rectangle<i32, Physical>, Rectangle<i32, Buffer>, u64),
}

/// The lens, drawn: a region of the offscreen picture onto `dst`, each pixel `level` screen pixels.
pub struct LensElement {
    id: Id,
    commit: CommitCounter,
    texture: GlesTexture,
    program: GlesTexProgram,
    src: Rectangle<i32, Buffer>,
    dst: Rectangle<i32, Physical>,
    uniforms: Vec<Uniform<'static>>,
}

impl Element for LensElement {
    fn id(&self) -> &Id {
        &self.id
    }
    fn current_commit(&self) -> CommitCounter {
        self.commit
    }
    fn src(&self) -> Rectangle<f64, Buffer> {
        self.src.to_f64()
    }
    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.dst
    }
    fn damage_since(&self, scale: Scale<f64>, commit: Option<CommitCounter>) -> DamageSet<i32, Physical> {
        if commit == Some(self.commit) {
            DamageSet::default()
        } else {
            DamageSet::from_slice(&[Rectangle::from_size(self.geometry(scale).size)])
        }
    }
    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        OpaqueRegions::from_slice(&[Rectangle::from_size(self.geometry(scale).size)])
    }
    fn kind(&self) -> Kind {
        Kind::Unspecified
    }
}

impl RenderElement<GlesRenderer> for LensElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        frame.render_texture_from_to(&self.texture, src, dst, damage, opaque_regions, Transform::Normal, 1.0, Some(&self.program), &self.uniforms)
    }
}

/// The lens over `output`: `scene` (its elements, front first, without the pointer) drawn offscreen,
/// and the lens on `pointer` (physical pixels of the output) over it, with its frame. None when it cannot
/// be drawn.
pub fn lens<E: RenderElement<GlesRenderer>>(
    renderer: &mut GlesRenderer,
    output: &Output,
    size: (i32, i32),
    scene: &[E],
    lens: &Lens,
    pointer: Point<f64, Physical>,
    mm: f64,
) -> Option<(LensElement, [Rectangle<i32, Physical>; 8])> {
    if size.0 <= 0 || size.1 <= 0 {
        return None;
    }
    output.user_data().insert_if_missing(LensTarget::default);
    let cell = output.user_data().get::<LensTarget>()?;
    let mut target = cell.0.borrow_mut();
    if target.as_ref().is_none_or(|t| t.size != size) {
        let texture: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, (size.0, size.1).into()).ok()?;
        let names = [
            UniformName::new("texsize", UniformType::_2f),
            UniformName::new("level", UniformType::_1f),
            UniformName::new("centre", UniformType::_2f),
        ];
        let program = match target.take() {
            Some(t) => t.program,
            None => renderer.compile_custom_texture_shader(SHADER, &names).map_err(|e| eprintln!("draftpaper: the lens's shader: {e:?}")).ok()?,
        };
        let damage = OutputDamageTracker::new(size, output.current_scale().fractional_scale(), Transform::Normal);
        *target = Some(Target { texture, size, damage, program, id: Id::new(), commit: CommitCounter::default(), last: Default::default() });
    }
    let t = target.as_mut()?;

    // the output, drawn again where it changed
    let changed = {
        let mut fb = renderer.bind(&mut t.texture).ok()?;
        let result = t.damage.render_output(renderer, &mut fb, 1, scene, [0.0, 0.0, 0.0, 1.0]).ok()?;
        result.damage.is_some_and(|d| !d.is_empty())
    };

    // the region and the lens (place())
    let level = lens.level();
    let side = (lens.size_mm * mm).round();
    let (src, dst, c) = place(size, pointer, level, side);
    let last = (dst, src, (level.to_bits() ^ ((c.0 as u64) << 20) ^ ((c.1 as u64) << 40)));
    if changed || last != t.last {
        t.commit.increment();
        t.last = last;
    }

    // the frame: dark outside, light inside, a pixel each
    let (x, y, w, h) = (dst.loc.x, dst.loc.y, dst.size.w, dst.size.h);
    let ring = |d: i32| {
        [
            Rectangle::new((x - d, y - d).into(), (w + 2 * d, 1).into()),
            Rectangle::new((x - d, y + h + d - 1).into(), (w + 2 * d, 1).into()),
            Rectangle::new((x - d, y - d + 1).into(), (1, h + 2 * d - 2).into()),
            Rectangle::new((x + w + d - 1, y - d + 1).into(), (1, h + 2 * d - 2).into()),
        ]
    };
    let (outer, inner) = (ring(2), ring(1));
    let frame = [outer[0], outer[1], outer[2], outer[3], inner[0], inner[1], inner[2], inner[3]];

    let uniforms = vec![
        Uniform::new("texsize", [size.0 as f32, size.1 as f32]),
        Uniform::new("level", level as f32),
        Uniform::new("centre", [c.0 as f32, c.1 as f32]),
    ];
    let element = LensElement { id: t.id.clone(), commit: t.commit, texture: t.texture.clone(), program: t.program.clone(), src, dst, uniforms };
    Some((element, frame))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wheel_zooms_and_sizes() {
        let mut lens = Lens::default();
        assert_eq!(lens.level(), 8.0);
        // a notch up zooms in, down out, and no further than the ends
        assert!(lens.scroll(-1.0, false));
        assert_eq!(lens.level(), 12.0);
        for _ in 0..20 {
            lens.scroll(-1.0, false);
        }
        assert_eq!(lens.level(), 64.0);
        for _ in 0..20 {
            lens.scroll(1.0, false);
        }
        assert_eq!(lens.level(), 2.0);
        // a touchpad's fractions add up to a notch
        assert!(!lens.scroll(-0.5, false));
        assert!(lens.scroll(-0.5, false));
        assert_eq!(lens.level(), 3.0);
        // with Shift, the size: larger up, within 20 to 200 mm
        let before = lens.size_mm;
        lens.scroll(-1.0, true);
        assert!(lens.size_mm > before);
        for _ in 0..50 {
            lens.scroll(1.0, true);
        }
        assert_eq!(lens.size_mm, 20.0);
    }
}
