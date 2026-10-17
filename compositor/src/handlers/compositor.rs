// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
use crate::{state::ClientState, Draftpaper};
use smithay::wayland::seat::WaylandFocus;
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_shm,
    reexports::wayland_server::{
        protocol::{wl_buffer, wl_surface::WlSurface},
        Client,
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{
            get_parent, is_sync_subsurface, CompositorClientState, CompositorHandler, CompositorState,
        },
        shm::{ShmHandler, ShmState},
    },
};

use super::xdg_shell;

impl CompositorHandler for Draftpaper {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        // Xwayland is a client Smithay created, with its own client data
        if let Some(state) = client.get_data::<smithay::xwayland::XWaylandClientData>() {
            return &state.compositor_state;
        }
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(i) = self.pending.iter().position(|w| w.toplevel().unwrap().wl_surface() == &root) {
                let window = self.pending.remove(i);
                self.place(window.clone());
                // told to draw wherever it went: a window placed out of sight (on a workspace not shown,
                // or parked: a restored session's) would otherwise wait for it, and never draw
                if let Some(t) = window.toplevel() {
                    if !t.is_initial_configure_sent() {
                        t.send_configure();
                    }
                }
            }
            // any window of ours, shown or not, keeps up with what its client commits
            let window = self.window_where(|w| w.wl_surface().as_deref() == Some(&root));
            if let Some(window) = window {
                window.on_commit();
                self.float_if_fixed(&window);
                if self.screens.iter().any(|s| s.workspaces.iter().any(|w| w.floating.contains(&window))) {
                    self.relayout();
                }
            }
        };

        if super::layer_shell::handle_commit(self, surface) {
            return;
        }
        xdg_shell::handle_commit(&mut self.popups, &self.space, surface);
    }
}

impl BufferHandler for Draftpaper {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for Draftpaper {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(Draftpaper);
delegate_shm!(Draftpaper);
