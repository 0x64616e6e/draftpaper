//! The TTY backend: Draftpaper on the real screens, started from a text console. A libseat session
//! (logind or seatd) opens the devices; the primary GPU renders with GLES and drives every connected
//! display at its preferred mode through Smithay's DRM compositor; libinput reads keyboards, touchpads
//! and the TrackPoint. One GPU only. Displays plugged in or out later are picked up from udev; each has
//! its own screen (screens.rs), with scale and position from config.toml ([outputs.<name>]). Tools
//! that speak wlr-output-management (kanshi, wlr-randr) change modes, positions and scales, and turn
//! displays off and on again (configure; output_management.rs).
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

/// An output's wl_output global, removed when the output goes (clients then stop naming it).
struct OutputGlobal(smithay::reexports::wayland_server::backend::GlobalId);

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
    /// displays connected but turned off through output management (by name); the scan leaves them be
    disabled: std::collections::HashSet<String>,
    /// taking the displays over as they were failed, and they were reset (reset_once)
    reset: bool,
    /// the outputs that have had a frame queued (logged once, with the time since the start: handoff.rs)
    drawn: std::collections::HashSet<crtc::Handle>,
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
    // the displays as they are, not reset: a display showing the greeter's last frame (or the firmware's
    // picture) in the mode Draftpaper wants is taken over without going dark (adopt)
    let (drm, drm_notifier) = DrmDevice::new(fd.clone(), false).map_err(err("drm device"))?;
    log_screens(&drm, if drm.is_atomic() { "device opened (atomic)" } else { "device opened (legacy: tests are commits)" });
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
    data.tty_data = Some(TtyData { session, renderer, manager, node, outputs: HashMap::new(), cursor: Cursor::load(), libinput: libinput.clone(), devices: Vec::new(), disabled: Default::default(), reset: false, drawn: Default::default() });

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
                    eprintln!("draftpaper: session paused after {} ms", data.start_time.elapsed().as_millis());
                    tty.libinput.suspend();
                    tty.manager.pause();
                }
                SessionEvent::ActivateSession => {
                    eprintln!("draftpaper: session active after {} ms", data.start_time.elapsed().as_millis());
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

    // the outputs, set up but not drawn yet: what the previous owner left beside them goes first
    let Some(mut tty_data) = data.tty_data.take() else { return Ok(()) };
    let started = scan_with(data, &mut tty_data);
    log_screens(tty_data.manager.device(), "outputs set up");
    take_pictures(&mut tty_data);
    adopt(&tty_data);
    log_screens(tty_data.manager.device(), "the rest switched off");
    data.tty_data = Some(tty_data);
    data.relayout();
    for crtc in started {
        render(data, crtc);
    }
    data.refresh_output_management();
    if data.tty_data.as_ref().unwrap().outputs.is_empty() {
        return Err(TtyError::Device("no connected display".into()));
    }
    Ok(())
}

