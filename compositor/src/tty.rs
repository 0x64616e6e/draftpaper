//! The TTY backend: Draftpaper on the real screens, started from a text console. A libseat session
//! (logind or seatd) opens the devices; the primary GPU renders with GLES and drives every connected
//! display at its preferred mode through Smithay's DRM compositor; libinput reads keyboards, touchpads
//! and the TrackPoint. One GPU only. Displays plugged in or out later are picked up from udev; each has
//! its own screen (screens.rs), with scale and position from config.toml ([outputs.<name>]).
//!
//! Clients render on the GPU: linux-dmabuf is offered with the GPU's render node and formats as its
//! default feedback (without it Mesa falls back to llvmpipe, rendering on the CPU).
//!
//! Follows the structure of anvil's udev.rs (Smithay v0.7.0, MIT), without multi-GPU, leasing and
//! per-surface dmabuf feedback.

use std::{collections::HashMap, path::PathBuf, time::Duration};

use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{
            compositor::FrameFlags,
            exporter::gbm::GbmFramebufferExporter,
            output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements},
            DrmDevice, DrmDeviceFd, DrmEvent, DrmNode,
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{gles::GlesRenderer, ImportDma, ImportMemWl},
        session::{libseat::LibSeatSession, Event as SessionEvent, Session},
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
    },
    output::{Mode as WlMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop, LoopHandle,
        },
        drm::control::{connector, crtc, Device as ControlDevice, ModeTypeFlags},
        input::Libinput,
        rustix::fs::OFlags,
    },
    utils::DeviceFd,
};

use crate::{
    cursor::Cursor,
    render::{attach_sheet, output_elements, OutputElements},
    theme::{output_settings, Theme},
    Draftpaper,
};

type Allocator = GbmAllocator<DrmDeviceFd>;
type Exporter = GbmFramebufferExporter<DrmDeviceFd>;
type Manager = DrmOutputManager<Allocator, Exporter, (), DrmDeviceFd>;
type Surface = DrmOutput<Allocator, Exporter, (), DrmDeviceFd>;

/// About 160 PPI, for displays that report no physical size.
const FALLBACK_MM: f32 = 6.2;

pub struct TtyData {
    pub session: LibSeatSession,
    renderer: GlesRenderer,
    manager: Manager,
    node: DrmNode,
    /// per crtc: the output, its DRM surface, and the connector driving it
    outputs: HashMap<crtc::Handle, (Output, Surface, connector::Handle)>,
    cursor: Cursor,
    libinput: Libinput,
    /// input devices, for touchpad settings
    devices: Vec<smithay::reexports::input::Device>,
}

impl TtyData {
    /// Whether the renderer takes a client's dmabuf (linux-dmabuf's import check).
    pub fn import_dmabuf(&mut self, dmabuf: &smithay::backend::allocator::dmabuf::Dmabuf) -> bool {
        self.renderer.import_dmabuf(dmabuf, None).is_ok()
    }

