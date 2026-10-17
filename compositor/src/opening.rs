//! The opening: what the displays showed when the compositor took them over (the login screen's last
//! frame: the sheet blurred, the name in the title block; or the session's, back at the login screen;
//! or another display manager's), shown as it is until the desktop is up, then revealed: the picture
//! fades into the desktop drawn blurred as the login screen blurred it, and the blur goes to nothing.
//!
//! On the TTY the picture is the framebuffer the display scans out, read back (GETFB2, as master) and
//! imported as a texture: the very pixels, whoever drew them. Headless, DRAFTPAPER_OPENING=FILE.png
//! stands in for it (the tests). The desktop is up when the shell (or the login card) says it has
//! lettered everything once (IPC "ready"), a moment later; or after DRAFTPAPER_OPENING_WAIT seconds (5)
//! whatever happens.

use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

use smithay::{
    backend::{
        allocator::{
            dmabuf::{Dmabuf, DmabufFlags},
            Fourcc,
        },
        renderer::{
            element::Id,
            gles::{GlesRenderer, GlesTexture},
            utils::CommitCounter,
            ImportDma, ImportMem,
        },
    },
    output::Output,
    reexports::drm::{
        buffer::DrmModifier,
        control::{framebuffer, Device as ControlDevice},
    },
    utils::Rectangle,
};

use crate::blur::TextureElement;

/// How blurred the sheet is behind the login card, and so at the start of the reveal, in millimetres.
pub const BLUR_MM: f64 = 6.0;
/// After the shell said it is ready, before the reveal: its last frames on screen.
const SETTLE: Duration = Duration::from_millis(100);
/// The picture fading into the blurred desktop, and the blur going.
const FADE: Duration = Duration::from_millis(250);
const REVEAL: Duration = Duration::from_millis(700);

struct Opening {
    texture: GlesTexture,
    size: (i32, i32),
    since: Instant,
    /// when the desktop was first seen up (a layer surface on the output)
    up: Option<Instant>,
    /// when the reveal began
    reveal: Option<Instant>,
    id: Id,
    commit: CommitCounter,
}

/// Kept with each output while its opening lasts.
#[derive(Default)]
pub struct OpeningCell(RefCell<Option<Opening>>);

fn wait() -> Duration {
    let secs = std::env::var("DRAFTPAPER_OPENING_WAIT").ok().and_then(|s| s.parse::<f64>().ok()).unwrap_or(5.0);
    Duration::from_secs_f64(secs.clamp(0.0, 60.0))
}

/// Give `output` its opening picture (of the output's size in pixels, else none).
pub fn start(output: &Output, texture: GlesTexture, size: (i32, i32)) {
    if size != crate::render::mode_size(output) {
        eprintln!("draftpaper: {}: the picture on it is {}x{}, not its size: no opening", output.name(), size.0, size.1);
        return;
    }
    output.user_data().insert_if_missing(OpeningCell::default);
    if let Some(cell) = output.user_data().get::<OpeningCell>() {
        *cell.0.borrow_mut() = Some(Opening { texture, size, since: Instant::now(), up: None, reveal: None, id: Id::new(), commit: CommitCounter::default() });
    }
}

/// What the opening adds to a frame of `output` now: the picture over everything (its opacity going
/// from 1 to 0) and how blurred the desktop under it is (millimetres); `alone` while only the picture is
/// shown. None once it is over (or there was none). `up`: the shell has said it is ready (IPC "ready").
pub fn frame(output: &Output, up: bool) -> Option<(TextureElement, f64, bool)> {
    let cell = output.user_data().get::<OpeningCell>()?;
    let mut slot = cell.0.borrow_mut();
    let o = slot.as_mut()?;
    let now = Instant::now();
    if up && o.up.is_none() {
        o.up = Some(now);
        eprintln!("draftpaper: {}: opening: the desktop up after {} ms", output.name(), o.since.elapsed().as_millis());
    }
    if o.reveal.is_none() && (o.up.is_some_and(|t| now >= t + SETTLE) || now >= o.since + wait()) {
        o.reveal = Some(now);
        eprintln!("draftpaper: {}: opening: revealing after {} ms", output.name(), o.since.elapsed().as_millis());
    }
    let element = |o: &mut Opening, alpha: f32| TextureElement {
        id: o.id.clone(),
        commit: o.commit,
        texture: o.texture.clone(),
        src: Rectangle::from_size((o.size.0 as f64, o.size.1 as f64).into()),
        dst: Rectangle::from_size(o.size.into()),
        alpha,
    };
    let Some(t0) = o.reveal else {
        return Some((element(o, 1.0), BLUR_MM, true));
    };
    let t = now - t0;
    if t >= REVEAL {
        eprintln!("draftpaper: {}: opening: over after {} ms", output.name(), o.since.elapsed().as_millis());
        *slot = None;
        return None;
    }
    let fade = 1.0 - (t.as_secs_f64() / FADE.as_secs_f64()).min(1.0);
    let e = t.as_secs_f64() / REVEAL.as_secs_f64();
    // ease out: quick at first, settling
    let blur = BLUR_MM * (1.0 - e).powi(3);
    o.commit.increment();
    Some((element(o, fade as f32), blur, false))
}