/// The device was not reset (DrmDevice::new above): switch off what the previous owner (the greeter,
/// another compositor, the console) left on that Draftpaper does not draw: displays it does not drive,
/// and on its own CRTCs every plane but the primary (a pointer, an overlay), which its first frames would
/// not touch. The primary planes stay: their pictures stay up until the first frame replaces them.
fn adopt(tty: &TtyData) {
    use smithay::reexports::drm::control::{atomic::AtomicModeReq, property, AtomicCommitFlags};
    let device = tty.manager.device();
    if !device.is_atomic() {
        return;
    }
    let (Ok(resources), Ok(planes)) = (device.resource_handles(), device.plane_handles()) else { return };
    let ours: HashMap<crtc::Handle, smithay::reexports::drm::control::plane::Handle> =
        tty.outputs.iter().map(|(c, (_, s, _))| (*c, s.with_compositor(|c| c.surface().plane()))).collect();
    // a property of a DRM object, by name
    fn prop<T: smithay::reexports::drm::control::ResourceHandle>(device: &DrmDevice, handle: T, name: &str) -> Option<property::Handle> {
        let props = device.get_properties(handle).ok()?;
        let (ids, _) = props.as_props_and_values();
        ids.iter().copied().find(|&id| device.get_property(id).is_ok_and(|p| p.name().to_str() == Ok(name)))
    }
    let mut req = AtomicModeReq::new();
    let mut what = Vec::new();
    for plane in planes {
        let Ok(info) = device.get_plane(plane) else { continue };
        let Some(crtc) = info.crtc() else { continue };
        if ours.get(&crtc) == Some(&plane) {
            continue;
        }
        if let (Some(c), Some(f)) = (prop(device, plane, "CRTC_ID"), prop(device, plane, "FB_ID")) {
            req.add_property(plane, c, property::Value::CRTC(None));
            req.add_property(plane, f, property::Value::Framebuffer(None));
            what.push(format!("plane {}", u32::from(plane)));
        }
    }
    for &conn in resources.connectors() {
        let Ok(info) = device.get_connector(conn, false) else { continue };
        let crtc = info.current_encoder().and_then(|e| device.get_encoder(e).ok()).and_then(|e| e.crtc());
        if crtc.is_some_and(|c| !ours.contains_key(&c)) {
            if let Some(p) = prop(device, conn, "CRTC_ID") {
                req.add_property(conn, p, property::Value::CRTC(None));
                what.push(connector_name(&info));
            }
        }
    }
    for &crtc in resources.crtcs() {
        if ours.contains_key(&crtc) || device.get_crtc(crtc).ok().and_then(|c| c.mode()).is_none() {
            continue;
        }
        if let (Some(a), Some(m)) = (prop(device, crtc, "ACTIVE"), prop(device, crtc, "MODE_ID")) {
            req.add_property(crtc, a, property::Value::Boolean(false));
            req.add_property(crtc, m, property::Value::Unknown(0));
            what.push(format!("crtc {}", u32::from(crtc)));
        }
    }
    if what.is_empty() {
        return;
    }
    match device.atomic_commit(AtomicCommitFlags::ALLOW_MODESET, req) {
        Ok(()) => eprintln!("draftpaper: switched off what was left on: {}", what.join(", ")),
        Err(e) => eprintln!("draftpaper: switching off {}: {e}", what.join(", ")),
    }
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
    data.refresh_output_management();
}

/// "eDP-1", "DP-3": the connector's name as the kernel and other compositors give it.
fn connector_name(info: &connector::Info) -> String {
    format!("{}-{}", info.interface().as_str(), info.interface_id())
}

/// The displays connected now (with at least one mode).
fn connected(tty: &TtyData) -> Vec<connector::Info> {
    let device = tty.manager.device();
    let Ok(resources) = device.resource_handles() else { return Vec::new() };
    resources
        .connectors()
        .iter()
        .filter_map(|&h| device.get_connector(h, false).ok())
        .filter(|i| i.state() == connector::State::Connected && !i.modes().is_empty())
        .collect()
}

