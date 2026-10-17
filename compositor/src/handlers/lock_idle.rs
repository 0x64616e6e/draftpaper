//! Locking and idle. ext-session-lock (Draftpaper's lock screen, qml/lock.qml; swaylock): while locked,
//! each output shows only its lock surface, over the bare sheet blurred (render.rs), and the lock surface has the keyboard; bindings other
//! than VT switches are off. One locker holds the lock: another is refused while it lives, and may take
//! over only once it has died (a locker that crashes leaves the session locked, on black). Only the
//! holder unlocks: Smithay calls `unlock` even for a lock that was refused, so the unlock is carried out
//! after the request, and only if the holder's lock object is gone (it destroys itself in unlocking). ext-idle-notify (swayidle): any input counts as activity. idle-inhibit: a
//! surface that inhibits (a playing video) keeps the session from going idle.

use smithay::{
    delegate_idle_inhibit, delegate_idle_notify, delegate_session_lock,
    reexports::wayland_server::Resource,
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
        if self.locked && self.lock_holder.as_ref().is_some_and(|h| h.is_alive()) {
            // already locked by a live locker: dropping the confirmation refuses this one (finished)
            return;
        }
        // a new lock, or a takeover after the locker died: its surfaces are gone with it
        self.lock_surfaces.clear();
        self.lock_holder = Some(confirmation.ext_session_lock().clone());
        self.locked = true;
        confirmation.lock();
    }

    fn unlock(&mut self) {
        let holder = self.lock_holder.clone();
        let _ = self.loop_handle.insert_idle(move |data| {
            // the holder unlocked: its lock object was destroyed by the request; a refused locker that
            // asks to unlock leaves the holder alive, and the session locked
            if data.lock_holder != holder || holder.as_ref().is_some_and(|h| h.is_alive()) {
                return;
            }
            data.lock_holder = None;
            data.locked = false;
            data.lock_surfaces.clear();
            let w = data.screens.get(data.active).and_then(|s| s.layout().focused());
            data.focus(w);
        });
    }

    fn new_surface(&mut self, surface: LockSurface, output: WlOutput) {
        // only the holder's surfaces: a refused locker must not cover the holder's or take the keyboard
        let holder = self.lock_holder.as_ref().and_then(|h| h.client());
        if holder.is_none() || surface.wl_surface().client() != holder {
            return;
        }
        let Some(output) = Output::from_resource(&output) else { return };
        let Some(geometry) = self.space.output_geometry(&output) else { return };
        surface.with_pending_state(|state| state.size = Some((geometry.size.w as u32, geometry.size.h as u32).into()));
        surface.send_configure();
        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, Some(crate::focus::KeyboardFocusTarget::Surface(surface.wl_surface().clone())), SERIAL_COUNTER.next_serial());
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