    /// Tap to click and natural scrolling on every device that has them.
    pub fn configure_devices(&mut self, input: &crate::config::Input) {
        for d in &mut self.devices {
            if d.config_tap_finger_count() > 0 {
                let _ = d.config_tap_set_enabled(input.tap);
            }
            if d.config_scroll_has_natural_scroll() {
                let _ = d.config_scroll_set_natural_scroll_enabled(input.natural_scroll);
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TtyError {
    #[error("no session (run from a text console, with logind or seatd): {0}")]
    Session(String),
    #[error("no GPU found")]
    NoGpu,
    #[error("{0}")]
    Device(String),
}

fn err<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> TtyError + '_ {
    move |e| TtyError::Device(format!("{what}: {e}"))
}

pub fn init_tty(event_loop: &mut EventLoop<'static, Draftpaper>, data: &mut Draftpaper) -> Result<(), TtyError> {
    let (session, session_notifier) = LibSeatSession::new().map_err(|e| TtyError::Session(e.to_string()))?;
    let seat = session.seat();

    let path: PathBuf = primary_gpu(&seat)
        .ok()
        .flatten()
        .or_else(|| all_gpus(&seat).ok()?.into_iter().next())
        .ok_or(TtyError::NoGpu)?;
    let node = DrmNode::from_path(&path).map_err(err("drm node"))?;
    eprintln!("draftpaper: GPU {}", path.display());

    let fd = session
        .clone()
        .open(&path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)
        .map_err(err("open gpu"))?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, drm_notifier) = DrmDevice::new(fd.clone(), true).map_err(err("drm device"))?;
    let gbm = GbmDevice::new(fd).map_err(err("gbm device"))?;
    let egl = unsafe { EGLDisplay::new(gbm.clone()) }.map_err(err("egl display"))?;
    let context = EGLContext::new(&egl).map_err(err("egl context"))?;
    let renderer = unsafe { GlesRenderer::new(context) }.map_err(err("gles renderer"))?;

    let render_formats = renderer.dmabuf_formats();

    // linux-dmabuf, so clients render on this GPU (its render node, the formats the renderer imports)
    let render_node = node.node_with_type(smithay::backend::drm::NodeType::Render).and_then(Result::ok).unwrap_or(node);
    let feedback = smithay::wayland::dmabuf::DmabufFeedbackBuilder::new(render_node.dev_id(), render_formats.clone())
        .build()
        .map_err(err("dmabuf feedback"))?;
    let global = data.dmabuf_state.create_global_with_default_feedback::<Draftpaper>(&data.display_handle, &feedback);
    data.dmabuf_global = Some(global);
    let manager = DrmOutputManager::new(
        drm,
        GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT),
        GbmFramebufferExporter::new(gbm.clone(), Some(node)),
        Some(gbm),
        [Fourcc::Abgr8888, Fourcc::Argb8888, Fourcc::Xbgr8888, Fourcc::Xrgb8888],
        render_formats,
    );
    data.shm_state.update_formats(renderer.shm_formats());

    let mut libinput = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput.udev_assign_seat(&seat).map_err(|_| TtyError::Device("libinput seat".into()))?;

    data.session = Some(session.clone());
    data.tty = true;
    data.tty_data = Some(TtyData { session, renderer, manager, node, outputs: HashMap::new(), cursor: Cursor::load(), libinput: libinput.clone(), devices: Vec::new() });

    let handle = event_loop.handle();
    handle
        .insert_source(LibinputInputBackend::new(libinput), |mut event, _, data| {
            if let smithay::backend::input::InputEvent::DeviceAdded { device } = &mut event {
                let input = data.config.input.clone();
                if let Some(tty) = data.tty_data.as_mut() {
                    tty.devices.push(device.clone());
                    tty.configure_devices(&input);
                }
            }
            data.process_input_event(event)
        })
        .map_err(|e| TtyError::Device(e.to_string()))?;
    handle
        .insert_source(drm_notifier, move |event, _, data| match event {
            DrmEvent::VBlank(crtc) => {
                if let Some((_, surface, _)) = data.tty_data.as_mut().and_then(|t| t.outputs.get_mut(&crtc)) {
                    let _ = surface.frame_submitted();
                }
                render(data, crtc);
            }
            DrmEvent::Error(e) => eprintln!("draftpaper: drm error {e:?}"),
        })
        .map_err(|e| TtyError::Device(e.to_string()))?;
    handle
        .insert_source(session_notifier, |event, _, data| {
            let Some(tty) = data.tty_data.as_mut() else { return };
            match event {
                SessionEvent::PauseSession => {
                    tty.libinput.suspend();
                    tty.manager.pause();
                }
                SessionEvent::ActivateSession => {
                    let _ = tty.libinput.resume();
                    let _ = tty.manager.activate(false);
                    let crtcs: Vec<_> = tty.outputs.keys().copied().collect();
                    for crtc in crtcs {
                        render(data, crtc);
                    }
                }
            }
        })
        .map_err(|e| TtyError::Device(e.to_string()))?;

    // displays plugged in or out: scan again
    let udev = UdevBackend::new(&seat).map_err(err("udev"))?;
    handle
        .insert_source(udev, |event, _, data| {
            if let UdevEvent::Changed { device_id } = event {
                if data.tty_data.as_ref().is_some_and(|t| t.node.dev_id() == device_id) {
                    scan_outputs(data);
                }
            }
        })
        .map_err(|e| TtyError::Device(e.to_string()))?;

    scan_outputs(data);
    if data.tty_data.as_ref().unwrap().outputs.is_empty() {
        return Err(TtyError::Device("no connected display".into()));
    }
    Ok(())
}

/// Bring the outputs in line with the connected displays: new ones get an output (at its preferred mode,
/// right of the others unless config.toml places it) and a screen; gone ones lose theirs.
fn scan_outputs(data: &mut Draftpaper) {
    // the TTY data is taken out of the state while outputs and screens change together
    let Some(mut tty_data) = data.tty_data.take() else { return };
    let started = scan_with(data, &mut tty_data);
    data.tty_data = Some(tty_data);
    data.relayout();
    for crtc in started {
        render(data, crtc);
    }
}

fn scan_with(data: &mut Draftpaper, tty: &mut TtyData) -> Vec<crtc::Handle> {
    let theme = Theme::load();
    let device = tty.manager.device();
    let Ok(resources) = device.resource_handles() else { return Vec::new() };

    let mut connected = Vec::new();
    for &handle in resources.connectors() {
        if let Ok(info) = device.get_connector(handle, true) {
            if info.state() == connector::State::Connected && !info.modes().is_empty() {
                connected.push(info);
            }
        }
    }

    // gone
    let gone: Vec<crtc::Handle> = tty
        .outputs
        .iter()
        .filter(|(_, (_, _, conn))| !connected.iter().any(|i| i.handle() == *conn))
        .map(|(crtc, _)| *crtc)
        .collect();
    for crtc in gone {
        if let Some((output, _surface, _)) = tty.outputs.remove(&crtc) {
            eprintln!("draftpaper: output {} gone", output.name());
            data.space.unmap_output(&output);
            data.remove_screen(&output);
        }
    }

    // new
    let mut started = Vec::new();
    for info in connected {
        if tty.outputs.values().any(|(_, _, c)| *c == info.handle()) {
            continue;
        }
        let device = tty.manager.device();
        let used: Vec<crtc::Handle> = tty.outputs.keys().copied().collect();
        let crtc = info
            .encoders()
            .iter()
            .filter_map(|e| device.get_encoder(*e).ok())
            .flat_map(|e| resources.filter_crtcs(e.possible_crtcs()))
            .find(|c| !used.contains(c));
        let Some(crtc) = crtc else { continue };

        let mode = *info.modes().iter().find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED)).unwrap_or(&info.modes()[0]);
        let (w, h) = (mode.size().0 as i32, mode.size().1 as i32);
        let (phys_w, phys_h) = info.size().unwrap_or((0, 0));
        let name = format!("{}-{}", info.interface().as_str(), info.interface_id());
        let settings = output_settings(&name);
        let mm = settings.mm.unwrap_or(if phys_w > 0 { w as f32 / phys_w as f32 } else { FALLBACK_MM });
        let scale = settings.scale.unwrap_or(1.0);
        let position = settings.position.unwrap_or_else(|| {
            let right = data.space.outputs().filter_map(|o| data.space.output_geometry(o)).map(|g| g.loc.x + g.size.w).max().unwrap_or(0);
            (right, 0)
        });
        eprintln!("draftpaper: output {name} {w}x{h} ({phys_w}x{phys_h} mm, {mm:.2} px/mm), scale {scale}, at {position:?}");

