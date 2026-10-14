//! Server-side decorations: every window is asked to draw no title bar or frame of its own; the
//! compositor draws a 1 px border around it in the theme's window colour (render.rs).

use smithay::{
    delegate_xdg_decoration,
    reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode,
    wayland::shell::xdg::{decoration::XdgDecorationHandler, ToplevelSurface},
};

use crate::Draftpaper;

fn server_side(toplevel: &ToplevelSurface) {
    toplevel.with_pending_state(|state| state.decoration_mode = Some(Mode::ServerSide));
    if toplevel.is_initial_configure_sent() {
        toplevel.send_pending_configure();
    }
}

impl XdgDecorationHandler for Draftpaper {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        server_side(&toplevel);
    }
    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: Mode) {
        server_side(&toplevel);
    }
    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        server_side(&toplevel);
    }
}

delegate_xdg_decoration!(Draftpaper);
