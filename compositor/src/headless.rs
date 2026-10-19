//! The headless backend (`--headless WxH[,WxH...] [--scale S]`), for tests: an output of each size, side by
//! side (the first named "headless"), drawn
//! offscreen on the GPU's render node and never shown. A render node draws but cannot drive a display, so
//! the displays of a running session are never touched; nothing opens a window either. Clients (the shell,
//! terminals) connect as to any session; SIGUSR1 writes a screenshot to ~/Pictures, and `make pixels`
//! checks those (examples/sheetcheck.rs). DRAFTPAPER_MM sets the pixels per millimetre (else 6.2).

use smithay::{
    backend::{
        allocator::{gbm::GbmDevice, Fourcc},
        drm::DrmNode,
        egl::{EGLContext, EGLDisplay},
        renderer::{damage::OutputDamageTracker, gles::{GlesRenderer, GlesTexture}, Bind, ImportDma, Offscreen},
    },
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::calloop::{
        timer::{TimeoutAction, Timer},
        EventLoop,
    },
    utils::Transform,
};

use crate::{render::{attach_sheet, output_elements}, theme::Theme, Draftpaper};

/// The renderer and the outputs, each with its offscreen target, kept in the state while headless.
pub struct Headless {
    pub renderer: GlesRenderer,
    outputs: Vec<(Output, Option<(GlesTexture, (i32, i32), OutputDamageTracker)>)>,
    /// the frame timer, and whether it is idle (the last frame unchanged: IDLE until woken)
    timer: Option<smithay::reexports::calloop::RegistrationToken>,
    idle: bool,
    last: Option<std::time::Instant>,
}

/// A frame every 16 ms while something changes; idle, a look every second (a safety net), as tty.rs.
const FRAME: std::time::Duration = std::time::Duration::from_millis(16);
const IDLE: std::time::Duration = std::time::Duration::from_secs(1);

impl Headless {
    pub fn import_dmabuf(&mut self, dmabuf: &smithay::backend::allocator::dmabuf::Dmabuf) -> bool {
        self.renderer.import_dmabuf(dmabuf, None).is_ok()
    }
}

/// The first render node (/dev/dri/renderD*).
fn render_node() -> Option<std::path::PathBuf> {
    let mut nodes: Vec<_> = std::fs::read_dir("/dev/dri").ok()?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("renderD"))).collect();
    nodes.sort();
    nodes.into_iter().next()
}

/// `sizes`: one output each, left to right in that order, their tops level (the first named "headless", the
/// next "headless-2" and so on); DRAFTPAPER_MM a density for all, or one per output ("6.4,4.3").
pub fn init_headless(_event_loop: &mut EventLoop<'static, Draftpaper>, data: &mut Draftpaper, sizes: &[(i32, i32)], scale: f64) -> Result<(), Box<dyn std::error::Error>> {
    let path = render_node().ok_or("no render node in /dev/dri")?;
    let file = std::fs::OpenOptions::new().read(true).write(true).open(&path)?;
    let node = DrmNode::from_path(&path)?;
    let gbm = GbmDevice::new(file)?;
    let egl = unsafe { EGLDisplay::new(gbm) }?;
    let context = EGLContext::new(&egl)?;
    let renderer = unsafe { GlesRenderer::new(context) }?;

    // clients' GPU buffers, as on the TTY
    let formats = renderer.dmabuf_formats();
    let feedback = smithay::wayland::dmabuf::DmabufFeedbackBuilder::new(node.dev_id(), formats).build()?;
    let global = data.dmabuf_state.create_global_with_default_feedback::<Draftpaper>(&data.display_handle, &feedback);
    data.dmabuf_global = Some(global);

    let densities: Vec<f32> = std::env::var("DRAFTPAPER_MM").ok().map(|v| v.split(',').filter_map(|m| m.parse().ok()).collect()).unwrap_or_default();
    // DRAFTPAPER_MODELS="0x1488,ASUS MB166C": the displays' models (their settings follow them, theme.rs)
    let models: Vec<String> = std::env::var("DRAFTPAPER_MODELS").ok().map(|v| v.split(',').map(String::from).collect()).unwrap_or_default();
    let mut renderer = renderer;
    let mut outputs = Vec::new();
    let mut x = 0;
    for (i, &size) in sizes.iter().enumerate() {
        let mm = densities.get(i).or(densities.last()).copied().unwrap_or(6.2f32);
        let name = if i == 0 { "headless".to_string() } else { format!("headless-{}", i + 1) };
        let output = Output::new(
            name.clone(),
            PhysicalProperties {
                size: ((size.0 as f32 / mm).round() as i32, (size.1 as f32 / mm).round() as i32).into(),
                subpixel: Subpixel::Unknown,
                make: "Draftpaper".into(),
                model: models.get(i).cloned().unwrap_or_else(|| "Headless".into()),
            },
        );
        let _global = output.create_global::<Draftpaper>(&data.display_handle);
        let mode = Mode { size: size.into(), refresh: 60_000 };
        output.change_current_state(Some(mode), Some(Transform::Normal), Some(Scale::Fractional(scale)), Some((x, 0).into()));
        output.set_preferred(mode);
        output.user_data().insert_if_missing(|| crate::theme::PlugDefaults { scale, position: (x, 0) });
        data.space.map_output(&output, (x, 0));
        x += (size.0 as f64 / scale).round() as i32;
        let theme = Theme::load();
        eprintln!("draftpaper: headless {name} {}x{} at {scale}, sheet in theme {}, margin {} mm, {mm} px/mm", size.0, size.1, theme.name, theme.margin_mm);
        let margin_mm = theme.margin_mm as f64;
        attach_sheet(&output, theme, mm);
        // measured when the test names the density (DRAFTPAPER_MM); without it, the guess a display without
        // a size gets
        data.add_screen(output.clone(), mm as f64, margin_mm, !densities.is_empty());
        // DRAFTPAPER_OPENING=FILE.png: the picture the first display would have shown before (opening.rs)
        if let (0, Some(path)) = (i, std::env::var_os("DRAFTPAPER_OPENING")) {
            if let Some((texture, size)) = crate::opening::from_png(&mut renderer, std::path::Path::new(&path)) {
                if crate::opening::is_black(&mut renderer, &texture, size) {
                    eprintln!("draftpaper: headless: the picture is black: no opening");
                } else {
                    crate::opening::start(&output, texture, size);
                }
            }
        }
        outputs.push((output, None));
    }
    data.headless = Some(Headless { renderer, outputs, timer: None, idle: false, last: None });
    crate::gamma::refresh_all(data);
    std::env::set_var("WAYLAND_DISPLAY", &data.socket_name);

    // frames, drawn offscreen where something changed: every 16 ms while they change, idle else (wake)
    schedule(data, std::time::Duration::ZERO);
    Ok(())
}

