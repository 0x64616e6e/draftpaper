// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent, PointerMotionEvent,
        KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent,
    },
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::SERIAL_COUNTER,
};

use smithay::{backend::input::KeyState, input::keyboard::{xkb::keysyms, Keysym, ModifiersState}};

use crate::{layout::Direction, state::Draftpaper};

/// What a key binding does.
#[derive(Debug, Clone, Copy)]
pub enum Action {
    Focus(Direction),
    Move(Direction),
    Resize(Direction),
    Terminal,
    Close,
    Quit,
    /// switch to virtual terminal n (TTY only)
    Vt(i32),
}

/// The bindings, on Super on the TTY and on Alt when nested inside another desktop (whose window manager
/// keeps Super for itself): arrows (or h/j/k/l) focus, with Shift move the window, with Ctrl resize by a
/// zone; Return opens a terminal, Shift+q closes the focused window, Shift+e quits Draftpaper.
/// Ctrl+Alt+F1..F12 switch virtual terminals on the TTY.
fn binding(mods: &ModifiersState, sym: Keysym, tty: bool) -> Option<Action> {
    let raw = sym.raw();
    if tty && (keysyms::KEY_XF86Switch_VT_1..=keysyms::KEY_XF86Switch_VT_12).contains(&raw) {
        return Some(Action::Vt((raw - keysyms::KEY_XF86Switch_VT_1 + 1) as i32));
    }
    if !(if tty { mods.logo } else { mods.alt }) {
        return None;
    }
    let dir = match sym.raw() {
        keysyms::KEY_Left | keysyms::KEY_h | keysyms::KEY_H => Some(Direction::Left),
        keysyms::KEY_Right | keysyms::KEY_l | keysyms::KEY_L => Some(Direction::Right),
        keysyms::KEY_Up | keysyms::KEY_k | keysyms::KEY_K => Some(Direction::Up),
        keysyms::KEY_Down | keysyms::KEY_j | keysyms::KEY_J => Some(Direction::Down),
        _ => None,
    };
    match (dir, mods.shift, mods.ctrl) {
        (Some(d), false, false) => Some(Action::Focus(d)),
        (Some(d), true, false) => Some(Action::Move(d)),
        (Some(d), false, true) => Some(Action::Resize(d)),
        _ => match sym.raw() {
            keysyms::KEY_Return => Some(Action::Terminal),
            keysyms::KEY_q | keysyms::KEY_Q if mods.shift => Some(Action::Close),
            keysyms::KEY_e | keysyms::KEY_E if mods.shift => Some(Action::Quit),
            _ => None,
        },
    }
}

