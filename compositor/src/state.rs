// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
use std::{ffi::OsString, sync::Arc};

use smithay::{
    desktop::{PopupManager, Space, Window, WindowSurfaceType},
    input::{Seat, SeatState},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle,
        },
    },
    utils::{Logical, Point},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::XdgShellState,
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;


pub struct Draftpaper {
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub space: Space<Window>,
    pub loop_signal: LoopSignal,

    // Smithay State
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub layer_shell_state: smithay::wayland::shell::wlr_layer::WlrLayerShellState,
    pub xdg_decoration_state: smithay::wayland::shell::xdg::decoration::XdgDecorationState,
    pub shm_state: ShmState,
    /// linux-dmabuf: GPU buffers from clients (the TTY backend creates the global for its GPU)
    pub dmabuf_state: smithay::wayland::dmabuf::DmabufState,
    pub dmabuf_global: Option<smithay::wayland::dmabuf::DmabufGlobal>,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Draftpaper>,
    pub data_device_state: DataDeviceState,
    // locking and idle (handlers/lock_idle.rs)
    pub session_lock_state: smithay::wayland::session_lock::SessionLockManagerState,
    pub idle_notifier_state: smithay::wayland::idle_notify::IdleNotifierState<Self>,
    pub idle_inhibit_state: smithay::wayland::idle_inhibit::IdleInhibitManagerState,
    pub locked: bool,
    /// the magnifier, while it is out (lens.rs)
    pub lens: Option<crate::lens::Lens>,
    /// the displays are off (DPMS; tty::set_power)
    pub screens_off: bool,
    /// the lock object of the locker holding the lock (handlers/lock_idle.rs)
    pub lock_holder: Option<smithay::reexports::wayland_protocols::ext::session_lock::v1::server::ext_session_lock_v1::ExtSessionLockV1>,
    /// lock surfaces by output name
    pub lock_surfaces: std::collections::HashMap<String, smithay::wayland::session_lock::LockSurface>,
    pub inhibitors: Vec<WlSurface>,
    pub primary_selection_state: smithay::wayland::selection::primary_selection::PrimarySelectionState,
    pub wlr_data_control_state: smithay::wayland::selection::wlr_data_control::DataControlState,
    pub ext_data_control_state: smithay::wayland::selection::ext_data_control::DataControlState,
    pub popups: PopupManager,

    pub seat: Seat<Self>,

    // one screen per output: its zone grid and layout (screens.rs); `active` takes new windows
    pub screens: Vec<crate::screens::Screen>,
    /// new windows, placed on their first commit (dialogs float, the rest tile)
    pub pending: Vec<Window>,
    pub active: usize,

    /// on the TTY: the session (for VT switches), and the bindings move from Alt to Super
    pub session: Option<smithay::backend::session::libseat::LibSeatSession>,
    pub tty: bool,
    /// the login screen (--greeter): no bindings but the VT switches, no IPC, no Xwayland
    pub greeter: bool,
    /// on the way out (tty::leave): the outputs whose last frame, the bare sheet, is not on screen yet
    pub leaving: Option<std::collections::HashSet<smithay::reexports::drm::control::crtc::Handle>>,
    /// the sheet blurred behind the login card (blur.rs), in millimetres: from, to, since (IPC "blur")
    pub blur: (f64, f64, std::time::Instant),
    /// the X11 window manager, once Xwayland is up (handlers/xwayland.rs)
    pub xwm: Option<smithay::xwayland::X11Wm>,
    pub xwayland_shell_state: smithay::wayland::xwayland_shell::XWaylandShellState,
    /// settings and bindings from config.toml (config.rs), read again when the file changes
    pub config: crate::config::Config,
    pub config_mtime: Option<std::time::SystemTime>,
    /// the TTY backend's devices and outputs (None when nested)
    pub tty_data: Option<crate::tty::TtyData>,
    /// the headless backend (headless.rs), for tests
    pub headless: Option<crate::headless::Headless>,
    pub loop_handle: smithay::reexports::calloop::LoopHandle<'static, Draftpaper>,
    /// the IPC socket and its clients (ipc.rs)
    pub ipc: crate::ipc::Ipc,
    /// windows flying into and out of cells, workspaces sliding (animation.rs)
    pub anim: crate::animation::Animations,
    /// nested: a screenshot of the next frame (tty::screenshot)
    pub screenshot_requested: bool,
    /// the window and the screen the pointer was last over (focus follows mouse, input.rs)
    pub pointer_over: (Option<Window>, Option<usize>),
    /// the timer repeating a held binding (input.rs)
    pub key_repeat: Option<smithay::reexports::calloop::RegistrationToken>,
    /// screencopy frames waiting for the backend's next frame (screencopy.rs)
    pub screencopies: Vec<crate::screencopy::PendingCopy>,
    /// the output-management clients and what they were last told (output_management.rs)
    pub output_management: crate::output_management::OutputManagement,
}

