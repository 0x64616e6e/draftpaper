// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
mod compositor;
mod decoration;
pub mod layer_shell;
mod selection;
mod xdg_shell;

use crate::Draftpaper;

//
// Wl Seat
//

use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::data_device::{
    set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::selection::SelectionHandler;
use smithay::{delegate_data_device, delegate_output, delegate_seat};

impl SeatHandler for Draftpaper {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Draftpaper> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: smithay::input::pointer::CursorImageStatus) {}

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client.clone());
        smithay::wayland::selection::primary_selection::set_primary_focus(dh, seat, client);
    }
}

delegate_seat!(Draftpaper);

//
// Wl Data Device
//

impl SelectionHandler for Draftpaper {
    type SelectionUserData = ();
}

impl DataDeviceHandler for Draftpaper {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for Draftpaper {}
impl ServerDndGrabHandler for Draftpaper {}

delegate_data_device!(Draftpaper);

//
// Wl Output & Xdg Output
//

impl OutputHandler for Draftpaper {}
delegate_output!(Draftpaper);
