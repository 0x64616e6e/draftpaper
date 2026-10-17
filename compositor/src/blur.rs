//! The sheet blurred: what is behind the login card (the greeter, handoff.rs), and the session's first
//! moments, the blur going to nothing as the desktop comes up (opening.rs). The scene is drawn offscreen
//! (kept between frames, so only what changed is drawn again), reduced to a quarter of its size by
//! averaging (two halvings, each pixel the mean of four), blurred there with a Gaussian across and then
//! down, and drawn back over the whole output.

use std::cell::RefCell;

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            damage::OutputDamageTracker,
            element::{Element, Id, Kind, RenderElement},
            gles::{GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformName, UniformType},
            utils::{CommitCounter, DamageSet, OpaqueRegions},
            Bind, Frame, Offscreen, Renderer,
        },
    },
    output::Output,
    utils::{Buffer, Physical, Rectangle, Scale, Size, Transform},
};

/// Taps on each side of the centre (the shader's loop).
const TAPS: i32 = 12;

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
// a tap's step, in texture coordinates (across or down)
uniform vec2 dir;

void main() {
    vec4 sum = vec4(0.0);
    float total = 0.0;
    // 25 taps, two and a half standard deviations each side
    for (int i = -12; i <= 12; i++) {
        float x = float(i) / 5.0;
        float w = exp(-0.5 * x * x);
        sum += texture2D(tex, v_coords + dir * float(i)) * w;
        total += w;
    }
    gl_FragColor = sum / total * alpha;
}
"#;

/// A texture drawn onto a rectangle of the output: the blurred scene, the opening picture.
pub struct TextureElement {
    pub id: Id,
    pub commit: CommitCounter,
    pub texture: GlesTexture,
    pub src: Rectangle<f64, Buffer>,
    pub dst: Rectangle<i32, Physical>,
    pub alpha: f32,
}

impl Element for TextureElement {
    fn id(&self) -> &Id {
        &self.id
    }
    fn current_commit(&self) -> CommitCounter {
        self.commit
    }
    fn src(&self) -> Rectangle<f64, Buffer> {
        self.src
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
        if self.alpha >= 1.0 {
            OpaqueRegions::from_slice(&[Rectangle::from_size(self.geometry(scale).size)])
        } else {
            OpaqueRegions::default()
        }
    }
    fn alpha(&self) -> f32 {
        self.alpha
    }
    fn kind(&self) -> Kind {
        Kind::Unspecified
    }
}

impl RenderElement<GlesRenderer> for TextureElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        frame.render_texture_from_to(&self.texture, src, dst, damage, opaque_regions, Transform::Normal, self.alpha, None, &[])
    }
}

/// Kept with each output: the offscreen scene and its damage tracker, the half and the two quarter
/// textures, the program, and what was blurred last.
#[derive(Default)]
pub struct BlurTarget(RefCell<Option<Target>>);

struct Target {
    size: (i32, i32),
    scene: GlesTexture,
    damage: OutputDamageTracker,
    half: GlesTexture,
    quarter: [GlesTexture; 2],
    program: GlesTexProgram,
    id: Id,
    commit: CommitCounter,
    radius: f64,
}

/// One pass: `from` (all of it) drawn over all of `to` (`to_size` pixels), through `program` if any.
fn pass(renderer: &mut GlesRenderer, from: &GlesTexture, from_size: (i32, i32), to: &mut GlesTexture, to_size: (i32, i32), program: Option<(&GlesTexProgram, [f32; 2])>) -> Option<()> {
    let mut fb = renderer.bind(to).ok()?;
    let size: Size<i32, Physical> = to_size.into();
    let mut frame = renderer.render(&mut fb, size, Transform::Normal).ok()?;
    let whole = Rectangle::from_size(size);
    let src = Rectangle::<f64, Buffer>::from_size((from_size.0 as f64, from_size.1 as f64).into());
    let result = match program {
        Some((p, dir)) => frame.render_texture_from_to(from, src, whole, &[whole], &[whole], Transform::Normal, 1.0, Some(p), &[Uniform::new("dir", dir)]),
        None => frame.render_texture_from_to(from, src, whole, &[whole], &[whole], Transform::Normal, 1.0, None, &[]),
    };
    result.ok()?;
    let _ = frame.finish().ok()?;
    Some(())
}

