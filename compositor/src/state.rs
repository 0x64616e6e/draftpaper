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

use crate::CalloopData;

pub struct Draftpaper {
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub space: Space<Window>,
    pub loop_signal: LoopSignal,

    // Smithay State
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: smithay::wayland::shell::xdg::decoration::XdgDecorationState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Draftpaper>,
    pub data_device_state: DataDeviceState,
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
    pub loop_handle: smithay::reexports::calloop::LoopHandle<'static, CalloopData>,
}

impl Draftpaper {
    pub fn new(event_loop: &mut EventLoop<'static, CalloopData>, display: Display<Self>) -> Self {
        let start_time = std::time::Instant::now();

        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state = smithay::wayland::shell::xdg::decoration::XdgDecorationState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
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
            xdg_decoration_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            popups,
            seat,
            screens: Vec::new(),
            pending: Vec::new(),
            active: 0,
            session: None,
            tty: false,
            loop_handle: event_loop.handle(),
        }
    }

    fn init_wayland_listener(
        display: Display<Draftpaper>,
        event_loop: &mut EventLoop<CalloopData>,
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
                        display.get_mut().dispatch_clients(&mut state.state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        socket_name
    }

    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.space.element_under(pos).and_then(|(window, location)| {
            window
                .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, p)| (s, (p + location).to_f64()))
        })
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
            s.layout().arrange(&mut self.space, &s.grid, ws.fullscreen.as_ref());
            // dialogs: centred inside the frame at their own size, above the tiles
            for window in &ws.floating {
                let size = window.geometry().size;
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
                }
                self.space.map_element(window.clone(), geometry.loc, false);
                self.space.raise_element(window, false);
            }
        }
    }

    /// The window with the keyboard focus.
    pub fn focused_window(&self) -> Option<Window> {
        let surface = self.seat.get_keyboard()?.current_focus()?;
        self.space.elements().find(|w| w.toplevel().map(|t| t.wl_surface()) == Some(&surface)).cloned()
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
        keyboard.set_focus(self, window.and_then(|w| w.toplevel().map(|t| t.wl_surface().clone())), serial);
    }

    /// Place a new window: a dialog (a window with a parent) floats, anything else joins the layout.
    pub fn place(&mut self, window: Window) {
        let dialog = window.toplevel().is_some_and(|t| t.parent().is_some());
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
