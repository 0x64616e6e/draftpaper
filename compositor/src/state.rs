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
    /// the X11 window manager, once Xwayland is up (handlers/xwayland.rs)
    pub xwm: Option<smithay::xwayland::X11Wm>,
    pub xwayland_shell_state: smithay::wayland::xwayland_shell::XWaylandShellState,
    /// settings and bindings from config.toml (config.rs), read again when the file changes
    pub config: crate::config::Config,
    pub config_mtime: Option<std::time::SystemTime>,
    /// the TTY backend's devices and outputs (None when nested)
    pub tty_data: Option<crate::tty::TtyData>,
    pub loop_handle: smithay::reexports::calloop::LoopHandle<'static, Draftpaper>,
    /// the IPC socket and its clients (ipc.rs)
    pub ipc: crate::ipc::Ipc,
}

impl Draftpaper {
    pub fn new(event_loop: &mut EventLoop<'static, Draftpaper>, display: Display<Self>) -> Self {
        let start_time = std::time::Instant::now();

        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
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
            xwm: None,
            xwayland_shell_state,
            config: crate::config::Config::default_config(),
            config_mtime: None,
            tty_data: None,
            loop_handle: event_loop.handle(),
            ipc: Default::default(),
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
            // dialogs: centred inside the frame at their own size, above the tiles
            for window in &ws.floating {
                let size = window.geometry().size;
                let a = s.grid.area;
                if let Some(x11) = window.x11_surface() {
                    let loc = (a.loc.x + (a.size.w - size.w).max(0) / 2, a.loc.y + (a.size.h - size.h).max(0) / 2);
                    let _ = x11.configure(smithay::utils::Rectangle::new(loc.into(), size));
                }
                let a = s.grid.area;
                let loc = (a.loc.x + (a.size.w - size.w).max(0) / 2, a.loc.y + (a.size.h - size.h).max(0) / 2);
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

    /// After a frame of `output`: frame callbacks to everything drawn on it, windows, layer surfaces (the
    /// shell's margins and title block) and the lock surface. A client that waits for its callback
    /// before drawing again (Qt does) stays on its first picture without one.
    pub fn send_frames(&self, output: &smithay::output::Output, now: std::time::Duration) {
        let throttle = Some(std::time::Duration::ZERO);
        self.space.elements().for_each(|w| w.send_frame(output, now, throttle, |_, _| Some(output.clone())));
        for layer in smithay::desktop::layer_map_for_output(output).layers() {
            layer.send_frame(output, now, throttle, |_, _| Some(output.clone()));
        }
        if let Some(lock) = self.lock_surfaces.get(&output.name()) {
            smithay::desktop::utils::send_frames_surface_tree(lock.wl_surface(), output, now, throttle, |_, _| Some(output.clone()));
        }
    }

    /// The first window, mapped, on a workspace not shown or parked, for which `f` holds.
    pub fn window_where(&self, f: impl Fn(&Window) -> bool) -> Option<Window> {
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

    /// Place a new Wayland window: a dialog (a window with a parent) floats, anything else joins the layout.
    pub fn place(&mut self, window: Window) {
        let dialog = window.toplevel().is_some_and(|t| t.parent().is_some());
        self.place_as(window, dialog);
    }

    /// Place a new window, floating (`dialog`) or in the layout of the active screen.
    pub fn place_as(&mut self, window: Window, dialog: bool) {
        if let Some(screen) = self.active_screen() {
            let grid = screen.grid;
            if dialog {
                screen.workspace_mut().floating.push(window.clone());
            } else {
                screen.layout_mut().add(window.clone(), &grid);
            }
            self.relayout();
            self.focus(Some(window));
        } else {
            self.space.map_element(window, (0, 0), false);
        }
    }
}
