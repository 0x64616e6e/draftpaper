//! wlr-layer-shell: panels, launchers, notifications and Draftpaper's own shell (the margins and the
//! title block, on the bottom layer: above the sheet, below the windows). Layer surfaces are arranged
//! per output by Smithay's layer map; the windows' grid keeps clear of exclusive zones (state.rs).
//! A layer surface that asks for the keyboard (a launcher) gets it while it is mapped.

use smithay::{
    delegate_layer_shell,
    desktop::{layer_map_for_output, LayerSurface, WindowSurfaceType},
    output::Output,
    reexports::wayland_server::protocol::{wl_output::WlOutput, wl_surface::WlSurface},
    wayland::{
        compositor::with_states,
        shell::wlr_layer::{
            KeyboardInteractivity, Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData, WlrLayerShellHandler,
            WlrLayerShellState,
        },
    },
};

use crate::Draftpaper;

impl WlrLayerShellHandler for Draftpaper {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(&mut self, surface: WlrLayerSurface, output: Option<WlOutput>, _layer: Layer, namespace: String) {
        let output = output
            .as_ref()
            .and_then(Output::from_resource)
            .or_else(|| self.screens.get(self.active).map(|s| s.output.clone()))
            .or_else(|| self.space.outputs().next().cloned());
        if let Some(output) = output {
            let _ = layer_map_for_output(&output).map_layer(&LayerSurface::new(surface, namespace));
        }
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let found = self.space.outputs().find_map(|o| {
            let map = layer_map_for_output(o);
            let layer = map.layers().find(|l| l.layer_surface() == &surface).cloned();
            layer.map(|l| (o.clone(), l))
        });
        if let Some((output, layer)) = found {
            layer_map_for_output(&output).unmap_layer(&layer);
            self.relayout();
            // the keyboard goes back to the windows
            let w = self.focused_window().or_else(|| self.screens.get(self.active).and_then(|s| s.layout().focused()));
            self.focus(w);
        }
    }
}

delegate_layer_shell!(Draftpaper);

/// On a commit of a layer surface: arrange its output's layers, send the first configure, and give the
/// keyboard to a surface that asks for it. Returns whether the surface was a layer surface.
pub fn handle_commit(state: &mut Draftpaper, surface: &WlSurface) -> bool {
    let Some(output) = state.space.outputs().find(|o| layer_map_for_output(o).layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).is_some()).cloned() else {
        return false;
    };
    let initial_sent = with_states(surface, |states| {
        states.data_map.get::<LayerSurfaceData>().map(|d| d.lock().unwrap().initial_configure_sent).unwrap_or(true)
    });
    let layer = {
        let mut map = layer_map_for_output(&output);
        map.arrange();
        map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).cloned()
    };
    let Some(layer) = layer else { return true };
    if !initial_sent {
        layer.layer_surface().send_configure();
    } else if layer.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive {
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let keyboard = state.seat.get_keyboard().unwrap();
        keyboard.set_focus(state, Some(crate::focus::KeyboardFocusTarget::Surface(surface.clone())), serial);
    }
    // exclusive zones may have changed the windows' room
    state.relayout();
    true
}