/// Withdraw an output: its global, its place in the space, its screen (its windows move to another).
fn remove_output(data: &mut Draftpaper, tty: &mut TtyData, crtc: crtc::Handle) {
    if let Some((output, _surface, _)) = tty.outputs.remove(&crtc) {
        if let Some(global) = output.user_data().get::<OutputGlobal>() {
            data.display_handle.remove_global::<Draftpaper>(global.0.clone());
        }
        data.space.unmap_output(&output);
        data.remove_screen(&output);
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
    // a display turned off and then unplugged is forgotten; and if every display left is off (the laptop
    // panel off while docked, then undocked), they come on again: there is always a screen
    tty.disabled.retain(|n| connected.iter().any(|i| connector_name(i) == *n));
    if connected.iter().all(|i| tty.disabled.contains(&connector_name(i))) {
        tty.disabled.clear();
    }
    connected.retain(|i| !tty.disabled.contains(&connector_name(i)));

    // gone
    let gone: Vec<crtc::Handle> = tty
        .outputs
        .iter()
        .filter(|(_, (_, _, conn))| !connected.iter().any(|i| i.handle() == *conn))
        .map(|(crtc, _)| *crtc)
        .collect();
    for crtc in gone {
        if let Some((output, _, _)) = tty.outputs.get(&crtc) {
            eprintln!("draftpaper: output {} gone", output.name());
        }
        remove_output(data, tty, crtc);
    }

    // new
    let mut started = Vec::new();
    for info in connected {
        if tty.outputs.values().any(|(_, _, c)| *c == info.handle()) {
            continue;
        }
        let device = tty.manager.device();
        let used: Vec<crtc::Handle> = tty.outputs.keys().copied().collect();
        // the CRTC the display is on already (the greeter's, the firmware's), so it is taken over as it
        // is; else the first free one
        let current = info.current_encoder().and_then(|e| device.get_encoder(e).ok()).and_then(|e| e.crtc()).filter(|c| !used.contains(c));
        let crtc = current.or_else(|| {
            info.encoders()
                .iter()
                .filter_map(|e| device.get_encoder(*e).ok())
                .flat_map(|e| resources.filter_crtcs(e.possible_crtcs()))
                .find(|c| !used.contains(c))
        });
        let Some(crtc) = crtc else { continue };

        let mode = *info.modes().iter().find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED)).unwrap_or(&info.modes()[0]);
        // the mode it shows already: then the first frame goes out without a modeset (no flash)
        let adopted = current == Some(crtc) && device.get_crtc(crtc).ok().and_then(|c| c.mode()) == Some(mode);
        let (w, h) = (mode.size().0 as i32, mode.size().1 as i32);
        let (phys_w, phys_h) = info.size().unwrap_or((0, 0));
        let name = connector_name(&info);
        let edid = Edid::read(device, info.handle());
        let settings = output_settings(&name);
        let mm = settings.mm.unwrap_or(if phys_w > 0 { w as f32 / phys_w as f32 } else { FALLBACK_MM });
        let scale = settings.scale.unwrap_or(1.0);
        let position = settings.position.unwrap_or_else(|| {
            let right = data.space.outputs().filter_map(|o| data.space.output_geometry(o)).map(|g| g.loc.x + g.size.w).max().unwrap_or(0);
            (right, 0)
        });
        eprintln!(
            "draftpaper: output {name} {w}x{h} ({phys_w}x{phys_h} mm, {mm:.2} px/mm), scale {scale}, at {position:?}, {}",
            if adopted { "taken over as it is" } else { "mode set" }
        );

        let output = Output::new(
            name,
            PhysicalProperties {
                size: (phys_w as i32, phys_h as i32).into(),
                subpixel: Subpixel::Unknown,
                make: if edid.make.is_empty() { "Unknown".into() } else { edid.make.clone() },
                model: if edid.model.is_empty() { "Unknown".into() } else { edid.model.clone() },
            },
        );
        // kept with the output, to withdraw it from clients when the output goes
        let global = output.create_global::<Draftpaper>(&data.display_handle);
        output.user_data().insert_if_missing(|| OutputGlobal(global));
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
                log_screens(tty.manager.device(), &format!("{} set up", output.name()));
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
    data.anim.prune(std::time::Instant::now());
    if !data.screencopies.is_empty() {
        if let Some(mut tty) = data.tty_data.take() {
            let pointer = cursor(&tty);
            let pointer = (pointer.0.clone(), pointer.1);
            crate::screencopy::finish(data, &mut tty.renderer, Some((&pointer.0, pointer.1)));
            data.tty_data = Some(tty);
        }
    }
    let Some(mut tty) = data.tty_data.take() else { return };
    let queued = render_with(data, &mut tty, crtc);
    data.tty_data = Some(tty);
    let Some(queued) = queued else { return };
    let Some(output) = data.space.outputs().find(|o| data.tty_data.as_ref().is_some_and(|t| t.outputs.get(&crtc).is_some_and(|(out, _, _)| out == *o))).cloned() else { return };
    after_frame(data, &output, queued, crtc);
}

