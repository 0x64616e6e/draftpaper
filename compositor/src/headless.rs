//! The headless backend (`--headless WxH [--scale S]`), for tests: one output of that many pixels, drawn
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

/// The renderer and the one output, kept in the state while headless.
pub struct Headless {
    pub renderer: GlesRenderer,
    output: Output,
    target: Option<(GlesTexture, (i32, i32), OutputDamageTracker)>,
}

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

pub fn init_headless(event_loop: &mut EventLoop<'static, Draftpaper>, data: &mut Draftpaper, size: (i32, i32), scale: f64) -> Result<(), Box<dyn std::error::Error>> {
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

    let mm = std::env::var("DRAFTPAPER_MM").ok().and_then(|v| v.parse().ok()).unwrap_or(6.2f32);
    let output = Output::new(
        "headless".to_string(),
        PhysicalProperties {
            size: ((size.0 as f32 / mm).round() as i32, (size.1 as f32 / mm).round() as i32).into(),
            subpixel: Subpixel::Unknown,
            make: "Draftpaper".into(),
            model: "Headless".into(),
        },
    );
    let _global = output.create_global::<Draftpaper>(&data.display_handle);
    let mode = Mode { size: size.into(), refresh: 60_000 };
    output.change_current_state(Some(mode), Some(Transform::Normal), Some(Scale::Fractional(scale)), Some((0, 0).into()));
    output.set_preferred(mode);
    data.space.map_output(&output, (0, 0));
    let theme = Theme::load();
    eprintln!("draftpaper: headless {}x{} at {scale}, sheet in theme {}, margin {} mm, {mm} px/mm", size.0, size.1, theme.name, theme.margin_mm);
    let margin_mm = theme.margin_mm as f64;
    attach_sheet(&output, theme, mm);
    data.add_screen(output.clone(), mm as f64, margin_mm);
    data.headless = Some(Headless { renderer, output, target: None });
    std::env::set_var("WAYLAND_DISPLAY", &data.socket_name);

    // a frame every 16 ms, drawn offscreen where something changed
    event_loop.handle().insert_source(Timer::immediate(), |_, _, data| {
        frame(data);
        TimeoutAction::ToDuration(std::time::Duration::from_millis(16))
    })?;
    Ok(())
}

fn frame(data: &mut Draftpaper) {
    let Some(mut h) = data.headless.take() else { return };
    data.anim.prune(std::time::Instant::now());
    let output = h.output.clone();
    let size = crate::render::mode_size(&output);
    if h.target.as_ref().is_none_or(|t| t.1 != size) {
        match h.renderer.create_buffer(Fourcc::Abgr8888, (size.0, size.1).into()) {
            Ok(texture) => h.target = Some((texture, size, OutputDamageTracker::from_output(&output))),
            Err(e) => eprintln!("draftpaper: headless target: {e:?}"),
        }
    }
    let elements = output_elements(data, &mut h.renderer, &output, None);
    if let Some((texture, _, damage)) = h.target.as_mut() {
        if let Ok(mut fb) = h.renderer.bind(texture) {
            let _ = damage.render_output(&mut h.renderer, &mut fb, 0, &elements, [0.1, 0.1, 0.1, 1.0]);
        }
    }
    drop(elements);
    if !data.screencopies.is_empty() {
        crate::screencopy::finish(data, &mut h.renderer, None);
    }
    if std::mem::take(&mut data.screenshot_requested) {
        let path = crate::tty::screenshot_path(&output);
        match crate::tty::capture(data, &mut h.renderer, &output, &path) {
            Ok(()) => eprintln!("draftpaper: screenshot {}", path.display()),
            Err(e) => eprintln!("draftpaper: screenshot: {e}"),
        }
    }
    data.send_frames(&output, data.start_time.elapsed());
    data.space.refresh();
    data.popups.cleanup();
    let _ = data.display_handle.flush_clients();
    data.headless = Some(h);
}

/// The output as drawn now, to DIR/last-<output>.png (DRAFTPAPER_LAST_FRAME, handoff.rs).
pub fn capture_into(data: &mut Draftpaper, dir: &std::path::Path) {
    let Some(mut h) = data.headless.take() else { return };
    let output = h.output.clone();
    let path = dir.join(format!("last-{}.png", output.name()));
    if let Err(e) = crate::tty::capture(data, &mut h.renderer, &output, &path) {
        eprintln!("draftpaper: last frame: {e}");
    }
    data.headless = Some(h);
}
