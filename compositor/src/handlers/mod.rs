// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
mod compositor;
mod decoration;
pub mod layer_shell;
mod lock_idle;
mod selection;
mod xwayland;
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
    type KeyboardFocus = crate::focus::KeyboardFocusTarget;
    type PointerFocus = crate::focus::PointerFocusTarget;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Draftpaper> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: smithay::input::pointer::CursorImageStatus) {}

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&crate::focus::KeyboardFocusTarget>) {
        use smithay::wayland::seat::WaylandFocus;
        let dh = &self.display_handle;
        let client = focused.and_then(|f| f.wl_surface()).and_then(|s| dh.get_client(s.id()).ok());
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

    // a Wayland client's clipboard or primary selection, offered to X11 clients
    fn new_selection(
        &mut self,
        ty: smithay::wayland::selection::SelectionTarget,
        source: Option<smithay::wayland::selection::SelectionSource>,
        _seat: Seat<Self>,
    ) {
        if let Some(xwm) = self.xwm.as_mut() {
            let _ = xwm.new_selection(ty, source.map(|s| s.mime_types()));
        }
    }

    fn send_selection(
        &mut self,
        ty: smithay::wayland::selection::SelectionTarget,
        mime_type: String,
        fd: std::os::fd::OwnedFd,
        _seat: Seat<Self>,
        _user_data: &(),
    ) {
        let handle = self.loop_handle.clone();
        if let Some(xwm) = self.xwm.as_mut() {
            let _ = xwm.send_selection(ty, mime_type, fd, handle);
        }
    }
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

// linux-dmabuf: a client's GPU buffer is accepted when the renderer can import it
impl smithay::wayland::dmabuf::DmabufHandler for Draftpaper {
    fn dmabuf_state(&mut self) -> &mut smithay::wayland::dmabuf::DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &smithay::wayland::dmabuf::DmabufGlobal,
        dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
        notifier: smithay::wayland::dmabuf::ImportNotifier,
    ) {
        if self.tty_data.as_mut().is_some_and(|t| t.import_dmabuf(&dmabuf)) {
            let _ = notifier.successful::<Draftpaper>();
        } else {
            notifier.failed();
        }
    }
}
smithay::delegate_dmabuf!(Draftpaper);
