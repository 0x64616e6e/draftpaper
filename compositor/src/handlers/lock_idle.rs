//! Locking and idle. ext-session-lock (swaylock, Quickshell's WlSessionLock): while locked, each
//! output shows only its lock surface on black, and the lock surface has the keyboard; bindings other
//! than VT switches are off. ext-idle-notify (swayidle): any input counts as activity. idle-inhibit: a
//! surface that inhibits (a playing video) keeps the session from going idle.

use smithay::{
    delegate_idle_inhibit, delegate_idle_notify, delegate_session_lock,
    output::Output,
    reexports::wayland_server::protocol::{wl_output::WlOutput, wl_surface::WlSurface},
    utils::SERIAL_COUNTER,
    wayland::{
        idle_inhibit::IdleInhibitHandler,
        idle_notify::{IdleNotifierHandler, IdleNotifierState},
        session_lock::{LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker},
    },
};

use crate::Draftpaper;

impl SessionLockHandler for Draftpaper {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock_state
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        self.locked = true;
        confirmation.lock();
    }

    fn unlock(&mut self) {
        self.locked = false;
        self.lock_surfaces.clear();
        let w = self.screens.get(self.active).and_then(|s| s.layout().focused());
        self.focus(w);
    }

    fn new_surface(&mut self, surface: LockSurface, output: WlOutput) {
        let Some(output) = Output::from_resource(&output) else { return };
        let Some(geometry) = self.space.output_geometry(&output) else { return };
        surface.with_pending_state(|state| state.size = Some((geometry.size.w as u32, geometry.size.h as u32).into()));
        surface.send_configure();
        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, Some(surface.wl_surface().clone()), SERIAL_COUNTER.next_serial());
        self.lock_surfaces.insert(output.name(), surface);
    }
}
delegate_session_lock!(Draftpaper);

impl IdleNotifierHandler for Draftpaper {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle_notifier_state
    }
}
delegate_idle_notify!(Draftpaper);

impl IdleInhibitHandler for Draftpaper {
    fn inhibit(&mut self, surface: WlSurface) {
        self.inhibitors.push(surface);
        self.idle_notifier_state.set_is_inhibited(true);
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        self.inhibitors.retain(|s| s != &surface);
        let inhibited = !self.inhibitors.is_empty();
        self.idle_notifier_state.set_is_inhibited(inhibited);
    }
}
delegate_idle_inhibit!(Draftpaper);