/// Draw every output; whether any of them changed.
fn frame(data: &mut Draftpaper) -> bool {
    let Some(mut h) = data.headless.take() else { return false };
    let mut changed = false;
    data.anim.prune(std::time::Instant::now());
    for (output, target) in h.outputs.iter_mut() {
        let size = crate::render::mode_size(output);
        // the texture kept from the last frame: its age is 1 (only what changed is drawn); a new one, 0
        let mut age = 1;
        if target.as_ref().is_none_or(|t| t.1 != size) {
            age = 0;
            match h.renderer.create_buffer(Fourcc::Abgr8888, (size.0, size.1).into()) {
                Ok(texture) => *target = Some((texture, size, OutputDamageTracker::from_output(output))),
                Err(e) => eprintln!("draftpaper: headless target: {e:?}"),
            }
        }
        let elements = output_elements(data, &mut h.renderer, output, None);
        if let Some((texture, _, damage)) = target.as_mut() {
            if let Ok(mut fb) = h.renderer.bind(texture) {
                if let Ok(r) = damage.render_output(&mut h.renderer, &mut fb, age, &elements, [0.1, 0.1, 0.1, 1.0]) {
                    if r.damage.is_some_and(|d| !d.is_empty()) {
                        changed = true;
                        *data.frames.entry(output.name()).or_default() += 1;
                    }
                }
            }
        }
    }
    if !data.screencopies.is_empty() {
        crate::screencopy::finish(data, &mut h.renderer, None);
    }
    let shoot = std::mem::take(&mut data.screenshot_requested);
    for (output, _) in &h.outputs {
        if shoot {
            let path = crate::tty::screenshot_path(output);
            match crate::tty::capture(data, &mut h.renderer, output, &path) {
                Ok(()) => eprintln!("draftpaper: screenshot {}", path.display()),
                Err(e) => eprintln!("draftpaper: screenshot: {e}"),
            }
        }
        data.send_frames(output, data.start_time.elapsed());
    }
    data.space.refresh();
    data.popups.cleanup();
    let _ = data.display_handle.flush_clients();
    data.headless = Some(h);
    changed
}

/// The next frame `delay` from now, then every FRAME while frames change, IDLE once one does not.
fn schedule(data: &mut Draftpaper, delay: std::time::Duration) {
    let handle = data.loop_handle.clone();
    let token = handle.insert_source(Timer::from_duration(delay), |_, _, data| {
        let changed = frame(data);
        let Some(h) = data.headless.as_mut() else { return TimeoutAction::Drop };
        h.last = Some(std::time::Instant::now());
        h.idle = !changed;
        TimeoutAction::ToDuration(if changed { FRAME } else { IDLE })
    });
    if let (Some(h), Ok(token)) = (data.headless.as_mut(), token) {
        if let Some(old) = h.timer.replace(token) {
            data.loop_handle.remove(old);
        }
        h.idle = false;
    }
}

/// Something may have changed the picture: an idle headless session draws again, a FRAME after its last.
pub fn wake(data: &mut Draftpaper) {
    let Some(h) = data.headless.as_ref() else { return };
    if !h.idle {
        return;
    }
    let next = h.last.map(|t| (t + FRAME).saturating_duration_since(std::time::Instant::now())).unwrap_or_default();
    schedule(data, next);
}

/// The output as drawn now, to DIR/last-<output>.png (DRAFTPAPER_LAST_FRAME, handoff.rs).
pub fn capture_into(data: &mut Draftpaper, dir: &std::path::Path) {
    let Some(mut h) = data.headless.take() else { return };
    for (output, _) in &h.outputs {
        let path = dir.join(format!("last-{}.png", output.name()));
        if let Err(e) = crate::tty::capture(data, &mut h.renderer, output, &path) {
            eprintln!("draftpaper: last frame: {e}");
        }
    }
    data.headless = Some(h);
}