impl Draftpaper {
    pub fn new(event_loop: &mut EventLoop<'static, Draftpaper>, display: Display<Self>) -> Self {
        let start_time = std::time::Instant::now();

        let dh = display.handle();

        // v6: wl_surface.preferred_buffer_scale and _transform (send_scale)
        let compositor_state = CompositorState::new_v6::<Self>(&dh);
        // a client draws at an output's exact scale (1.5) and says how big its buffer is on screen
        smithay::wayland::viewporter::ViewporterState::new::<Self>(&dh);
        smithay::wayland::fractional_scale::FractionalScaleManagerState::new::<Self>(&dh);
        // screenshots for clients (grim, slurp, the screenshot portal)
        crate::screencopy::init(&dh);
        // displays arranged by kanshi, wlr-randr, wdisplays
        crate::output_management::init(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xwayland_shell_state = smithay::wayland::xwayland_shell::XWaylandShellState::new::<Self>(&dh);
        let layer_shell_state = smithay::wayland::shell::wlr_layer::WlrLayerShellState::new::<Self>(&dh);
        let xdg_decoration_state = smithay::wayland::shell::xdg::decoration::XdgDecorationState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let session_lock_state = smithay::wayland::session_lock::SessionLockManagerState::new::<Self, _>(&dh, |_| true);
        let idle_notifier_state = smithay::wayland::idle_notify::IdleNotifierState::new(&dh, event_loop.handle());
        let idle_inhibit_state = smithay::wayland::idle_inhibit::IdleInhibitManagerState::new::<Self>(&dh);
        let primary_selection_state = smithay::wayland::selection::primary_selection::PrimarySelectionState::new::<Self>(&dh);
        let wlr_data_control_state =
            smithay::wayland::selection::wlr_data_control::DataControlState::new::<Self, _>(&dh, Some(&primary_selection_state), |_| true);
        let ext_data_control_state =
            smithay::wayland::selection::ext_data_control::DataControlState::new::<Self, _>(&dh, Some(&primary_selection_state), |_| true);
        let popups = PopupManager::default();

        // A seat is a group of keyboards, pointer and touch devices.
        // A seat typically has a pointer and maintains a keyboard focus and a pointer focus.
        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "winit");

        // Notify clients that we have a keyboard, for the sake of the example we assume that keyboard is always present.
        // You may want to track keyboard hot-plug in real compositor.
        seat.add_keyboard(Default::default(), 200, 25).unwrap();

        // Notify clients that we have a pointer (mouse)
        // Here we assume that there is always pointer plugged in
        seat.add_pointer();

        // A space represents a two-dimensional plane. Windows and Outputs can be mapped onto it.
        //
        // Windows get a position and stacking order through mapping.
        // Outputs become views of a part of the Space and can be rendered via Space::render_output.
        let space = Space::default();

        let socket_name = Self::init_wayland_listener(display, event_loop);

        // Get the loop signal, used to stop the event loop
        let loop_signal = event_loop.get_signal();

        Self {
            start_time,
            display_handle: dh,

            space,
            loop_signal,
            socket_name,

            compositor_state,
            xdg_shell_state,
            layer_shell_state,
            xdg_decoration_state,
            shm_state,
            dmabuf_state: smithay::wayland::dmabuf::DmabufState::new(),
            dmabuf_global: None,
            output_manager_state,
            seat_state,
            data_device_state,
            session_lock_state,
            idle_notifier_state,
            idle_inhibit_state,
            locked: false,
            lens: None,
            screens_off: false,
            lock_holder: None,
            lock_surfaces: Default::default(),
            inhibitors: Vec::new(),
            primary_selection_state,
            wlr_data_control_state,
            ext_data_control_state,
            popups,
            seat,
            screens: Vec::new(),
            pending: Vec::new(),
            active: 0,
            session: None,
            tty: false,
            greeter: false,
            leaving: None,
            blur: (0.0, 0.0, std::time::Instant::now()),
            xwm: None,
            xwayland_shell_state,
            config: crate::config::Config::default_config(),
            config_mtime: None,
            tty_data: None,
            headless: None,
            loop_handle: event_loop.handle(),
            ipc: Default::default(),
            anim: Default::default(),
            screenshot_requested: false,
            key_repeat: None,
            pointer_over: (None, None),
            screencopies: Vec::new(),
            output_management: Default::default(),
        }
    }

