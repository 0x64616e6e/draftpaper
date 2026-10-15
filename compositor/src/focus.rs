//! What the keyboard, the pointer and a finger can be on. The keyboard: a window (Wayland or X11; an
//! X11 window needs the window manager to set X input focus, which Smithay's X11Surface target does) or
//! a bare surface (a layer surface such as a launcher, the lock surface). The pointer, and a finger on
//! the touchscreen: a Wayland surface or an X11 window. Each delegates to Smithay's own targets.
//! Follows anvil's focus.rs (Smithay v0.7.0, MIT).

use std::borrow::Cow;

use smithay::{
    backend::input::KeyState,
    desktop::{Window, WindowSurface},
    input::{
        keyboard::{KeyboardTarget, KeysymHandle, ModifiersState},
        pointer::{
            AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
            GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
            GestureSwipeUpdateEvent, MotionEvent, PointerTarget, RelativeMotionEvent,
        },
        touch::{self, TouchTarget},
        Seat,
    },
    reexports::wayland_server::{backend::ObjectId, protocol::wl_surface::WlSurface},
    utils::{IsAlive, Serial},
    wayland::seat::WaylandFocus,
    xwayland::X11Surface,
};

use crate::Draftpaper;

#[derive(Debug, Clone, PartialEq)]
pub enum KeyboardFocusTarget {
    Window(Window),
    Surface(WlSurface),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PointerFocusTarget {
    Surface(WlSurface),
    X11(X11Surface),
}

// popup grabs (xdg_shell.rs): the popup takes the keyboard, the pointer follows the keyboard's target
impl From<smithay::desktop::PopupKind> for KeyboardFocusTarget {
    fn from(popup: smithay::desktop::PopupKind) -> Self {
        KeyboardFocusTarget::Surface(popup.wl_surface().clone())
    }
}

impl From<KeyboardFocusTarget> for PointerFocusTarget {
    fn from(target: KeyboardFocusTarget) -> Self {
        match target {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(t) => PointerFocusTarget::Surface(t.wl_surface().clone()),
                WindowSurface::X11(x) => PointerFocusTarget::X11(x.clone()),
            },
            KeyboardFocusTarget::Surface(s) => PointerFocusTarget::Surface(s),
        }
    }
}

impl IsAlive for KeyboardFocusTarget {
    fn alive(&self) -> bool {
        match self {
            KeyboardFocusTarget::Window(w) => w.alive(),
            KeyboardFocusTarget::Surface(s) => s.alive(),
        }
    }
}

impl IsAlive for PointerFocusTarget {
    fn alive(&self) -> bool {
        match self {
            PointerFocusTarget::Surface(s) => s.alive(),
            PointerFocusTarget::X11(x) => x.alive(),
        }
    }
}

impl WaylandFocus for KeyboardFocusTarget {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            KeyboardFocusTarget::Window(w) => w.wl_surface(),
            KeyboardFocusTarget::Surface(s) => Some(Cow::Borrowed(s)),
        }
    }
}

impl WaylandFocus for PointerFocusTarget {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            PointerFocusTarget::Surface(s) => Some(Cow::Borrowed(s)),
            PointerFocusTarget::X11(x) => x.wl_surface().map(Cow::Owned),
        }
    }
    fn same_client_as(&self, object_id: &ObjectId) -> bool {
        match self {
            PointerFocusTarget::Surface(s) => s.same_client_as(object_id),
            PointerFocusTarget::X11(x) => x.same_client_as(object_id),
        }
    }
}

/// Call the same method on whichever target this is.
macro_rules! keyboard {
    ($self:ident, $method:ident, $($arg:expr),*) => {
        match $self {
            KeyboardFocusTarget::Window(w) => match w.underlying_surface() {
                WindowSurface::Wayland(t) => KeyboardTarget::$method(t.wl_surface(), $($arg),*),
                WindowSurface::X11(x) => KeyboardTarget::$method(x, $($arg),*),
            },
            KeyboardFocusTarget::Surface(s) => KeyboardTarget::$method(s, $($arg),*),
        }
    };
}

