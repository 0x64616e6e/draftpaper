//! X11 applications. Xwayland (Debian's `xwayland`) starts with the compositor as a rootless Wayland
//! client, and Smithay's X11 window manager runs inside the compositor: X11 windows tile like Wayland
//! ones, transient windows and dialogs float, and unmanaged windows (menus, tooltips) are shown where
//! they ask to be. The clipboard and the primary selection are bridged both ways. Clients started after
//! Xwayland is up find it through DISPLAY. Follows anvil's shell/x11.rs (Smithay v0.7.0, MIT).

use std::os::fd::OwnedFd;

use smithay::{
    delegate_xwayland_shell,
    desktop::Window,
    utils::{Logical, Rectangle},
    wayland::{
        selection::{
            data_device::{clear_data_device_selection, current_data_device_selection_userdata, request_data_device_client_selection, set_data_device_selection},
            primary_selection::{clear_primary_selection, current_primary_selection_userdata, request_primary_client_selection, set_primary_selection},
            SelectionTarget,
        },
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        xwm::{Reorder, ResizeEdge, WmWindowType, XwmId},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
    },
};

use crate::{focus::KeyboardFocusTarget, Draftpaper};

/// The first X display number no running server holds: nothing answers on its abstract or path socket
/// and its lock file, if any, names no live process (leftovers of a killed session count as free).
/// Smithay, left to choose, takes a display whose lock file is missing and unlinks its socket file
/// before finding the abstract socket taken, which deletes a running Xorg's /tmp/.X11-unix/X0.
fn free_display() -> Option<u32> {
    use std::os::{linux::net::SocketAddrExt, unix::net::{SocketAddr, UnixStream}};
    (1..33).find(|n| {
        let path = format!("/tmp/.X11-unix/X{n}");
        let abstract_answers = SocketAddr::from_abstract_name(path.as_bytes()).is_ok_and(|a| UnixStream::connect_addr(&a).is_ok());
        let path_answers = UnixStream::connect(&path).is_ok();
        let locked = match std::fs::read_to_string(format!("/tmp/.X{n}-lock")) {
            Ok(pid) => pid.trim().parse::<u32>().map_or(true, |pid| std::path::Path::new(&format!("/proc/{pid}")).exists()),
            Err(e) => e.kind() != std::io::ErrorKind::NotFound,
        };
        !abstract_answers && !path_answers && !locked
    })
}

impl Draftpaper {
    /// Start Xwayland; the window manager attaches once it is ready.
    pub fn start_xwayland(&mut self) {
        let Some(number) = free_display() else {
            eprintln!("draftpaper: no free X display; X11 applications will not run");
            return;
        };
        let (xwayland, client) = match XWayland::spawn(
            &self.display_handle,
            Some(number),
            std::iter::empty::<(String, String)>(),
            true,
            std::process::Stdio::null(),
            std::process::Stdio::null(),
            |_| (),
        ) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("draftpaper: no Xwayland ({e}); X11 applications will not run");
                return;
            }
        };
        let handle = self.loop_handle.clone();
        let _ = self.loop_handle.insert_source(xwayland, move |event, _, data| match event {
            XWaylandEvent::Ready { x11_socket, display_number } => match X11Wm::start_wm(handle.clone(), x11_socket, client.clone()) {
                Ok(wm) => {
                    data.xwm = Some(wm);
                    std::env::set_var("DISPLAY", format!(":{display_number}"));
                    eprintln!("draftpaper: Xwayland on :{display_number}");
                }
                Err(e) => eprintln!("draftpaper: X11 window manager: {e}"),
            },
            XWaylandEvent::Error => eprintln!("draftpaper: Xwayland failed to start"),
        });
    }

    fn x11_window(&self, surface: &X11Surface) -> Option<Window> {
        self.space
            .elements()
            .chain(self.screens.iter().flat_map(|s| s.workspaces.iter().flat_map(|w| w.layout.windows().chain(w.floating.iter()))))
            .find(|w| w.x11_surface() == Some(surface))
            .cloned()
    }

    /// An X11 window is gone (unmapped or destroyed): out of its workspace and the space.
    fn forget_x11(&mut self, surface: &X11Surface) {
        let Some(window) = self.x11_window(surface) else { return };
        self.space.unmap_elem(&window);
        if let Some((i, k)) = self.workspace_of(&window) {
            let grid = self.screens[i].grid;
            let ws = &mut self.screens[i].workspaces[k];
            ws.floating.retain(|w| w != &window);
            if ws.fullscreen.as_ref() == Some(&window) {
                ws.fullscreen = None;
            }
            ws.layout.remove(&window, &grid);
            self.relayout();
            let next = self.screens.get(self.active).and_then(|s| s.layout().focused());
            self.focus(next);
        }
    }
}

