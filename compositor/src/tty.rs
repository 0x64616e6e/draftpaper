//! The TTY backend: Draftpaper on the real screens, started from a text console. A libseat session
//! (logind or seatd) opens the devices; the primary GPU renders with GLES and drives every connected
//! display at its preferred mode through Smithay's DRM compositor; libinput reads keyboards, touchpads
//! and the TrackPoint. One GPU only; displays connected later are picked up in #14.
//!
//! Follows the structure of anvil's udev.rs (Smithay v0.7.0, MIT), without multi-GPU, leasing and
//! dmabuf feedback.

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
        udev::{all_gpus, primary_gpu},
    },
    output::{Mode as WlMode, Output, PhysicalProperties, Subpixel},
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
    layout::Grid,
    render::{attach_sheet, output_elements, OutputElements},
    theme::Theme,
    CalloopData,
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
    outputs: HashMap<crtc::Handle, (Output, Surface)>,
    cursor: Cursor,
    libinput: Libinput,
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

pub fn init_tty(event_loop: &mut EventLoop<CalloopData>, data: &mut CalloopData) -> Result<(), TtyError> {
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
    let manager = DrmOutputManager::new(
        drm,
        GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT),
        GbmFramebufferExporter::new(gbm.clone(), Some(node)),
        Some(gbm),
        [Fourcc::Abgr8888, Fourcc::Argb8888, Fourcc::Xbgr8888, Fourcc::Xrgb8888],
        render_formats,
    );
    data.state.shm_state.update_formats(renderer.shm_formats());

    let mut libinput = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput.udev_assign_seat(&seat).map_err(|_| TtyError::Device("libinput seat".into()))?;

    data.state.session = Some(session.clone());
    data.state.tty = true;
    data.tty = Some(TtyData { session, renderer, manager, outputs: HashMap::new(), cursor: Cursor::load(), libinput: libinput.clone() });

    let handle = event_loop.handle();
    handle
        .insert_source(LibinputInputBackend::new(libinput), |event, _, data| data.state.process_input_event(event))
        .map_err(|e| TtyError::Device(e.to_string()))?;
    handle
        .insert_source(drm_notifier, move |event, _, data| match event {
            DrmEvent::VBlank(crtc) => {
                if let Some((_, surface)) = data.tty.as_mut().and_then(|t| t.outputs.get_mut(&crtc)) {
                    let _ = surface.frame_submitted();
                }
                render(data, crtc);
            }
            DrmEvent::Error(e) => eprintln!("draftpaper: drm error {e:?}"),
        })
        .map_err(|e| TtyError::Device(e.to_string()))?;
    handle
        .insert_source(session_notifier, |event, _, data| {
            let Some(tty) = data.tty.as_mut() else { return };
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

    connect_outputs(data)?;
    let crtcs: Vec<_> = data.tty.as_ref().unwrap().outputs.keys().copied().collect();
    for crtc in crtcs {
        render(data, crtc);
    }
    Ok(())
}

/// Every connected display gets an output, left to right, at its preferred mode.
fn connect_outputs(data: &mut CalloopData) -> Result<(), TtyError> {
    let theme = Theme::load();
    let tty = data.tty.as_mut().unwrap();
    let device = tty.manager.device();
    let resources = device.resource_handles().map_err(err("drm resources"))?;
    let mut used = Vec::new();
    let mut found = Vec::new();
    for &handle in resources.connectors() {
        let Ok(info) = device.get_connector(handle, true) else { continue };
        if info.state() != connector::State::Connected || info.modes().is_empty() {
            continue;
        }
        let crtc = info
            .encoders()
            .iter()
            .filter_map(|e| device.get_encoder(*e).ok())
            .flat_map(|e| resources.filter_crtcs(e.possible_crtcs()))
            .find(|c| !used.contains(c));
        let Some(crtc) = crtc else { continue };
        used.push(crtc);
        found.push((info, crtc));
    }

    let mut x = 0;
    for (info, crtc) in found {
        let mode = *info.modes().iter().find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED)).unwrap_or(&info.modes()[0]);
        let (w, h) = (mode.size().0 as i32, mode.size().1 as i32);
        let (phys_w, phys_h) = info.size().unwrap_or((0, 0));
        let name = format!("{}-{}", info.interface().as_str(), info.interface_id());
        let mm = if phys_w > 0 { w as f32 / phys_w as f32 } else { FALLBACK_MM };
        eprintln!("draftpaper: output {name} {w}x{h} ({phys_w}x{phys_h} mm, {mm:.2} px/mm)");

        let output = Output::new(
            name,
            PhysicalProperties { size: (phys_w as i32, phys_h as i32).into(), subpixel: Subpixel::Unknown, make: "Unknown".into(), model: "Unknown".into() },
        );
        let _global = output.create_global::<crate::Draftpaper>(&data.display_handle);
        let wl_mode = WlMode::from(mode);
        output.set_preferred(wl_mode);
        output.change_current_state(Some(wl_mode), None, None, Some((x, 0).into()));
        data.state.space.map_output(&output, (x, 0));
        attach_sheet(&output, theme.clone(), mm);
        if data.state.grid.is_none() {
            // the layout lives on the first output for now (#14 gives each output its own)
            data.state.grid = Some(Grid::new((w, h).into(), mm as f64, theme.margin_mm as f64));
        }
        x += w;

        let surface = tty
            .manager
            .initialize_output::<_, OutputElements>(crtc, mode, &[info.handle()], &output, None, &mut tty.renderer, &DrmOutputRenderElements::default())
            .map_err(err("initialize output"))?;
        tty.outputs.insert(crtc, (output, surface));
    }
    if tty.outputs.is_empty() {
        return Err(TtyError::Device("no connected display".into()));
    }
    Ok(())
}

/// Render one output; on an unchanged frame try again a refresh later.
fn render(data: &mut CalloopData, crtc: crtc::Handle) {
    let Some(tty) = data.tty.as_mut() else { return };
    if !tty.session.is_active() {
        return;
    }
    let Some((output, surface)) = tty.outputs.get_mut(&crtc) else { return };
    let output = output.clone();
    let elements = output_elements(&data.state, &mut tty.renderer, &output, Some((&tty.cursor.buffer, tty.cursor.hotspot)));
    let queued = match surface.render_frame(&mut tty.renderer, &elements, [0.1, 0.1, 0.1, 1.0], FrameFlags::DEFAULT) {
        Ok(result) if !result.is_empty => surface.queue_frame(()).is_ok(),
        Ok(_) => false,
        Err(e) => {
            eprintln!("draftpaper: render {e:?}");
            false
        }
    };

    let now = data.state.start_time.elapsed();
    data.state.space.elements().for_each(|w| w.send_frame(&output, now, Some(Duration::ZERO), |_, _| Some(output.clone())));
    data.state.space.refresh();
    data.state.popups.cleanup();
    let _ = data.display_handle.flush_clients();

    if !queued {
        let refresh = output.current_mode().map(|m| m.refresh).unwrap_or(60_000).max(1);
        let delay = Duration::from_micros(1_000_000_000 / refresh as u64);
        let handle: LoopHandle<CalloopData> = data.state.loop_handle.clone();
        let _ = handle.insert_source(Timer::from_duration(delay), move |_, _, data| {
            render(data, crtc);
            TimeoutAction::Drop
        });
    }
}