impl KeyboardTarget<Draftpaper> for KeyboardFocusTarget {
    fn enter(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, keys: Vec<KeysymHandle<'_>>, serial: Serial) {
        keyboard!(self, enter, seat, data, keys, serial)
    }
    fn leave(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, serial: Serial) {
        keyboard!(self, leave, seat, data, serial)
    }
    fn key(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, key: KeysymHandle<'_>, state: KeyState, serial: Serial, time: u32) {
        keyboard!(self, key, seat, data, key, state, serial, time)
    }
    fn modifiers(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, modifiers: ModifiersState, serial: Serial) {
        keyboard!(self, modifiers, seat, data, modifiers, serial)
    }
}

macro_rules! pointer {
    ($self:ident, $method:ident, $($arg:expr),*) => {
        match $self {
            PointerFocusTarget::Surface(s) => PointerTarget::$method(s, $($arg),*),
            PointerFocusTarget::X11(x) => PointerTarget::$method(x, $($arg),*),
        }
    };
}

impl PointerTarget<Draftpaper> for PointerFocusTarget {
    fn enter(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &MotionEvent) {
        pointer!(self, enter, seat, data, event)
    }
    fn motion(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &MotionEvent) {
        pointer!(self, motion, seat, data, event)
    }
    fn relative_motion(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &RelativeMotionEvent) {
        pointer!(self, relative_motion, seat, data, event)
    }
    fn button(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &ButtonEvent) {
        pointer!(self, button, seat, data, event)
    }
    fn axis(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, frame: AxisFrame) {
        pointer!(self, axis, seat, data, frame)
    }
    fn frame(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper) {
        pointer!(self, frame, seat, data)
    }
    fn leave(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, serial: Serial, time: u32) {
        pointer!(self, leave, seat, data, serial, time)
    }
    fn gesture_swipe_begin(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &GestureSwipeBeginEvent) {
        pointer!(self, gesture_swipe_begin, seat, data, event)
    }
    fn gesture_swipe_update(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &GestureSwipeUpdateEvent) {
        pointer!(self, gesture_swipe_update, seat, data, event)
    }
    fn gesture_swipe_end(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &GestureSwipeEndEvent) {
        pointer!(self, gesture_swipe_end, seat, data, event)
    }
    fn gesture_pinch_begin(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &GesturePinchBeginEvent) {
        pointer!(self, gesture_pinch_begin, seat, data, event)
    }
    fn gesture_pinch_update(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &GesturePinchUpdateEvent) {
        pointer!(self, gesture_pinch_update, seat, data, event)
    }
    fn gesture_pinch_end(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &GesturePinchEndEvent) {
        pointer!(self, gesture_pinch_end, seat, data, event)
    }
    fn gesture_hold_begin(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &GestureHoldBeginEvent) {
        pointer!(self, gesture_hold_begin, seat, data, event)
    }
    fn gesture_hold_end(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &GestureHoldEndEvent) {
        pointer!(self, gesture_hold_end, seat, data, event)
    }
}

macro_rules! touch {
    ($self:ident, $method:ident, $($arg:expr),*) => {
        match $self {
            PointerFocusTarget::Surface(s) => TouchTarget::$method(s, $($arg),*),
            PointerFocusTarget::X11(x) => TouchTarget::$method(x, $($arg),*),
        }
    };
}

impl TouchTarget<Draftpaper> for PointerFocusTarget {
    fn down(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &touch::DownEvent, seq: Serial) {
        touch!(self, down, seat, data, event, seq)
    }
    fn up(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &touch::UpEvent, seq: Serial) {
        touch!(self, up, seat, data, event, seq)
    }
    fn motion(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &touch::MotionEvent, seq: Serial) {
        touch!(self, motion, seat, data, event, seq)
    }
    fn frame(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, seq: Serial) {
        touch!(self, frame, seat, data, seq)
    }
    fn cancel(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, seq: Serial) {
        touch!(self, cancel, seat, data, seq)
    }
    fn shape(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &touch::ShapeEvent, seq: Serial) {
        touch!(self, shape, seat, data, event, seq)
    }
    fn orientation(&self, seat: &Seat<Draftpaper>, data: &mut Draftpaper, event: &touch::OrientationEvent, seq: Serial) {
        touch!(self, orientation, seat, data, event, seq)
    }
}
