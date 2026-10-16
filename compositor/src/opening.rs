//! The opening: what the displays showed when the compositor took them over (the login screen's last
//! frame: the sheet blurred, the name in the title block; or the session's, back at the login screen;
//! or another display manager's), shown as it is until the desktop is up, then revealed: the picture
//! fades into the desktop drawn blurred as the login screen blurred it, and the blur goes to nothing.
//!
//! On the TTY the picture is the framebuffer the display scans out, read back (GETFB2, as master) and
//! imported as a texture: the very pixels, whoever drew them. Headless, DRAFTPAPER_OPENING=FILE.png
//! stands in for it (the tests). The desktop is up when a layer surface (the shell) is on the output, a
//! moment later; or after DRAFTPAPER_OPENING_WAIT seconds (5) whatever happens.

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
        drm::DrmDevice,
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
/// After the shell's first surface, before the reveal: its first frames drawn.
const SETTLE: Duration = Duration::from_millis(300);
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
/// shown. None once it is over (or there was none). `up`: the desktop has a surface on this output.
pub fn frame(output: &Output, up: bool) -> Option<(TextureElement, f64, bool)> {
    let cell = output.user_data().get::<OpeningCell>()?;
    let mut slot = cell.0.borrow_mut();
    let o = slot.as_mut()?;
    let now = Instant::now();
    if up && o.up.is_none() {
        o.up = Some(now);
    }
    if o.reveal.is_none() && (o.up.is_some_and(|t| now >= t + SETTLE) || now >= o.since + wait()) {
        o.reveal = Some(now);
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
pub fn from_framebuffer(device: &DrmDevice, renderer: &mut GlesRenderer, fb: framebuffer::Handle) -> Option<(GlesTexture, (i32, i32))> {
    let info = device.get_planar_framebuffer(fb).map_err(|e| eprintln!("draftpaper: the picture on screen: {e}")).ok()?;
    let (w, h) = info.size();
    let modifier = info.modifier().unwrap_or(DrmModifier::Invalid);
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