/// Draw and queue a frame; None when nothing can be drawn (session inactive, unknown crtc).
fn render_with(data: &mut Draftpaper, tty: &mut TtyData, crtc: crtc::Handle) -> Option<bool> {
    // inactive (another VT) or the displays off: no frames until set_power or the session comes back
    if !tty.session.is_active() || data.screens_off {
        return None;
    }
    let (output, surface, _) = tty.outputs.get_mut(&crtc)?;
    let output = output.clone();
    // leaving (handoff.rs): no pointer
    let pointer = data.leaving.is_none().then_some((&tty.cursor.buffer, tty.cursor.hotspot));
    let elements = output_elements(data, &mut tty.renderer, &output, pointer);
    let queued = match surface.render_frame(&mut tty.renderer, &elements, [0.1, 0.1, 0.1, 1.0], FrameFlags::DEFAULT) {
        Ok(result) if !result.is_empty => match surface.queue_frame(()) {
            Ok(()) => {
                if tty.drawn.insert(crtc) {
                    eprintln!("draftpaper: {}: first frame after {} ms", output.name(), data.start_time.elapsed().as_millis());
                }
                true
            }
            Err(e) => {
                eprintln!("draftpaper: frame {e:?}");
                reset_once(tty, &format!("{e:?}"));
                false
            }
        },
        Ok(_) => false,
        Err(e) => {
            eprintln!("draftpaper: render {e:?}");
            reset_once(tty, &format!("{e:?}"));
            false
        }
    };
    // leaving: nothing new to draw, so the bare sheet is what the display shows
    if let (Some(leaving), false) = (data.leaving.as_mut(), queued) {
        leaving.remove(&crtc);
        return None;
    }
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

/// ~/Pictures/draftpaper-<seconds>-<output>.png
pub fn screenshot_path(output: &Output) -> PathBuf {
    let dir = PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Pictures");
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    dir.join(format!("draftpaper-{stamp}-{}.png", output.name()))
}

/// Every output as drawn (without the pointer), to ~/Pictures/draftpaper-<seconds>-<output>.png. Nested,
/// the next frame is taken (winit.rs).
pub fn screenshot(data: &mut Draftpaper) {
    let Some(mut tty) = data.tty_data.take() else {
        data.screenshot_requested = true;
        return;
    };
    let outputs: Vec<Output> = tty.outputs.values().map(|(o, _, _)| o.clone()).collect();
    for output in outputs {
        let path = screenshot_path(&output);
        match capture(data, &mut tty.renderer, &output, &path) {
            Ok(()) => eprintln!("draftpaper: screenshot {}", path.display()),
            Err(e) => eprintln!("draftpaper: screenshot of {}: {e}", output.name()),
        }
    }
    data.tty_data = Some(tty);
}

/// `output` as drawn, offscreen: RGBA bytes, row by row, with its size in pixels. Upright whatever the
/// output's transform (the Print screenshot), or as the output scans out (`upright` false: screencopy,
/// whose clients apply the transform they are told). `pointer` adds the cursor.
pub fn render_pixels(
    data: &Draftpaper,
    renderer: &mut GlesRenderer,
    output: &Output,
    pointer: Option<(&smithay::backend::renderer::element::memory::MemoryRenderBuffer, smithay::utils::Point<i32, smithay::utils::Logical>)>,
    upright: bool,
) -> Result<(Vec<u8>, i32, i32), String> {
    use smithay::backend::renderer::{damage::OutputDamageTracker, gles::GlesTexture, Bind, ExportMem, Offscreen};
    use smithay::utils::{Rectangle, Size};
    let mode = output.current_mode().ok_or("no mode")?;
    let size: Size<i32, smithay::utils::Buffer> = (mode.size.w, mode.size.h).into();
    let elements = output_elements(data, renderer, output, pointer);
    let mut texture: GlesTexture = renderer.create_buffer(Fourcc::Abgr8888, size).map_err(|e| format!("{e:?}"))?;
    let mut target = renderer.bind(&mut texture).map_err(|e| format!("{e:?}"))?;
    let transform = if upright { smithay::utils::Transform::Normal } else { output.current_transform() };
    let mut damage = OutputDamageTracker::new((size.w, size.h), output.current_scale().fractional_scale(), transform);
    damage.render_output(renderer, &mut target, 0, &elements, [0.1, 0.1, 0.1, 1.0]).map_err(|e| format!("{e:?}"))?;
    let mapping = renderer.copy_framebuffer(&target, Rectangle::from_size(size), Fourcc::Abgr8888).map_err(|e| format!("{e:?}"))?;
    let pixels = renderer.map_texture(&mapping).map_err(|e| format!("{e:?}"))?.to_vec();
    Ok((pixels, size.w, size.h))
}

/// The pointer image of the TTY backend, for a screencopy that wants it.
pub fn cursor(tty: &TtyData) -> (&smithay::backend::renderer::element::memory::MemoryRenderBuffer, smithay::utils::Point<i32, smithay::utils::Logical>) {
    (&tty.cursor.buffer, tty.cursor.hotspot)
}

pub fn capture(data: &Draftpaper, renderer: &mut GlesRenderer, output: &Output, path: &std::path::Path) -> Result<(), String> {
    let (pixels, w, h) = render_pixels(data, renderer, output, None, true)?;
    let size = smithay::utils::Size::<i32, smithay::utils::Buffer>::from((w, h));
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

/// Turn every display off (DPMS: no signal, planes and the pending frame dropped) or on again (the next
/// frame re-enables it). Nested there is nothing to do.
pub fn set_power(data: &mut Draftpaper, on: bool) {
    if data.screens_off == !on {
        return;
    }
    let Some(tty) = data.tty_data.as_mut() else { return };
    data.screens_off = !on;
    let crtcs: Vec<crtc::Handle> = tty.outputs.keys().copied().collect();
    if on {
        for crtc in crtcs {
            render(data, crtc);
        }
    } else {
        for (_, surface, _) in tty.outputs.values() {
            if let Err(e) = surface.with_compositor(|c| c.clear()) {
                eprintln!("draftpaper: power off: {e:?}");
            }
        }
    }
}

/// Make, model and serial number from a display's EDID (what kanshi matches displays by). The make is the
/// three-letter PNP id ("DEL", "BOE"); model and serial are the monitor's name and serial descriptors,
/// or the product code and serial number when it has none.
#[derive(Debug, Clone, Default)]
struct Edid {
    make: String,
    model: String,
    serial: String,
}

impl Edid {
    fn read(device: &smithay::backend::drm::DrmDevice, conn: connector::Handle) -> Edid {
        let blob = (|| {
            let props = device.get_properties(conn).ok()?;
            let (handles, values) = props.as_props_and_values();
            handles.iter().zip(values).find_map(|(&h, &v)| {
                let info = device.get_property(h).ok()?;
                (info.name().to_str().ok()? == "EDID" && v != 0).then(|| device.get_property_blob(v).ok()).flatten()
            })
        })();
        blob.map(|b| Edid::parse(&b)).unwrap_or_default()
    }

    fn parse(b: &[u8]) -> Edid {
        if b.len() < 128 || b[0..8] != [0, 255, 255, 255, 255, 255, 255, 0] {
            return Edid::default();
        }
        let id = u16::from_be_bytes([b[8], b[9]]);
        let letter = |v: u16| char::from(b'@' + (v & 31) as u8);
        let make: String = [letter(id >> 10), letter(id >> 5), letter(id)].iter().collect();
        let product = u16::from_le_bytes([b[10], b[11]]);
        let number = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);
        // display descriptors: 18 bytes each from 54; tag 0xFC the name, 0xFF the serial
        let text = |tag: u8| {
            (0..4).map(|i| &b[54 + i * 18..72 + i * 18]).find(|d| d[0] == 0 && d[1] == 0 && d[3] == tag).map(|d| {
                let t: String = d[5..].iter().take_while(|&&c| c != 0x0a && c != 0).map(|&c| c as char).collect();
                t.trim().to_string()
            })
        };
        Edid {
            make,
            model: text(0xfc).filter(|t| !t.is_empty()).unwrap_or_else(|| format!("0x{product:04X}")),
            serial: text(0xff).filter(|t| !t.is_empty()).unwrap_or_else(|| if number != 0 { number.to_string() } else { String::new() }),
        }
    }
}

/// The displays for output management: every connected one, on or off.
pub fn heads(data: &Draftpaper) -> Vec<crate::output_management::HeadInfo> {
    use crate::output_management::{HeadInfo, ModeInfo};
    let Some(tty) = data.tty_data.as_ref() else { return Vec::new() };
    let device = tty.manager.device();
    let mut heads: Vec<HeadInfo> = connected(tty)
        .into_iter()
        .map(|info| {
            let name = connector_name(&info);
            let edid = Edid::read(device, info.handle());
            let mut modes: Vec<ModeInfo> = Vec::new();
            for m in info.modes() {
                let wl = WlMode::from(*m);
                let mode = ModeInfo { w: wl.size.w, h: wl.size.h, refresh: wl.refresh, preferred: m.mode_type().contains(ModeTypeFlags::PREFERRED) };
                match modes.iter_mut().find(|x| x.w == mode.w && x.h == mode.h && x.refresh == mode.refresh) {
                    Some(x) => x.preferred |= mode.preferred,
                    None => modes.push(mode),
                }
            }
            let output = tty.outputs.values().find(|(_, _, c)| *c == info.handle()).map(|(o, _, _)| o);
            let (phys_w, phys_h) = info.size().unwrap_or((0, 0));
            let mut head = HeadInfo {
                name,
                make: edid.make,
                model: edid.model,
                serial: edid.serial,
                physical: (phys_w as i32, phys_h as i32),
                modes,
                enabled: output.is_some(),
                current: None,
                position: (0, 0),
                transform: smithay::utils::Transform::Normal,
                scale: 1.0,
            };
            if let Some(output) = output {
                head.current = output.current_mode().and_then(|m| head.modes.iter().find(|x| x.w == m.size.w && x.h == m.size.h && x.refresh == m.refresh).copied());
                let loc = output.current_location();
                head.position = (loc.x, loc.y);
                head.transform = output.current_transform();
                head.scale = output.current_scale().fractional_scale();
            }
            head
        })
        .collect();
    heads.sort_by(|a, b| a.name.cmp(&b.name));
    heads
}

/// Carry out (or, for `test`, only check) an output configuration: displays off, displays on, then
/// each one's mode, position, scale and transform.
pub fn configure(data: &mut Draftpaper, changes: &[crate::output_management::HeadChange], test: bool) -> Result<(), String> {
    let Some(mut tty) = data.tty_data.take() else { return Err("no TTY".into()) };
    let result = configure_with(data, &mut tty, changes, test);
    data.tty_data = Some(tty);
    let (started, changed) = result?;
    for output in changed {
        data.update_screen(&output);
    }
    data.relayout();
    for crtc in started {
        render(data, crtc);
    }
    Ok(())
}

/// The display mode `want` names on this connector: the same size and refresh (a custom mode with no
/// refresh, as kanshi sends `mode 3840x2160`: the display's preferred one at that size, else the fastest).
fn find_mode(info: &connector::Info, want: &crate::output_management::ModeInfo) -> Option<smithay::reexports::drm::control::Mode> {
    let at_size = info.modes().iter().filter(|m| {
        let wl = WlMode::from(**m);
        wl.size.w == want.w && wl.size.h == want.h
    });
    if want.refresh == 0 {
        return at_size.max_by_key(|m| (m.mode_type().contains(ModeTypeFlags::PREFERRED), WlMode::from(**m).refresh)).copied();
    }
    // a custom refresh within 0.5 Hz of the display's own
    at_size.min_by_key(|m| (WlMode::from(**m).refresh - want.refresh).abs()).filter(|m| (WlMode::from(**m).refresh - want.refresh).abs() <= 500).copied()
}

#[allow(clippy::type_complexity)]
fn configure_with(
    data: &mut Draftpaper,
    tty: &mut TtyData,
    changes: &[crate::output_management::HeadChange],
    test: bool,
) -> Result<(Vec<crtc::Handle>, Vec<Output>), String> {
    let connected = connected(tty);
    let info_of = |name: &str| connected.iter().find(|i| connector_name(i) == name);
    // check everything first
    if !changes.iter().any(|c| c.config.is_some()) {
        return Err("every display would be off".into());
    }
    for change in changes {
        let info = info_of(&change.name).ok_or_else(|| format!("{} is not connected", change.name))?;
        let Some(config) = &change.config else { continue };
        if let Some(mode) = &config.mode {
            find_mode(info, mode).ok_or_else(|| format!("{} has no mode {}x{}@{}", change.name, mode.w, mode.h, mode.refresh as f64 / 1000.0))?;
        }
        if let Some(scale) = config.scale {
            if !(0.25..=4.0).contains(&scale) {
                return Err(format!("scale {scale} for {} (0.25 to 4)", change.name));
            }
        }
        if config.adaptive_sync == Some(true) {
            return Err("adaptive sync is not supported".into());
        }
    }
    if test {
        return Ok((Vec::new(), Vec::new()));
    }

    let crtc_of = |tty: &TtyData, name: &str| tty.outputs.iter().find(|(_, (o, _, _))| o.name() == name).map(|(c, _)| *c);
    // off
    for change in changes.iter().filter(|c| c.config.is_none()) {
        if let Some(crtc) = crtc_of(tty, &change.name) {
            eprintln!("draftpaper: output {} off", change.name);
            remove_output(data, tty, crtc);
        }
        tty.disabled.insert(change.name.clone());
    }
    // on: the scan brings them up at their preferred mode
    let mut started = Vec::new();
    if changes.iter().any(|c| c.config.is_some() && tty.disabled.remove(&c.name)) {
        started = scan_with(data, tty);
    }
    // each one's settings
    let mut changed = Vec::new();
    for change in changes {
        let Some(config) = &change.config else { continue };
        let crtc = crtc_of(tty, &change.name).ok_or_else(|| format!("{} could not be turned on", change.name))?;
        let info = info_of(&change.name).unwrap();
        let (output, surface, _) = tty.outputs.get_mut(&crtc).unwrap();
        let output = output.clone();
        if let Some(want) = &config.mode {
            let mode = find_mode(info, want).unwrap();
            let wl = WlMode::from(mode);
            if output.current_mode() != Some(wl) {
                surface.use_mode(mode, &mut tty.renderer, &DrmOutputRenderElements::<GlesRenderer, OutputElements>::default()).map_err(|e| format!("{}: mode: {e:?}", change.name))?;
                output.change_current_state(Some(wl), None, None, None);
                // millimetres in the new mode's pixels (unless config.toml fixes them)
                let (phys_w, _) = info.size().unwrap_or((0, 0));
                let mm = output_settings(&change.name).mm.unwrap_or(if phys_w > 0 { wl.size.w as f32 / phys_w as f32 } else { FALLBACK_MM });
                if let Some(sheet) = output.user_data().get::<crate::render::OutputSheet>() {
                    sheet.0.borrow_mut().mm = mm;
                }
                if let Some(screen) = data.screens.iter_mut().find(|s| s.output == output) {
                    screen.phys_mm = mm as f64;
                }
                eprintln!("draftpaper: output {} mode {}x{}@{:.3}", change.name, wl.size.w, wl.size.h, wl.refresh as f64 / 1000.0);
            }
        }
        let scale = config.scale.map(Scale::Fractional);
        let position = config.position.map(smithay::utils::Point::from);
        output.change_current_state(None, config.transform, scale, position);
        if let Some(p) = position {
            data.space.map_output(&output, p);
        } else {
            // the size may have changed: map it again where it is
            data.space.map_output(&output, output.current_location());
        }
        changed.push(output);
        if !started.contains(&crtc) {
            started.push(crtc);
        }
    }
    Ok((started, changed))
}

/// Render one output now (handoff.rs).
pub fn render_crtc(data: &mut Draftpaper, crtc: crtc::Handle) {
    render(data, crtc);
}

/// The framebuffer each output's primary plane shows (handoff.rs).
pub fn shown(tty: &TtyData) -> Vec<(crtc::Handle, smithay::reexports::drm::control::framebuffer::Handle)> {
    let device = tty.manager.device();
    tty.outputs
        .iter()
        .filter_map(|(crtc, (_, surface, _))| {
            let plane = surface.with_compositor(|c| c.surface().plane());
            Some((*crtc, device.get_plane(plane).ok()?.framebuffer()?))
        })
        .collect()
}

pub fn device(tty: &TtyData) -> &DrmDevice {
    tty.manager.device()
}

/// The GPU's device file (/dev/dri/card1).
pub fn device_path(tty: &TtyData) -> PathBuf {
    tty.node.dev_path().unwrap_or_default()
}

/// Every output as drawn now, to DIR/last-<output>.png (DRAFTPAPER_LAST_FRAME, handoff.rs).
pub fn capture_into(data: &mut Draftpaper, dir: &std::path::Path) {
    let Some(mut tty) = data.tty_data.take() else { return };
    let outputs: Vec<Output> = tty.outputs.values().map(|(o, _, _)| o.clone()).collect();
    for output in outputs {
        let path = dir.join(format!("last-{}.png", output.name()));
        if let Err(e) = capture(data, &mut tty.renderer, &output, &path) {
            eprintln!("draftpaper: last frame of {}: {e}", output.name());
        }
    }
    data.tty_data = Some(tty);
}

/// A first frame the driver refused, likely on the state the previous owner left (adopt): reset every display,
/// as Draftpaper did before it took them over as they were, once; the next frames set their modes anew.
/// Not when the frame was refused for want of the device (not its master yet: the previous owner's
/// session still ending, or another VT): that passes, and a reset then would darken the displays.
fn reset_once(tty: &mut TtyData, why: &str) {
    let master = why.contains("PermissionDenied") || why.contains("DeviceInactive") || why.contains("os error 13") || why.contains("os error 1)");
    // only while taking over: once a frame is on screen, the state is Draftpaper's own, and a refused
    // frame (a client's buffer a plane would not take) is dropped, not answered by darkening the displays
    if master || !tty.session.is_active() || !tty.drawn.is_empty() {
        return;
    }
    if std::mem::replace(&mut tty.reset, true) {
        return;
    }
    eprintln!("draftpaper: resetting the displays");
    if let Err(e) = tty.manager.device_mut().reset_state() {
        eprintln!("draftpaper: reset: {e}");
    }
    for (_, surface, _) in tty.outputs.values_mut() {
        if let Err(e) = surface.with_compositor(|c| c.reset_state()) {
            eprintln!("draftpaper: reset: {e:?}");
        }
    }
}

/// What each display shows before Draftpaper draws over it (the login screen's last frame), for the
/// opening (opening.rs).
fn take_pictures(tty: &mut TtyData) {
    let device = tty.manager.device();
    for (crtc, (output, _, _)) in &tty.outputs {
        let Some(fb) = device.get_crtc(*crtc).ok().and_then(|c| c.framebuffer()) else { continue };
        if let Some((texture, size)) = crate::opening::from_framebuffer(device, &mut tty.renderer, fb) {
            if crate::opening::is_black(&mut tty.renderer, &texture, size) {
                eprintln!("draftpaper: {}: the picture on it is black: no opening", output.name());
                continue;
            }
            eprintln!("draftpaper: {}: opening on the picture it showed ({}x{})", output.name(), size.0, size.1);
            crate::opening::start(output, texture, size);
        }
    }
}

/// What each CRTC shows (its primary plane's framebuffer, 0: nothing), with when: following the
/// handoff in the log.
fn log_screens(device: &DrmDevice, when: &str) {
    let Ok(resources) = device.resource_handles() else { return };
    let shown: Vec<String> = resources
        .crtcs()
        .iter()
        .filter_map(|c| device.get_crtc(*c).ok())
        .filter(|i| i.mode().is_some())
        .map(|i| format!("crtc {} framebuffer {}", u32::from(i.handle()), i.framebuffer().map(u32::from).unwrap_or(0)))
        .collect();
    eprintln!("draftpaper: {when}: {}", if shown.is_empty() { "no CRTC on".into() } else { shown.join(", ") });
}