impl XWaylandShellHandler for Draftpaper {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}
delegate_xwayland_shell!(Draftpaper);

impl XwmHandler for Draftpaper {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().unwrap()
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}
    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        let _ = surface.set_mapped(true);
        let dialog = surface.is_transient_for().is_some()
            || matches!(surface.window_type(), Some(WmWindowType::Dialog | WmWindowType::Utility | WmWindowType::Splash | WmWindowType::Toolbar));
        self.place_as(Window::new_x11_window(surface), dialog);
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        let loc = surface.geometry().loc;
        let window = Window::new_x11_window(surface);
        self.space.map_element(window.clone(), loc, false);
        self.space.raise_element(&window, false);
    }

    fn unmapped_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        self.forget_x11(&surface);
        if !surface.is_override_redirect() {
            let _ = surface.set_mapped(false);
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, surface: X11Surface) {
        self.forget_x11(&surface);
    }

    fn configure_request(&mut self, _xwm: XwmId, surface: X11Surface, _x: Option<i32>, _y: Option<i32>, w: Option<u32>, h: Option<u32>, _reorder: Option<Reorder>) {
        // a tiled window keeps its tile; a floating or not yet placed one may choose its size
        let tiled = self.x11_window(&surface).is_some_and(|w| self.screens.iter().any(|s| s.workspaces.iter().any(|ws| ws.layout.contains(&w))));
        if tiled {
            self.relayout();
            return;
        }
        let mut geometry: Rectangle<i32, Logical> = surface.geometry();
        if let Some(w) = w {
            geometry.size.w = w as i32;
        }
        if let Some(h) = h {
            geometry.size.h = h as i32;
        }
        let _ = surface.configure(geometry);
    }

    fn configure_notify(&mut self, _xwm: XwmId, surface: X11Surface, geometry: Rectangle<i32, Logical>, _above: Option<u32>) {
        // unmanaged windows (menus, tooltips) move themselves
        if surface.is_override_redirect() {
            if let Some(window) = self.x11_window(&surface) {
                self.space.map_element(window, geometry.loc, false);
            }
        }
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        if let Some(window) = self.x11_window(&surface) {
            self.set_fullscreen(&window, true);
        }
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, surface: X11Surface) {
        if let Some(window) = self.x11_window(&surface) {
            let _ = surface.set_fullscreen(false);
            self.set_fullscreen(&window, false);
        }
    }

    // tiling: windows do not move or resize themselves with the mouse
    fn resize_request(&mut self, _xwm: XwmId, _surface: X11Surface, _button: u32, _edge: ResizeEdge) {}
    fn move_request(&mut self, _xwm: XwmId, _surface: X11Surface, _button: u32) {}

    fn allow_selection_access(&mut self, xwm: XwmId, _selection: SelectionTarget) -> bool {
        // only while an X11 window of this Xwayland has the keyboard
        matches!(
            self.seat.get_keyboard().and_then(|k| k.current_focus()),
            Some(KeyboardFocusTarget::Window(w)) if w.x11_surface().and_then(|x| x.xwm_id()) == Some(xwm)
        )
    }

    fn send_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_type: String, fd: OwnedFd) {
        let _ = match selection {
            SelectionTarget::Clipboard => request_data_device_client_selection(&self.seat, mime_type, fd).map_err(|e| e.to_string()),
            SelectionTarget::Primary => request_primary_client_selection(&self.seat, mime_type, fd).map_err(|e| e.to_string()),
        };
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        match selection {
            SelectionTarget::Clipboard => set_data_device_selection(&self.display_handle, &self.seat, mime_types, ()),
            SelectionTarget::Primary => set_primary_selection(&self.display_handle, &self.seat, mime_types, ()),
        }
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        match selection {
            SelectionTarget::Clipboard => {
                if current_data_device_selection_userdata(&self.seat).is_some() {
                    clear_data_device_selection(&self.display_handle, &self.seat)
                }
            }
            SelectionTarget::Primary => {
                if current_primary_selection_userdata(&self.seat).is_some() {
                    clear_primary_selection(&self.display_handle, &self.seat)
                }
            }
        }
    }
}