/// `scene` (the elements of `output`, front first) blurred by `radius` physical pixels (about the
/// Gaussian's reach, two and a half deviations), as one element over `area` of the output (physical
/// pixels: the inside of the frame, the margins left sharp). None when it cannot be drawn.
pub fn blur<E: RenderElement<GlesRenderer>>(renderer: &mut GlesRenderer, output: &Output, size: (i32, i32), scene: &[E], radius: f64, area: Rectangle<i32, Physical>) -> Option<TextureElement> {
    if size.0 < 8 || size.1 < 8 {
        return None;
    }
    output.user_data().insert_if_missing(BlurTarget::default);
    let cell = output.user_data().get::<BlurTarget>()?;
    let mut target = cell.0.borrow_mut();
    let half_size = ((size.0 + 1) / 2, (size.1 + 1) / 2);
    let quarter_size = ((half_size.0 + 1) / 2, (half_size.1 + 1) / 2);
    if target.as_ref().is_none_or(|t| t.size != size) {
        let texture = |renderer: &mut GlesRenderer, s: (i32, i32)| -> Option<GlesTexture> { renderer.create_buffer(Fourcc::Abgr8888, (s.0, s.1).into()).ok() };
        let program = match target.take() {
            Some(t) => t.program,
            None => renderer
                .compile_custom_texture_shader(SHADER, &[UniformName::new("dir", UniformType::_2f)])
                .map_err(|e| eprintln!("draftpaper: the blur's shader: {e:?}"))
                .ok()?,
        };
        *target = Some(Target {
            size,
            scene: texture(renderer, size)?,
            damage: OutputDamageTracker::new(size, output.current_scale().fractional_scale(), Transform::Normal),
            half: texture(renderer, half_size)?,
            quarter: [texture(renderer, quarter_size)?, texture(renderer, quarter_size)?],
            program,
            id: Id::new(),
            commit: CommitCounter::default(),
            radius: -1.0,
        });
    }
    let t = target.as_mut()?;

    let changed = {
        let mut fb = renderer.bind(&mut t.scene).ok()?;
        let result = t.damage.render_output(renderer, &mut fb, 1, scene, [0.0, 0.0, 0.0, 1.0]).ok()?;
        result.damage.is_some_and(|d| !d.is_empty())
    };
    if changed || t.radius != radius {
        t.radius = radius;
        // in quarter pixels, a tap apart: twelve taps reach the radius
        let step = (radius / 4.0 / TAPS as f64).clamp(0.0, 3.0) as f32;
        pass(renderer, &t.scene, size, &mut t.half, half_size, None)?;
        let [a, b] = &mut t.quarter;
        pass(renderer, &t.half, half_size, a, quarter_size, None)?;
        pass(renderer, a, quarter_size, b, quarter_size, Some((&t.program, [step / quarter_size.0 as f32, 0.0])))?;
        pass(renderer, b, quarter_size, a, quarter_size, Some((&t.program, [0.0, step / quarter_size.1 as f32])))?;
        t.commit.increment();
    }
    Some(TextureElement {
        id: t.id.clone(),
        commit: t.commit,
        texture: t.quarter[0].clone(),
        // the area, in the quarter texture's pixels
        src: Rectangle::new(
            (area.loc.x as f64 * quarter_size.0 as f64 / size.0 as f64, area.loc.y as f64 * quarter_size.1 as f64 / size.1 as f64).into(),
            (area.size.w as f64 * quarter_size.0 as f64 / size.0 as f64, area.size.h as f64 * quarter_size.1 as f64 / size.1 as f64).into(),
        ),
        dst: area,
        alpha: 1.0,
    })
}