        let output = Output::new(
            name,
            PhysicalProperties { size: (phys_w as i32, phys_h as i32).into(), subpixel: Subpixel::Unknown, make: "Unknown".into(), model: "Unknown".into() },
        );
        let _global = output.create_global::<Draftpaper>(&data.display_handle);
        let wl_mode = WlMode::from(mode);
        output.set_preferred(wl_mode);
        output.change_current_state(Some(wl_mode), None, Some(Scale::Fractional(scale)), Some(position.into()));
        data.space.map_output(&output, position);
        attach_sheet(&output, theme.clone(), mm);

        match tty.manager.initialize_output::<_, OutputElements>(
            crtc,
            mode,
            &[info.handle()],
            &output,
            None,
            &mut tty.renderer,
            &DrmOutputRenderElements::default(),
        ) {
            Ok(surface) => {
                data.add_screen(output.clone(), mm as f64, theme.margin_mm as f64);
                tty.outputs.insert(crtc, (output, surface, info.handle()));
                started.push(crtc);
            }
            Err(e) => {
                eprintln!("draftpaper: output {}: {e:?}", output.name());
                data.space.unmap_output(&output);
            }
        }
    }
    started
}

/// Render one output; on an unchanged frame try again a refresh later.
fn render(data: &mut Draftpaper, crtc: crtc::Handle) {
    let Some(mut tty) = data.tty_data.take() else { return };
    let queued = render_with(data, &mut tty, crtc);
    data.tty_data = Some(tty);
    let Some(queued) = queued else { return };
    let Some(output) = data.space.outputs().find(|o| data.tty_data.as_ref().is_some_and(|t| t.outputs.get(&crtc).is_some_and(|(out, _, _)| out == *o))).cloned() else { return };
    after_frame(data, &output, queued, crtc);
}

