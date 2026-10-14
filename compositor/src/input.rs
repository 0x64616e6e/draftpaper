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
    utils::SERIAL_COUNTER,
};

use smithay::backend::input::KeyState;

use crate::{config::Action, state::Draftpaper};

impl Draftpaper {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        let seat = self.seat.clone();
        self.idle_notifier_state.notify_activity(&seat);
        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);
                let pressed = event.state() == KeyState::Pressed;
                let tty = self.tty;
                let locked = self.locked;

                let action = self.seat.get_keyboard().unwrap().input::<Action, _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    time,
                    |state, mods, keysym| match pressed
                        .then(|| state.config.action(mods, keysym.modified_sym(), keysym.raw_latin_sym_or_raw_current_sym(), tty))
                        .flatten()
                        .filter(|a| !locked || matches!(a, Action::Vt(_)))
                    {
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
                    // a click focuses the window under the pointer; on the paper it keeps the focus
                    let _ = (&keyboard, serial);
                    if let Some((window, _loc)) = self
                        .space
                        .element_under(pointer.current_location())
                        .map(|(w, l)| (w.clone(), l))
                    {
                        self.focus(Some(window));
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
                let w = match self.neighbour(i, d).filter(|_| self.screens[i].layout().at_edge(d)) {
                    Some(n) => {
                        self.active = n;
                        self.screens[n].layout().focused()
                    }
                    None => self.screens[i].layout_mut().focus_dir(d),
                };
                self.focus(w);
            }
            Action::Move(d) => {
                let alone = self.screens[i].layout().at_edge(d);
                match self.neighbour(i, d).filter(|_| alone) {
                    Some(n) => {
                        // to the neighbouring screen, as a new column there
                        if let Some(w) = self.screens[i].layout().focused() {
                            let (g, gn) = (self.screens[i].grid, self.screens[n].grid);
                            self.screens[i].layout_mut().remove(&w, &g);
                            self.screens[n].layout_mut().add(w.clone(), &gn);
                            self.relayout();
                            self.focus(Some(w));
                        }
                    }
                    None => {
                        let grid = self.screens[i].grid;
                        self.screens[i].layout_mut().move_dir(d, &grid);
                        self.relayout();
                    }
                }
            }
            Action::Resize(d) => {
                self.screens[i].layout_mut().resize(d);
                self.relayout();
            }
            Action::Spawn(command) => {
                let _ = std::process::Command::new("sh").arg("-c").arg(command).spawn();
            }
            Action::Quit => self.loop_signal.stop(),
            Action::Screenshot => crate::tty::screenshot(self),
            Action::Workspace(n) => self.switch_to(n),
            Action::Fullscreen => {
                if let Some(w) = self.focused_window() {
                    let on = self.workspace_of(&w).is_some_and(|(i, k)| self.screens[i].workspaces[k].fullscreen.as_ref() != Some(&w));
                    self.set_fullscreen(&w, on);
                }
            }
            Action::SendTo(n) => self.send_to(n),
            Action::Vt(n) => {
                if let Some(session) = self.session.as_mut() {
                    use smithay::backend::session::Session;
                    let _ = session.change_vt(n);
                }
            }
            Action::Close => {
                if let Some(w) = self.focused_window() {
                    if let Some(t) = w.toplevel() {
                        t.send_close();
                    } else if let Some(x) = w.x11_surface() {
                        let _ = x.close();
                    }
                }
            }
        }
    }
}