impl Draftpaper {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);
                let pressed = event.state() == KeyState::Pressed;
                let tty = self.tty;

                let action = self.seat.get_keyboard().unwrap().input::<Action, _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    time,
                    |_, mods, keysym| match pressed.then(|| binding(mods, keysym.modified_sym(), tty)).flatten() {
                        Some(action) => FilterResult::Intercept(action),
                        None => FilterResult::Forward,
                    },
                );
                if let Some(action) = action {
                    self.act(action);
                }
            }
            InputEvent::PointerMotion { event, .. } => {
                // mice, touchpads and the TrackPoint move by deltas: keep the pointer on the outputs
                let pointer = self.seat.get_pointer().unwrap();
                let mut pos = pointer.current_location() + event.delta();
                let bounds = self
                    .space
                    .outputs()
                    .filter_map(|o| self.space.output_geometry(o))
                    .reduce(|a, b| a.merge(b));
                if let Some(b) = bounds {
                    pos.x = pos.x.clamp(b.loc.x as f64, (b.loc.x + b.size.w - 1) as f64);
                    pos.y = pos.y.clamp(b.loc.y as f64, (b.loc.y + b.size.h - 1) as f64);
                }
                let under = self.surface_under(pos);
                let serial = SERIAL_COUNTER.next_serial();
                pointer.motion(self, under, &MotionEvent { location: pos, serial, time: event.time_msec() });
                pointer.frame(self);
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let output = self.space.outputs().next().unwrap();

                let output_geo = self.space.output_geometry(output).unwrap();

                let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();

                let serial = SERIAL_COUNTER.next_serial();

                let pointer = self.seat.get_pointer().unwrap();

                let under = self.surface_under(pos);

                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: pos,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerButton { event, .. } => {
                let pointer = self.seat.get_pointer().unwrap();
                let keyboard = self.seat.get_keyboard().unwrap();

                let serial = SERIAL_COUNTER.next_serial();

                let button = event.button_code();

                let button_state = event.state();

                if ButtonState::Pressed == button_state && !pointer.is_grabbed() {
                    if let Some((window, _loc)) = self
                        .space
                        .element_under(pointer.current_location())
                        .map(|(w, l)| (w.clone(), l))
                    {
                        self.space.raise_element(&window, true);
                        if let Some(i) = self.screen_of(&window) {
                            self.active = i;
                            self.screens[i].layout.focus_window(&window);
                        }
                        keyboard.set_focus(
                            self,
                            Some(window.toplevel().unwrap().wl_surface().clone()),
                            serial,
                        );
                        self.space.elements().for_each(|window| {
                            window.toplevel().unwrap().send_pending_configure();
                        });
                    } else {
                        self.space.elements().for_each(|window| {
                            window.set_activated(false);
                            window.toplevel().unwrap().send_pending_configure();
                        });
                        keyboard.set_focus(self, Option::<WlSurface>::None, serial);
                    }
                };

                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state: button_state,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event, .. } => {
                let source = event.source();

                let horizontal_amount = event
                    .amount(Axis::Horizontal)
                    .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.);
                let vertical_amount = event
                    .amount(Axis::Vertical)
                    .unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.);
                let horizontal_amount_discrete = event.amount_v120(Axis::Horizontal);
                let vertical_amount_discrete = event.amount_v120(Axis::Vertical);

                let mut frame = AxisFrame::new(event.time_msec()).source(source);
                if horizontal_amount != 0.0 {
                    frame = frame.value(Axis::Horizontal, horizontal_amount);
                    if let Some(discrete) = horizontal_amount_discrete {
                        frame = frame.v120(Axis::Horizontal, discrete as i32);
                    }
                }
                if vertical_amount != 0.0 {
                    frame = frame.value(Axis::Vertical, vertical_amount);
                    if let Some(discrete) = vertical_amount_discrete {
                        frame = frame.v120(Axis::Vertical, discrete as i32);
                    }
                }

                if source == AxisSource::Finger {
                    if event.amount(Axis::Horizontal) == Some(0.0) {
                        frame = frame.stop(Axis::Horizontal);
                    }
                    if event.amount(Axis::Vertical) == Some(0.0) {
                        frame = frame.stop(Axis::Vertical);
                    }
                }

                let pointer = self.seat.get_pointer().unwrap();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            _ => {}
        }
    }
}

impl Draftpaper {
    pub fn act(&mut self, action: Action) {
        let i = self.active;
        if i >= self.screens.len() && matches!(action, Action::Focus(_) | Action::Move(_) | Action::Resize(_)) {
            return;
        }
        match action {
            Action::Focus(d) => {
                // at the edge of a screen, on to the neighbouring one
                let w = match self.neighbour(i, d).filter(|_| self.screens[i].layout.at_edge(d)) {
                    Some(n) => {
                        self.active = n;
                        self.screens[n].layout.focused()
                    }
                    None => self.screens[i].layout.focus_dir(d),
                };
                self.focus(w);
            }
            Action::Move(d) => {
                let alone = self.screens[i].layout.at_edge(d);
                match self.neighbour(i, d).filter(|_| alone) {
                    Some(n) => {
                        // to the neighbouring screen, as a new column there
                        if let Some(w) = self.screens[i].layout.focused() {
                            let (g, gn) = (self.screens[i].grid, self.screens[n].grid);
                            self.screens[i].layout.remove(&w, &g);
                            self.screens[n].layout.add(w.clone(), &gn);
                            self.relayout();
                            self.focus(Some(w));
                        }
                    }
                    None => {
                        let grid = self.screens[i].grid;
                        self.screens[i].layout.move_dir(d, &grid);
                        self.relayout();
                    }
                }
            }
            Action::Resize(d) => {
                self.screens[i].layout.resize(d);
                self.relayout();
            }
            Action::Terminal => {
                let _ = std::process::Command::new("alacritty").spawn();
            }
            Action::Quit => self.loop_signal.stop(),
            Action::Vt(n) => {
                if let Some(session) = self.session.as_mut() {
                    use smithay::backend::session::Session;
                    let _ = session.change_vt(n);
                }
            }
            Action::Close => {
                if let Some(t) = self.screens.get(i).and_then(|s| s.layout.focused()).and_then(|w| w.toplevel().cloned()) {
                    t.send_close();
                }
            }
        }
    }
}