/// The framebuffer `fb` (another program's, still on screen) as a texture: its buffers exported and
/// imported. Needs the device's master (or root) to see its buffers.
pub fn from_framebuffer(device: &impl ControlDevice, renderer: &mut GlesRenderer, fb: framebuffer::Handle) -> Option<(GlesTexture, (i32, i32))> {
    let info = device.get_planar_framebuffer(fb).map_err(|e| eprintln!("draftpaper: the picture on screen: {e}")).ok()?;
    let (w, h) = info.size();
    let modifier = info.modifier().unwrap_or(DrmModifier::Invalid);
    eprintln!("draftpaper: the picture on screen: framebuffer {} {}x{} {:?} {:?}", u32::from(fb), w, h, info.pixel_format(), modifier);
    let mut builder = Dmabuf::builder((w as i32, h as i32), info.pixel_format(), modifier, DmabufFlags::empty());
    let handles = info.buffers();
    let mut closed = Vec::new();
    let mut ok = true;
    for (i, handle) in handles.iter().enumerate() {
        let Some(handle) = handle else { continue };
        // O_CLOEXEC
        match device.buffer_to_prime_fd(*handle, 0o2000000) {
            Ok(fd) => {
                builder.add_plane(fd, i as u32, info.offsets()[i], info.pitches()[i]);
            }
            Err(e) => {
                eprintln!("draftpaper: the picture on screen: export: {e}");
                ok = false;
            }
        }
        if !closed.contains(handle) {
            closed.push(*handle);
        }
    }
    for handle in closed {
        let _ = device.close_buffer(handle);
    }
    let planes = handles.iter().filter(|h| h.is_some()).count();
    eprintln!("draftpaper: the picture on screen: {planes} plane(s)");
    let dmabuf = builder.build().filter(|_| ok)?;
    let texture = renderer.import_dmabuf(&dmabuf, None).map_err(|e| eprintln!("draftpaper: the picture on screen: import: {e:?}")).ok()?;
    Some((texture, (w as i32, h as i32)))
}

/// A PNG as a texture (DRAFTPAPER_OPENING, headless).
pub fn from_png(renderer: &mut GlesRenderer, path: &std::path::Path) -> Option<(GlesTexture, (i32, i32))> {
    let image = image::open(path).map_err(|e| eprintln!("draftpaper: opening {}: {e}", path.display())).ok()?.to_rgba8();
    let (w, h) = (image.width() as i32, image.height() as i32);
    let texture = renderer.import_memory(image.as_raw(), Fourcc::Abgr8888, (w, h).into(), false).ok()?;
    Some((texture, (w, h)))
}