/// Draw and queue a frame; None when nothing can be drawn (session inactive, unknown crtc).
fn render_with(data: &mut Draftpaper, tty: &mut TtyData, crtc: crtc::Handle) -> Option<bool> {
    if !tty.session.is_active() {
        return None;
    }
    let (output, surface, _) = tty.outputs.get_mut(&crtc)?;
    let output = output.clone();
    let elements = output_elements(data, &mut tty.renderer, &output, Some((&tty.cursor.buffer, tty.cursor.hotspot)));
    let queued = match surface.render_frame(&mut tty.renderer, &elements, [0.1, 0.1, 0.1, 1.0], FrameFlags::DEFAULT) {
        Ok(result) if !result.is_empty => surface.queue_frame(()).is_ok(),
        Ok(_) => false,
        Err(e) => {
            eprintln!("draftpaper: render {e:?}");
            false
        }
    };
    Some(queued)
}

/// After a frame: frame callbacks, housekeeping, and on an unchanged frame a retry a refresh later.
fn after_frame(data: &mut Draftpaper, output: &smithay::output::Output, queued: bool, crtc: crtc::Handle) {
    let output = output.clone();

    let now = data.start_time.elapsed();
    data.send_frames(&output, now);
    data.space.refresh();
    data.popups.cleanup();
    let _ = data.display_handle.flush_clients();

    if !queued {
        let refresh = output.current_mode().map(|m| m.refresh).unwrap_or(60_000).max(1);
        let delay = Duration::from_micros(1_000_000_000 / refresh as u64);
        let handle: LoopHandle<'static, Draftpaper> = data.loop_handle.clone();
        let _ = handle.insert_source(Timer::from_duration(delay), move |_, _, data| {
            render(data, crtc);
            TimeoutAction::Drop
        });
    }
}

/// Every output as drawn (without the pointer), to ~/Pictures/draftpaper-<seconds>-<output>.png.
pub fn screenshot(data: &mut Draftpaper) {
    let Some(mut tty) = data.tty_data.take() else { return };
    let outputs: Vec<Output> = tty.outputs.values().map(|(o, _, _)| o.clone()).collect();
    let dir = PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Pictures");
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    for output in outputs {
        let path = dir.join(format!("draftpaper-{stamp}-{}.png", output.name()));
        match capture(data, &mut tty.renderer, &output, &path) {
            Ok(()) => eprintln!("draftpaper: screenshot {}", path.display()),
            Err(e) => eprintln!("draftpaper: screenshot of {}: {e}", output.name()),
        }
    }
    data.tty_data = Some(tty);
}

fn capture(data: &Draftpaper, renderer: &mut GlesRenderer, output: &Output, path: &std::path::Path) -> Result<(), String> {
    use smithay::backend::renderer::{damage::OutputDamageTracker, gles::GlesTexture, Bind, ExportMem, Offscreen};
    use smithay::utils::{Rectangle, Size};
    let mode = output.current_mode().ok_or("no mode")?;
    let size: Size<i32, smithay::utils::Buffer> = (mode.size.w, mode.size.h).into();
    let elements = output_elements(data, renderer, output, None);
    let mut texture: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, size).map_err(|e| format!("{e:?}"))?;
    let pixels = {
        let mut target = renderer.bind(&mut texture).map_err(|e| format!("{e:?}"))?;
        let mut damage = OutputDamageTracker::from_output(output);
        damage.render_output(renderer, &mut target, 0, &elements, [0.1, 0.1, 0.1, 1.0]).map_err(|e| format!("{e:?}"))?;
        let mapping = renderer.copy_framebuffer(&target, Rectangle::from_size(size), Fourcc::Abgr8888).map_err(|e| format!("{e:?}"))?;
        renderer.map_texture(&mapping).map_err(|e| format!("{e:?}"))?.to_vec()
    };
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.w as u32, size.h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    // opaque: the alpha the renderer leaves is not the picture's
    let mut rgba = pixels;
    rgba.chunks_exact_mut(4).for_each(|p| p[3] = 255);
    encoder.write_header().and_then(|mut w| w.write_image_data(&rgba)).map_err(|e| e.to_string())
}
