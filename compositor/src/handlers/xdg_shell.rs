// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
use smithay::{
    delegate_xdg_shell,
    desktop::{find_popup_root_surface, get_popup_toplevel_coords, PopupKind, PopupManager, Space, Window},
    reexports::{
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface},
        },
    },
    utils::Serial,
    wayland::{
        compositor::with_states,
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceData,
        },
    },
};

use crate::Draftpaper;

impl XdgShellHandler for Draftpaper {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        // the scale of the screen it will open on, before it first draws
        if let Some(output) = self.active_output() {
            Self::send_scale(surface.wl_surface(), &output);
        }
        // placed on its first commit, when its parent (if it is a dialog) is known
        self.pending.push(Window::new_wayland_window(surface));
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.pending.retain(|w| w.toplevel() != Some(&surface));
        let Some(window) = self.window_where(|w| w.toplevel() == Some(&surface)) else { return };
        self.space.unmap_elem(&window);
        if let Some((i, k)) = self.workspace_of(&window) {
            let grid = self.screens[i].grid;
            let ws = &mut self.screens[i].workspaces[k];
            ws.floating.retain(|w| w != &window);
            ws.parked.retain(|p| p.window != window);
            if ws.fullscreen.as_ref() == Some(&window) {
                ws.fullscreen = None;
            }
            ws.layout.remove(&window, &grid);
            self.relayout();
            if k == self.screens[i].current {
                let next = self.screens[i].layout().focused();
                self.focus(next);
            }
        }
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>) {
        let window = self.window_where(|w| w.toplevel() == Some(&surface));
        if let Some(window) = window {
            self.set_fullscreen(&window, true);
        }
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        let window = self.window_where(|w| w.toplevel() == Some(&surface));
        if let Some(window) = window {
            self.set_fullscreen(&window, false);
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    /// A menu takes the keyboard and the pointer until it is dismissed (a click outside closes it).
    /// Follows anvil's shell/xdg.rs (Smithay v0.7.0, MIT).
    fn grab(&mut self, surface: PopupSurface, seat: wl_seat::WlSeat, serial: Serial) {
        use crate::focus::KeyboardFocusTarget;
        use smithay::{
            desktop::{find_popup_root_surface, PopupKeyboardGrab, PopupPointerGrab, PopupUngrabStrategy},
            input::{pointer::Focus, Seat},
            wayland::seat::WaylandFocus,
        };
        let Some(seat) = Seat::<Self>::from_resource(&seat) else { return };
        let kind = PopupKind::Xdg(surface);
        let Ok(root) = find_popup_root_surface(&kind) else { return };
        // the root is a window's surface or a layer surface (the shell's tray menu)
        let root = self
            .window_where(|w| w.wl_surface().as_deref() == Some(&root))
            .map(KeyboardFocusTarget::Window)
            .unwrap_or(KeyboardFocusTarget::Surface(root));
        let Ok(mut grab) = self.popups.grab_popup(root, kind, &seat, serial) else { return };
        if let Some(keyboard) = seat.get_keyboard() {
            if keyboard.is_grabbed() && !(keyboard.has_grab(serial) || keyboard.has_grab(grab.previous_serial().unwrap_or(serial))) {
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            keyboard.set_focus(self, grab.current_grab(), serial);
            keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
        }
        if let Some(pointer) = seat.get_pointer() {
            if pointer.is_grabbed() && !(pointer.has_grab(serial) || pointer.has_grab(grab.previous_serial().unwrap_or_else(|| grab.serial()))) {
                grab.ungrab(PopupUngrabStrategy::All);
                return;
            }
            pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
        }
    }
}

// Xdg Shell
delegate_xdg_shell!(Draftpaper);


/// Should be called on `WlSurface::commit`
pub fn handle_commit(popups: &mut PopupManager, space: &Space<Window>, surface: &WlSurface) {
    // Handle toplevel commits.
    if let Some(window) = space
        .elements()
        .find(|w| w.toplevel().map(|t| t.wl_surface()) == Some(surface))
        .cloned()
    {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });

        if !initial_configure_sent {
            if let Some(t) = window.toplevel() {
                t.send_configure();
            }
        }
    }

    // Handle popup commits.
    popups.commit(surface);
    if let Some(popup) = popups.find_popup(surface) {
        match popup {
            PopupKind::Xdg(ref xdg) => {
                if !xdg.is_initial_configure_sent() {
                    // NOTE: This should never fail as the initial configure is always
                    // allowed.
                    xdg.send_configure().expect("initial configure failed");
                }
            }
            PopupKind::InputMethod(ref _input_method) => {}
        }
    }
}

impl Draftpaper {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t.wl_surface()) == Some(&root))
        else {
            return;
        };

        let output = self.space.outputs().next().unwrap();
        let output_geo = self.space.output_geometry(output).unwrap();
        let window_geo = self.space.element_geometry(window).unwrap();

        // The target geometry for the positioner should be relative to its parent's geometry, so
        // we will compute that here.
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}