/// The device for `grab`: a DRM node opened for reading what it shows.
struct Card(std::fs::File);
impl std::os::fd::AsFd for Card {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl smithay::reexports::drm::Device for Card {}
impl ControlDevice for Card {}

/// `draftpaper-compositor grab DEVICE DIR [WIDTH]` (as root): each CRTC's picture, read back and imported as the
/// opening does it, to DIR/grab-<crtc>.png, with how much of it is opaque. Changes nothing on the device (it never becomes its master
/// while a session holds it): for checking the opening.
pub fn grab(args: &[String]) -> i32 {
    use smithay::backend::{
        allocator::gbm::GbmDevice,
        egl::{EGLContext, EGLDisplay},
        renderer::{Bind, ExportMem, Frame, Offscreen, Renderer},
    };
    use smithay::utils::{Physical, Size, Transform};
    let (Some(path), Some(dir)) = (args.first(), args.get(1)) else {
        eprintln!("draftpaper-compositor grab DEVICE DIR [WIDTH]");
        return 2;
    };
    // WIDTH: scaled down to it (a series of them stays small)
    let width = args.get(2).and_then(|w| w.parse::<i32>().ok()).filter(|w| *w > 0);
    let Ok(file) = std::fs::OpenOptions::new().read(true).write(true).open(path) else {
        eprintln!("draftpaper: grab: cannot open {path}");
        return 1;
    };
    let card = Card(file.try_clone().unwrap());
    let Ok(gbm) = GbmDevice::new(file) else { return 1 };
    let Ok(egl) = (unsafe { EGLDisplay::new(gbm) }) else { return 1 };
    let Ok(context) = EGLContext::new(&egl) else { return 1 };
    let Ok(mut renderer) = (unsafe { GlesRenderer::new(context) }) else { return 1 };
    let Ok(resources) = card.resource_handles() else { return 1 };
    for &crtc in resources.crtcs() {
        let Some(fb) = card.get_crtc(crtc).ok().and_then(|c| c.framebuffer()) else { continue };
        let Some((texture, (w, h))) = from_framebuffer(&card, &mut renderer, fb) else { continue };
        eprintln!("draftpaper: grab: crtc {}: black: {}", u32::from(crtc), is_black(&mut renderer, &texture, (w, h)));
        // drawn as the opening draws it (blended), over the clear colour the compositor uses
        let (ow, oh) = match width {
            Some(tw) if tw < w => (tw, (h as i64 * tw as i64 / w as i64) as i32),
            _ => (w, h),
        };
        let result = (|| -> Option<Vec<u8>> {
            let size: Size<i32, Physical> = (ow, oh).into();
            let mut target: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, (ow, oh).into()).ok()?;
            {
                let mut fb = renderer.bind(&mut target).ok()?;
                let mut frame = renderer.render(&mut fb, size, Transform::Normal).ok()?;
                let whole = Rectangle::from_size(size);
                frame.clear([0.1, 0.1, 0.1, 0.0].into(), &[whole]).ok()?;
                frame.render_texture_from_to(&texture, Rectangle::from_size((w as f64, h as f64).into()), whole, &[whole], &[], Transform::Normal, 1.0, None, &[]).ok()?;
                let _ = frame.finish().ok()?;
            }
            let fb = renderer.bind(&mut target).ok()?;
            let mapping = renderer.copy_framebuffer(&fb, Rectangle::from_size((ow, oh).into()), Fourcc::Abgr8888).ok()?;
            Some(renderer.map_texture(&mapping).ok()?.to_vec())
        })();
        let Some(mut rgba) = result else { continue };
        // the picture's own alpha (over a clear one): a display ignores it, a texture does not
        let opaque = rgba.chunks_exact(4).filter(|p| p[3] == 255).count();
        eprintln!("draftpaper: grab: crtc {}: {:.1}% of the pixels opaque", u32::from(crtc), 100.0 * opaque as f64 / (rgba.len() / 4) as f64);
        rgba.chunks_exact_mut(4).for_each(|p| p[3] = 255);
        let out = std::path::Path::new(dir).join(format!("grab-{}.png", u32::from(crtc)));
        let Ok(f) = std::fs::File::create(&out) else { continue };
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(f), ow as u32, oh as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        if encoder.write_header().and_then(|mut wr| wr.write_image_data(&rgba)).is_ok() {
            eprintln!("draftpaper: grab: {}", out.display());
        }
    }
    0
}

/// Whether `texture` (of `size`) is black all over: drawn small offscreen and read back. A picture like
/// that is no opening (the screen was dark, or the picture did not come through).
pub fn is_black(renderer: &mut GlesRenderer, texture: &GlesTexture, size: (i32, i32)) -> bool {
    use smithay::backend::renderer::{Bind, ExportMem, Frame, Offscreen, Renderer};
    use smithay::utils::{Physical, Size, Transform};
    let small: Size<i32, Physical> = (32, 20).into();
    let read = (|| -> Option<Vec<u8>> {
        let mut target: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, (small.w, small.h).into()).ok()?;
        {
            let mut fb = renderer.bind(&mut target).ok()?;
            let mut frame = renderer.render(&mut fb, small, Transform::Normal).ok()?;
            let whole = Rectangle::from_size(small);
            frame.clear([0.0, 0.0, 0.0, 1.0].into(), &[whole]).ok()?;
            let src = Rectangle::from_size((size.0 as f64, size.1 as f64).into());
            frame.render_texture_from_to(texture, src, whole, &[whole], &[], Transform::Normal, 1.0, None, &[]).ok()?;
            let _ = frame.finish().ok()?;
        }
        let fb = renderer.bind(&mut target).ok()?;
        let mapping = renderer.copy_framebuffer(&fb, Rectangle::from_size((small.w, small.h).into()), Fourcc::Abgr8888).ok()?;
        Some(renderer.map_texture(&mapping).ok()?.to_vec())
    })();
    match read {
        Some(pixels) => pixels.chunks_exact(4).all(|p| p[0] < 6 && p[1] < 6 && p[2] < 6),
        // not readable: not trusted either
        None => true,
    }
}

/// The picture as an element over the whole output, opaque: for Smithay's first frame on it (tty.rs).
pub fn element(texture: &GlesTexture, size: (i32, i32)) -> TextureElement {
    TextureElement {
        id: Id::new(),
        commit: CommitCounter::default(),
        texture: texture.clone(),
        src: Rectangle::from_size((size.0 as f64, size.1 as f64).into()),
        dst: Rectangle::from_size(size.into()),
        alpha: 1.0,
    }
}