    fn init_wayland_listener(
        display: Display<Draftpaper>,
        event_loop: &mut EventLoop<'static, Draftpaper>,
    ) -> OsString {
        // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
        let listening_socket = ListeningSocketSource::new_auto().unwrap();

        // Get the name of the listening socket.
        // Clients will connect to this socket.
        let socket_name = listening_socket.socket_name().to_os_string();

        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                // Inside the callback, you should insert the client into the display.
                //
                // You may also associate some data with the client when inserting the client.
                state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    .unwrap();
            })
            .expect("Failed to init the wayland event source.");

        // You also need to add the display itself to the event loop, so that client events will be processed by wayland-server.
        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // Safety: we don't drop the display
                    unsafe {
                        display.get_mut().dispatch_clients(state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        socket_name
    }

    /// The popups (menus) of the bottom and background layer surfaces on `output`, parents first, each
    /// with the global position of its surface. A menu opens above every window, whatever layer its
    /// parent is on (the shell's tray menu belongs to a margin, under the windows).
    pub fn low_layer_popups(&self, output: &smithay::output::Output) -> Vec<(smithay::desktop::PopupKind, Point<i32, Logical>)> {
        use smithay::{desktop::{layer_map_for_output, PopupManager}, wayland::shell::wlr_layer::Layer};
        let Some(og) = self.space.output_geometry(output) else { return Vec::new() };
        let map = layer_map_for_output(output);
        let mut popups = Vec::new();
        for layer in map.layers().filter(|l| matches!(l.layer(), Layer::Bottom | Layer::Background)) {
            let Some(lg) = map.layer_geometry(layer) else { continue };
            for (popup, offset) in PopupManager::popups_for_surface(layer.wl_surface()) {
                popups.push((popup.clone(), og.loc + lg.loc + offset - popup.geometry().loc));
            }
        }
        popups
    }

    /// The layer surface under `pos`, if what is under it belongs to one (the surface itself, a popup of
    /// it), with whether it reads keys (its keyboard interactivity is not none).
    pub fn layer_under(&self, pos: Point<f64, Logical>) -> Option<(smithay::reexports::wayland_server::protocol::wl_surface::WlSurface, bool)> {
        use smithay::{desktop::layer_map_for_output, wayland::shell::wlr_layer::KeyboardInteractivity};
        let (crate::focus::PointerFocusTarget::Surface(surface), _) = self.surface_under(pos)? else { return None };
        self.space.outputs().find_map(|o| {
            let map = layer_map_for_output(o);
            let layer = map.layer_for_surface(&surface, WindowSurfaceType::ALL)?;
            Some((layer.wl_surface().clone(), layer.cached_state().keyboard_interactivity != KeyboardInteractivity::None))
        })
    }

    /// The surface under `pos`: popups of the low layers, overlay and top layer surfaces, then windows,
    /// then bottom and background layer surfaces (Draftpaper's margins and title block live on the
    /// bottom layer).
    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(crate::focus::PointerFocusTarget, Point<f64, Logical>)> {
        use crate::focus::PointerFocusTarget as T;
        use smithay::{desktop::layer_map_for_output, wayland::shell::wlr_layer::Layer};
        let output = self.space.outputs().find(|o| self.space.output_geometry(o).is_some_and(|g| g.to_f64().contains(pos)))?;
        let og = self.space.output_geometry(output)?;
        if self.locked {
            let lock = self.lock_surfaces.get(&output.name())?;
            return Some((T::Surface(lock.wl_surface().clone()), og.loc.to_f64()));
        }
        // a low layer's menu, above everything (the last popup is the innermost)
        let popup = self.low_layer_popups(output).into_iter().rev().find_map(|(popup, loc)| {
            smithay::desktop::utils::under_from_surface_tree(popup.wl_surface(), pos, loc, WindowSurfaceType::ALL)
                .map(|(s, p)| (T::Surface(s), p.to_f64()))
        });
        if popup.is_some() {
            return popup;
        }
        let map = layer_map_for_output(output);
        let layer_under = |layers: &[Layer]| {
            layers.iter().find_map(|l| {
                let layer = map.layer_under(*l, pos - og.loc.to_f64())?;
                let lg = map.layer_geometry(layer)?;
                layer
                    .surface_under(pos - og.loc.to_f64() - lg.loc.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (T::Surface(s), (p + lg.loc + og.loc).to_f64()))
            })
        };
        layer_under(&[Layer::Overlay, Layer::Top])
            .or_else(|| {
                self.space.element_under(pos).and_then(|(window, location)| {
                    // an X11 window takes the pointer whole; a Wayland window by the surface under it
                    if let Some(x) = window.x11_surface() {
                        return Some((T::X11(x.clone()), location.to_f64()));
                    }
                    window
                        .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                        .map(|(s, p)| (T::Surface(s), (p + location).to_f64()))
                })
            })
            .or_else(|| layer_under(&[Layer::Bottom, Layer::Background]))
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

impl Draftpaper {
    /// Place the windows on their screens' grids again (after a change in a layout or an output).
    pub fn relayout(&mut self) {
        // a workspace that is neither shown nor holds a window goes away (its last window closed or
        // moved while another workspace was shown)
        for s in &mut self.screens {
            let current = s.workspaces[s.current].number;
            s.workspaces.retain(|w| w.number == current || !w.is_empty());
            s.current = s.workspaces.iter().position(|w| w.number == current).unwrap_or(0);
        }
        // every layout on its screen's zones (another screen or scale since); a window with no zone left floats
        for s in &mut self.screens {
            let grid = s.grid;
            for w in &mut s.workspaces {
                w.layout.fit(&grid);
                let evicted = w.layout.take_evicted();
                w.floating.extend(evicted);
            }
        }
        for s in &self.screens {
            // windows of workspaces not shown leave the space
            for (k, w) in s.workspaces.iter().enumerate() {
                if k != s.current {
                    for window in w.layout.windows().chain(w.floating.iter()) {
                        self.space.unmap_elem(window);
                    }
                }
            }
            let ws = s.workspace();
            // panels with an exclusive zone (a foreign bar) take their room from the frame's interior
            let mut grid = s.grid;
            if let Some(og) = self.space.output_geometry(&s.output) {
                // arranged first: after a change of the output's size or scale the map still has the old one
                let mut map = smithay::desktop::layer_map_for_output(&s.output);
                map.arrange();
                let mut zone = map.non_exclusive_zone();
                drop(map);
                zone.loc += og.loc;
                if let Some(area) = grid.area.intersection(zone) {
                    grid.area = area;
                }
            }
            s.layout().arrange(&mut self.space, &grid, ws.fullscreen.as_ref());
            // floating windows, above the tiles, at their own size: centred inside the frame, or (a rule's
            // "corner": picture-in-picture) in its bottom-right corner, above the title block
            for window in &ws.floating {
                crate::layout::set_tile(window, None);
                let size = window.geometry().size;
                let a = s.grid.area;
                let loc = if crate::rules::is_cornered(window) {
                    let gap = (4.0 * s.mm).round() as i32;
                    let block = (36.0 * s.mm).round() as i32;
                    let x = (a.loc.x + a.size.w - size.w - gap).max(a.loc.x);
                    let y = (a.loc.y + a.size.h - block - gap - size.h).max(a.loc.y);
                    (x, y)
                } else {
                    (a.loc.x + (a.size.w - size.w).max(0) / 2, a.loc.y + (a.size.h - size.h).max(0) / 2)
                };
                if let Some(x11) = window.x11_surface() {
                    let _ = x11.configure(smithay::utils::Rectangle::new(loc.into(), size));
                }
                self.space.map_element(window.clone(), loc, false);
                self.space.raise_element(window, false);
            }
            // a fullscreen window covers the whole output, sheet and margins included
            if let (Some(window), Some(geometry)) = (&ws.fullscreen, self.space.output_geometry(&s.output)) {
                if let Some(t) = window.toplevel() {
                    t.with_pending_state(|state| {
                        state.size = Some(geometry.size);
                        state.states.set(xdg_toplevel::State::Fullscreen);
                    });
                    if t.is_initial_configure_sent() {
                        t.send_pending_configure();
                    }
                } else if let Some(x11) = window.x11_surface() {
                    let _ = x11.set_fullscreen(true);
                    let _ = x11.configure(geometry);
                }
                self.space.map_element(window.clone(), geometry.loc, false);
                self.space.raise_element(window, false);
            }
        }
    }

    /// Tell a surface (and its subsurfaces) the scale and transform of `output`, the one it is on: the
    /// integer scale (wl_surface v6) and the exact one (fractional-scale-v1). A client that knows them
    /// before it first draws never has to guess from the largest scale of all outputs (Firefox does,
    /// and then draws at the wrong size on another output).
    pub fn send_scale(surface: &WlSurface, output: &smithay::output::Output) {
        use smithay::wayland::{
            compositor::{send_surface_state, with_surface_tree_downward, TraversalAction},
            fractional_scale::with_fractional_scale,
        };
        let (scale, transform) = (output.current_scale(), output.current_transform());
        with_surface_tree_downward(
            surface,
            (),
            |_, _, _| TraversalAction::DoChildren(()),
            |surface, states, _| {
                send_surface_state(surface, states, scale.integer_scale(), transform);
                with_fractional_scale(states, |fs| fs.set_preferred_scale(scale.fractional_scale()));
            },
            |_, _, _| true,
        );
    }

    /// The output a new surface will most likely be on: the active screen's.
    pub fn active_output(&self) -> Option<smithay::output::Output> {
        self.screens.get(self.active).map(|s| s.output.clone()).or_else(|| self.space.outputs().next().cloned())
    }

    /// After a frame of `output`: frame callbacks to everything drawn on it, windows, layer surfaces (the
    /// shell's margins and title block) and the lock surface. A client that waits for its callback
    /// before drawing again (Qt does) stays on its first picture without one. The windows and layer
    /// surfaces on it are told its scale (send_scale; unchanged values are not sent again).
    pub fn send_frames(&self, output: &smithay::output::Output, now: std::time::Duration) {
        let throttle = Some(std::time::Duration::ZERO);
        self.space.elements().for_each(|w| w.send_frame(output, now, throttle, |_, _| Some(output.clone())));
        for w in self.space.elements_for_output(output) {
            if let Some(surface) = smithay::wayland::seat::WaylandFocus::wl_surface(w) {
                Self::send_scale(&surface, output);
            }
        }
        for layer in smithay::desktop::layer_map_for_output(output).layers() {
            layer.send_frame(output, now, throttle, |_, _| Some(output.clone()));
            Self::send_scale(layer.wl_surface(), output);
        }
        if let Some(lock) = self.lock_surfaces.get(&output.name()) {
            smithay::desktop::utils::send_frames_surface_tree(lock.wl_surface(), output, now, throttle, |_, _| Some(output.clone()));
        }
    }

    /// The first window, mapped, on a workspace not shown or parked, for which `f` holds.
    pub fn window_where(&self, mut f: impl FnMut(&Window) -> bool) -> Option<Window> {
        self.space
            .elements()
            .chain(self.screens.iter().flat_map(|s| {
                s.workspaces.iter().flat_map(|w| w.layout.windows().chain(w.floating.iter()).chain(w.parked.iter().map(|p| &p.window)))
            }))
            .find(|w| f(w))
            .cloned()
    }

    /// The window with the keyboard focus.
    pub fn focused_window(&self) -> Option<Window> {
        match self.seat.get_keyboard()?.current_focus()? {
            crate::focus::KeyboardFocusTarget::Window(w) => Some(w),
            _ => None,
        }
    }

    /// Fullscreen on or off for `window` (on its workspace).
    pub fn set_fullscreen(&mut self, window: &Window, on: bool) {
        let Some((i, k)) = self.workspace_of(window) else { return };
        let ws = &mut self.screens[i].workspaces[k];
        if on {
            ws.fullscreen = Some(window.clone());
        } else if ws.fullscreen.as_ref() == Some(window) {
            ws.fullscreen = None;
            if let Some(t) = window.toplevel() {
                t.with_pending_state(|state| state.states.unset(xdg_toplevel::State::Fullscreen));
            }
        }
        self.relayout();
    }

    /// Give the keyboard to `window` (or to nobody) and mark it activated.
    pub fn focus(&mut self, window: Option<Window>) {
        if self.locked {
            return;
        }
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        if let Some(w) = &window {
            if let Some(i) = self.screen_of(w) {
                self.active = i;
                self.screens[i].layout_mut().focus_window(w);
            }
            self.space.raise_element(w, true);
        }
        for w in self.space.elements() {
            w.set_activated(Some(w) == window.as_ref());
            if let Some(t) = w.toplevel() {
                if t.is_initial_configure_sent() {
                    t.send_pending_configure();
                }
            }
        }
        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, window.map(crate::focus::KeyboardFocusTarget::Window), serial);
    }

    /// Place a new Wayland window: a dialog (a window with a parent) floats, and so does one the window
    /// rules float (rules.rs: picture-in-picture, by default, in the corner and on every workspace);
    /// anything else joins the layout (until it turns out not to fit a tile: float_if_fixed).
    pub fn place(&mut self, window: Window) {
        let dialog = window.toplevel().is_some_and(|t| t.parent().is_some());
        let d = self.decide(&window);
        self.place_as(window, dialog || d.float, d);
    }

    /// What the window rules decide for `window`, as it is now (rules.rs).
    pub fn decide(&self, window: &Window) -> crate::rules::Decision {
        let (app_id, title, _) = crate::ipc::names(window);
        let bounded = window.toplevel().is_some_and(|t| {
            smithay::wayland::compositor::with_states(t.wl_surface(), |states| {
                let mut cached = states.cached_state.get::<smithay::wayland::shell::xdg::SurfaceCachedState>();
                let max = cached.current().max_size;
                max.w > 0 || max.h > 0
            })
        });
        // the titles of the other windows of the same client (the browser's own, for a picture-in-picture)
        use smithay::{reexports::wayland_server::Resource, wayland::seat::WaylandFocus};
        let client = window.wl_surface().and_then(|s| s.client()).map(|c| c.id());
        let mut siblings = Vec::new();
        if client.is_some() {
            self.window_where(|w| {
                if w != window && w.wl_surface().and_then(|s| s.client()).map(|c| c.id()) == client {
                    siblings.push(crate::ipc::names(w).1);
                }
                false
            });
        }
        let pip = window.toplevel().is_some() && crate::rules::looks_like_pip(&title, bounded, &siblings);
        crate::rules::decide(&self.config.rules, &crate::rules::Facts { app_id, title, pip })
    }

    /// A tiled Wayland window that cannot fill its tile floats, centred at its own size: one of one fixed
    /// size (its smallest and largest the same, as swappy's), or one whose largest is smaller than the
    /// tile (a tile would only leave empty paper around it). Clients say so with their first frame,
    /// after the window was placed, so this runs on every commit.
    pub fn float_if_fixed(&mut self, window: &Window) {
        let Some(t) = window.toplevel() else { return };
        let tile = t.with_pending_state(|state| state.size);
        let fixed = smithay::wayland::compositor::with_states(t.wl_surface(), |states| {
            let mut cached = states.cached_state.get::<smithay::wayland::shell::xdg::SurfaceCachedState>();
            let size = cached.current();
            let (min, max) = (size.min_size, size.max_size);
            let one_size = min.w > 0 && min.h > 0 && min == max;
            let too_small = tile.is_some_and(|tile| (max.w > 0 && max.w < tile.w) || (max.h > 0 && max.h < tile.h));
            one_size || too_small
        });
        if !fixed {
            return;
        }
        let Some((i, k)) = self.workspace_of(window) else { return };
        let grid = self.screens[i].grid;
        let ws = &mut self.screens[i].workspaces[k];
        if !ws.layout.windows().any(|w| w == window) || ws.fullscreen.as_ref() == Some(window) {
            return;
        }
        ws.layout.remove(window, &grid);
        ws.floating.push(window.clone());
        self.relayout();
    }

    /// Place a new window, floating (`dialog`) or in the layout of the active screen, as the window
    /// rules decided (`d`: sticky, the corner, the keyboard or not).
    pub fn place_as(&mut self, window: Window, dialog: bool, d: crate::rules::Decision) {
        if dialog && d.sticky {
            window.user_data().insert_if_missing(|| crate::rules::Sticky);
        }
        if dialog && d.place == crate::rules::Place::Corner {
            window.user_data().insert_if_missing(|| crate::rules::Cornered);
        }
        if let Some(screen) = self.active_screen() {
            let grid = screen.grid;
            if dialog {
                screen.workspace_mut().floating.push(window.clone());
            } else if !screen.layout_mut().add(window.clone(), &grid) {
                // every zone taken (Layout::add): it floats
                screen.workspace_mut().floating.push(window.clone());
            }
            self.relayout();
            if d.focus {
                self.focus(Some(window));
            }
        } else {
            self.space.map_element(window, (0, 0), false);
        }
    }
}

/// How long the blur takes to change (IPC "blur").
const BLUR_CHANGE: std::time::Duration = std::time::Duration::from_millis(400);

impl Draftpaper {
    /// The blur now, in millimetres (its change eased in and out).
    pub fn blur_mm(&self) -> f64 {
        let (from, to, since) = self.blur;
        let e = (since.elapsed().as_secs_f64() / BLUR_CHANGE.as_secs_f64()).min(1.0);
        from + (to - from) * e * e * (3.0 - 2.0 * e)
    }

    /// Blur the sheet to `mm`, from where it is now.
    pub fn set_blur(&mut self, mm: f64) {
        self.blur = (self.blur_mm(), mm.clamp(0.0, 20.0), std::time::Instant::now());
    }
}
