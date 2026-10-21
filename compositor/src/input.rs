// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Device, DeviceCapability, Event, InputBackend,
        InputEvent, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent, TouchEvent,
    },
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
        touch,
    },
    utils::{Logical, Point, SERIAL_COUNTER},
};

use smithay::backend::input::KeyState;

use crate::{config::Action, state::Draftpaper};

impl Draftpaper {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        let seat = self.seat.clone();
        self.idle_notifier_state.notify_activity(&seat);
        // the displays were turned off: any input turns them on (swayidle's resume does too)
        if self.screens_off {
            crate::tty::set_power(self, true);
        }
        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);
                let pressed = event.state() == KeyState::Pressed;
                let tty = self.tty;
                // the login screen takes no bindings either: nothing is started before someone logs in
                let locked = self.locked || self.greeter;
                // a held binding repeats until a key goes up or down
                if let Some(token) = self.key_repeat.take() {
                    self.loop_handle.remove(token);
                }

                let action = self.seat.get_keyboard().unwrap().input::<Option<(Action, bool)>, _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    time,
                    |state, mods, keysym| {
                        // the leader is armed (Mod tapped): this key is read as though Mod were still
                        // down, and it does not reach whatever has the focus — the one place anything
                        // here takes a key away (docs/keyboard.md)
                        if pressed && state.leader {
                            if spends_leader(keysym.modified_sym()) {
                                let act = state.leader_key(mods, keysym.modified_sym(), keysym.raw_latin_sym_or_raw_current_sym(), tty);
                                // a leader never repeats: the key it was typed with is already up
                                return FilterResult::Intercept(act.map(|a| (a, false)));
                            }
                            // a modifier on its own does not spend it: Mod tap, Shift, Left is Mod+Shift+Left
                            return FilterResult::Intercept(None);
                        }
                        // the legend watches every key go by without taking any of them (docs/keyboard.md)
                        state.legend_key(pressed, keysym.modified_sym(), locked);
                        match pressed
                            .then(|| {
                                // the magnifier out: Escape puts it away
                                let escape = keysym.modified_sym() == smithay::input::keyboard::Keysym::Escape && !mods.ctrl && !mods.alt && !mods.logo && !mods.shift;
                                if state.lens.is_some() && escape {
                                    return Some(Action::Zoom);
                                }
                                state.config.action(mods, keysym.modified_sym(), keysym.raw_latin_sym_or_raw_current_sym(), tty)
                            })
                            .flatten()
                            .filter(|a| !locked || matches!(a, Action::Vt(_)))
                        {
                            Some(action) => {
                                // volume, brightness and resizing go on while the key is held
                                let name = smithay::input::keyboard::xkb::keysym_get_name(keysym.modified_sym());
                                let held = matches!(action, Action::Resize(_))
                                    || matches!(name.as_str(), "XF86AudioRaiseVolume" | "XF86AudioLowerVolume" | "XF86MonBrightnessUp" | "XF86MonBrightnessDown");
                                FilterResult::Intercept(Some((action, held)))
                            }
                            None => FilterResult::Forward,
                        }
                    },
                );
                if let Some((action, held)) = action.flatten() {
                    self.act(action.clone());
                    if held {
                        self.repeat(action);
                    }
                }
            }
            InputEvent::PointerMotion { event, .. } => {
                self.pointer_by(event.delta(), event.time_msec());
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
                self.follow_mouse(pos);
            }
            InputEvent::PointerButton { event, .. } => {
                // the leader is spent by reaching for the pointer: you have gone somewhere else, and a
                // mode left armed behind a click is the kind you forget you are in (docs/keyboard.md)
                self.drop_leader();
                let pointer = self.seat.get_pointer().unwrap();
                let keyboard = self.seat.get_keyboard().unwrap();

                let serial = SERIAL_COUNTER.next_serial();

                let button = event.button_code();

                let button_state = event.state();

                if ButtonState::Pressed == button_state && !pointer.is_grabbed() {
                    // a click focuses the window under the pointer; on the paper it keeps the focus. On a
                    // layer surface (the settings, the launcher, the shell's margins and title block, a
                    // panel's menu) the window under it keeps out of it: such a surface takes the keyboard
                    // if it reads keys, else the keyboard stays where it was. (Handing it to the window
                    // under an overlay made Qt cancel the press, and every second click was lost.)
                    let _ = (&keyboard, serial);
                    let pos = pointer.current_location();
                    match self.layer_under(pos) {
                        Some((surface, true)) => {
                            let serial = SERIAL_COUNTER.next_serial();
                            keyboard.set_focus(self, Some(crate::focus::KeyboardFocusTarget::Surface(surface)), serial);
                        }
                        Some((_, false)) => {}
                        None => {
                            if let Some(window) = self.space.element_under(pos).map(|(w, _)| w.clone()) {
                                self.focus(Some(window));
                            }
                        }
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
                // the magnifier out: the wheel zooms it, with Shift sizes it (the windows do not scroll)
                if self.lens.is_some() {
                    let notches = event
                        .amount_v120(Axis::Vertical)
                        .map(|v| v / 120.0)
                        .or_else(|| event.amount(Axis::Vertical).map(|a| a / 15.0))
                        .unwrap_or(0.0);
                    let shift = self.seat.get_keyboard().unwrap().modifier_state().shift;
                    if let Some(lens) = self.lens.as_mut() {
                        lens.scroll(notches, shift);
                    }
                    return;
                }

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
            // the seat has touch once a touchscreen is plugged in, and keeps it
            InputEvent::DeviceAdded { device } => {
                if device.has_capability(DeviceCapability::Touch) && self.seat.get_touch().is_none() {
                    self.seat.add_touch();
                }
            }
            // a finger down focuses the window under it, as a click does; the surface under it has the
            // finger until it lifts
            InputEvent::TouchDown { event } => {
                let (Some(touch), Some(pos)) = (self.seat.get_touch(), self.touch_location(&event)) else { return };
                if !touch.is_grabbed() {
                    if let Some(window) = self.space.element_under(pos).map(|(w, _)| w.clone()) {
                        self.focus(Some(window));
                    }
                }
                let under = self.surface_under(pos);
                let serial = SERIAL_COUNTER.next_serial();
                touch.down(self, under, &touch::DownEvent { slot: event.slot(), location: pos, serial, time: event.time_msec() });
            }
            InputEvent::TouchMotion { event } => {
                let (Some(touch), Some(pos)) = (self.seat.get_touch(), self.touch_location(&event)) else { return };
                let under = self.surface_under(pos);
                touch.motion(self, under, &touch::MotionEvent { slot: event.slot(), location: pos, time: event.time_msec() });
            }
            InputEvent::TouchUp { event } => {
                let Some(touch) = self.seat.get_touch() else { return };
                let serial = SERIAL_COUNTER.next_serial();
                touch.up(self, &touch::UpEvent { slot: event.slot(), serial, time: event.time_msec() });
            }
            InputEvent::TouchFrame { .. } => {
                if let Some(touch) = self.seat.get_touch() {
                    touch.frame(self);
                }
            }
            InputEvent::TouchCancel { .. } => {
                if let Some(touch) = self.seat.get_touch() {
                    touch.cancel(self);
                }
            }
            _ => {}
        }
    }
}

impl Draftpaper {
    /// Focus follows the mouse, as in i3: the window the pointer enters takes the focus; moving onto
    /// another screen's paper makes that screen active, with its own focused window. Only on entering
    /// (moving within the focused window, or after a new window took the focus, changes nothing), and
    /// not while a menu or a drag holds the pointer, a layer surface (the launcher) or the lock has the
    /// keyboard, or for unmanaged X11 windows (menus, tooltips).
    fn follow_mouse(&mut self, pos: smithay::utils::Point<f64, smithay::utils::Logical>) {
        let window = self.space.element_under(pos).map(|(w, _)| w.clone()).filter(|w| self.workspace_of(w).is_some());
        let screen = self.screen_at(pos);
        let entered_window = window.is_some() && window != self.pointer_over.0;
        let entered_screen = window.is_none() && screen.is_some() && screen != self.pointer_over.1;
        self.pointer_over = (window.clone(), screen);
        if !self.config.input.focus_follows_mouse || self.locked || self.seat.get_pointer().is_some_and(|p| p.is_grabbed()) {
            return;
        }
        if matches!(self.seat.get_keyboard().and_then(|k| k.current_focus()), Some(crate::focus::KeyboardFocusTarget::Surface(s)) if smithay::utils::IsAlive::alive(&s)) {
            return;
        }
        if entered_window {
            if self.focused_window() != window {
                self.focus(window);
            }
        } else if entered_screen {
            let i = screen.unwrap();
            if i != self.active {
                self.active = i;
                let w = self.screens[i].layout().focused();
                self.focus(w);
            }
        }
    }

    /// Where a finger is: the touchscreen is the laptop's own panel (or, with none, the first output),
    /// and the panel's corners turn with the output's transform.
    fn touch_location<I: InputBackend, E: AbsolutePositionEvent<I>>(&self, event: &E) -> Option<Point<f64, Logical>> {
        let internal = |o: &&smithay::output::Output| ["eDP", "LVDS", "DSI"].iter().any(|p| o.name().starts_with(p));
        let output = self.space.outputs().find(internal).or_else(|| self.space.outputs().next())?;
        let geometry = self.space.output_geometry(output)?;
        let transform = output.current_transform();
        let size = transform.invert().transform_size(geometry.size);
        Some(transform.transform_point_in(event.position_transformed(size), &size.to_f64()) + geometry.loc.to_f64())
    }

    /// Run `action` again while its key is held: after the keyboard's repeat delay, then at its rate.
    /// The legend's gesture (docs/keyboard.md): Mod held **alone** raises the keymap over the sheet once
    /// `[keys] legend_after` has gone by, and anything else at all puts it away again.
    ///
    /// It takes no key. Mod is forwarded as it always was, so an application that wants it still has it,
    /// and the first key of a chord cancels the timer — which is why a slow `Mod+Shift+i` never flashes
    /// the legend on its way to the picker. Nothing here is a mode: the key is held, so it cannot be
    /// forgotten (Raskin's quasimode, the argument the design doc sets out).
    fn legend_key(&mut self, pressed: bool, sym: smithay::input::keyboard::Keysym, locked: bool) {
        let usable = !locked && !self.greeter;
        if arms_legend(pressed, sym, usable) {
            self.arm_legend();
            return;
        }
        // Mod let go before the hold's time was up, with nothing in between: a tap, and the leader arms
        // (docs/keyboard.md). Held past it the legend is already out, and letting go puts it away as it
        // always did — the same gesture, told apart by whether the wait is still running.
        if taps_leader(pressed, sym, self.legend_timer.is_some(), self.legend, usable) {
            self.drop_legend();
            self.arm_leader();
            return;
        }
        self.drop_legend();
    }

    /// Mod tapped: the legend comes up and stays, and the next key composes with it. There is no silent
    /// leader — a mode you cannot see is the one Raskin's argument is against — so this arms nothing
    /// when the legend is turned off ([keys] legend_after = 0), which the pending timer already settles.
    fn arm_leader(&mut self) {
        self.leader = true;
        self.legend = true;
        self.touched = true;
    }

    pub fn drop_leader(&mut self) {
        if self.leader {
            self.leader = false;
            self.legend = false;
            self.touched = true;
        }
    }

    /// The key typed after the leader. It always disarms: one key, then the leader is spent, whether it
    /// was bound or not. `Esc` leaves without running anything, as it must for a mode to be admissible.
    /// A key with no binding does nothing — and, for now, does not say so (docs/keyboard.md).
    fn leader_key(
        &mut self,
        mods: &smithay::input::keyboard::ModifiersState,
        sym: smithay::input::keyboard::Keysym,
        raw: Option<smithay::input::keyboard::Keysym>,
        tty: bool,
    ) -> Option<Action> {
        self.drop_leader();
        if sym == smithay::input::keyboard::Keysym::Escape {
            return None;
        }
        self.config.led(mods, sym, raw, tty)
    }

    fn arm_legend(&mut self) {
        use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
        let Some(after) = self.config.legend_after else { return };
        if self.legend || self.legend_timer.is_some() {
            return;
        }
        self.legend_timer = self
            .loop_handle
            .insert_source(Timer::from_duration(after), move |_, _, data: &mut Self| {
                data.legend = true;
                data.legend_timer = None;
                data.touched = true;        // whoever letters the sheet hears of it in the next outputs
                TimeoutAction::Drop
            })
            .ok();
    }

    fn drop_legend(&mut self) {
        if let Some(token) = self.legend_timer.take() {
            self.loop_handle.remove(token);
        }
        if self.legend {
            self.legend = false;
            self.touched = true;
        }
    }

    fn repeat(&mut self, action: Action) {
        use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
        let delay = std::time::Duration::from_millis(self.config.input.repeat_delay.max(1) as u64);
        let every = std::time::Duration::from_millis(1000 / self.config.input.repeat_rate.max(1) as u64);
        self.key_repeat = self
            .loop_handle
            .insert_source(Timer::from_duration(delay), move |_, _, data| {
                data.touched = true;
                data.act(action.clone());
                TimeoutAction::ToDuration(every)
            })
            .ok();
    }

    /// The pointer put at `pos` (layout, logical): the `pointer` action, and a workspace switch to another
    /// screen (screens.rs). What is under it learns of it as of any motion.
    pub fn warp_pointer(&mut self, pos: smithay::utils::Point<f64, smithay::utils::Logical>) {
        let pointer = self.seat.get_pointer().unwrap();
        let under = self.surface_under(pos);
        let serial = SERIAL_COUNTER.next_serial();
        pointer.motion(self, under, &MotionEvent { location: pos, serial, time: self.start_time.elapsed().as_millis() as u32 });
        pointer.frame(self);
        // the rulers' steps count again from here
        self.pointer_raw = None;
    }

    /// The pointer moved by `delta` (mice, touchpads, the TrackPoint; `pointer_by` for scripts and tests):
    /// kept on the outputs, and while the rulers are out held to their steps (rulers.rs), from where the
    /// motion itself took it, so that small movements add up.
    pub fn pointer_by(&mut self, delta: smithay::utils::Point<f64, smithay::utils::Logical>, time: u32) {
        let pointer = self.seat.get_pointer().unwrap();
        let from = self.pointer_raw.filter(|_| self.rulers.is_some()).unwrap_or_else(|| pointer.current_location());
        let mut raw = from + delta;
        let bounds = self.space.outputs().filter_map(|o| self.space.output_geometry(o)).reduce(|a, b| a.merge(b));
        let clamp = |p: &mut smithay::utils::Point<f64, smithay::utils::Logical>| {
            if let Some(b) = bounds {
                p.x = p.x.clamp(b.loc.x as f64, (b.loc.x + b.size.w - 1) as f64);
                p.y = p.y.clamp(b.loc.y as f64, (b.loc.y + b.size.h - 1) as f64);
            }
        };
        clamp(&mut raw);
        let mut pos = raw;
        if let Some(r) = self.rulers.as_ref().filter(|r| r.snap) {
            // on the rulers' output only
            let on = self.space.outputs().find(|o| o.name() == r.output).and_then(|o| self.space.output_geometry(o)).is_some_and(|g| g.to_f64().contains(raw));
            if on {
                let coarse = self.seat.get_keyboard().is_some_and(|k| k.modifier_state().shift);
                pos = crate::rulers::snap(raw, r, coarse);
                clamp(&mut pos);
            }
        }
        self.pointer_raw = self.rulers.is_some().then_some(raw);
        let under = self.surface_under(pos);
        let serial = SERIAL_COUNTER.next_serial();
        pointer.motion(self, under, &MotionEvent { location: pos, serial, time });
        pointer.frame(self);
        self.follow_mouse(pos);
    }

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
                // a window against the frame that way goes on to the neighbouring screen (Layout::move_dir
                // moves it about this one)
                let off = self.screens[i].layout().against_frame(d);
                match self.neighbour(i, d).filter(|_| off) {
                    Some(n) => {
                        // to the neighbouring screen, placed there as a new window
                        if let Some(w) = self.screens[i].layout().focused() {
                            let (g, gn) = (self.screens[i].grid, self.screens[n].grid);
                            self.screens[i].layout_mut().remove(&w, &g);
                            if !self.screens[n].layout_mut().add(w.clone(), &gn) {
                                // every zone taken there (Layout::add): it floats
                                self.screens[n].workspace_mut().floating.push(w.clone());
                            }
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
                let grid = self.screens[i].grid;
                self.screens[i].layout_mut().resize(d, &grid);
                self.relayout();
            }
            Action::Spawn(command) => {
                let _ = std::process::Command::new("sh").arg("-c").arg(command).spawn();
            }
            // the session as it is, for the next login (session.rs)
            Action::Quit => {
                self.session_save(false);
                self.loop_signal.stop();
            }
            Action::SessionSave => self.session_save(true),
            Action::Screenshot => crate::tty::screenshot(self),
            Action::PointerBy(dx, dy) => self.pointer_by(smithay::utils::Point::from((dx, dy)), self.start_time.elapsed().as_millis() as u32),
            Action::Pointer(x, y) => self.warp_pointer(smithay::utils::Point::from((x, y))),
            Action::Button(button, down) => {
                let pointer = self.seat.get_pointer().unwrap();
                let state = if down { smithay::backend::input::ButtonState::Pressed } else { smithay::backend::input::ButtonState::Released };
                let event = ButtonEvent { button, state, serial: SERIAL_COUNTER.next_serial(), time: self.start_time.elapsed().as_millis() as u32 };
                pointer.button(self, &event);
                pointer.frame(self);
            }
            Action::Zoom => {
                self.lens = match self.lens.take() {
                    Some(_) => None,
                    None => Some(crate::lens::Lens::default()),
                };
            }
            Action::Park => self.park_toggle(),
            Action::ParkAt(cell, ws) => self.park_at(cell, ws),
            Action::Unpark(id) => self.unpark(id),
            Action::ParkCycle(d) => self.park_cycle(d),
            Action::Power(on) => crate::tty::set_power(self, on),
            Action::Reload => {
                // as if the file had changed
                self.config_mtime = Some(std::time::UNIX_EPOCH);
                self.reload_config();
            }
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

/// Whether a key going by arms the legend's wait or puts it away (docs/keyboard.md).
///
/// Mod pressed on its own starts the count; **everything else stops it**, including Mod's own release,
/// which is what makes the gesture safe to put on a modifier. The first key of a chord lands here before
/// the binding runs, so `Mod+Shift+i` cancels the wait on the Shift and the legend never flashes on its
/// way to the picker. `usable` is false on the lock and login screens, where nothing is bound anyway.
fn arms_legend(pressed: bool, sym: smithay::input::keyboard::Keysym, usable: bool) -> bool {
    use smithay::input::keyboard::Keysym;
    // Mod is Super on the TTY and Alt when nested (config.rs), so the legend answers to both
    let is_mod = matches!(sym, Keysym::Super_L | Keysym::Super_R | Keysym::Alt_L | Keysym::Alt_R);
    pressed && is_mod && usable
}

/// Whether Mod going up is a **tap** — the gesture that arms the leader (docs/keyboard.md).
///
/// It is a tap when the hold's wait is still running: Mod went down, nothing came between (anything
/// that did would have cancelled the wait), and it came up before the legend was lettered. Held past
/// that and `legend` is already true, which is the hold ending, not a tap. One gesture on one key,
/// told apart by the clock that was already there.
fn taps_leader(pressed: bool, sym: smithay::input::keyboard::Keysym, waiting: bool, legend_up: bool, usable: bool) -> bool {
    use smithay::input::keyboard::Keysym;
    let is_mod = matches!(sym, Keysym::Super_L | Keysym::Super_R | Keysym::Alt_L | Keysym::Alt_R);
    !pressed && is_mod && waiting && !legend_up && usable
}

/// Whether a key typed with the leader armed spends it. A modifier on its own does not: `Mod` tapped,
/// then Shift, then Left must reach `Mod+Shift+Left`, so the leader waits for the key being modified.
/// Everything else spends it, bound or not.
fn spends_leader(sym: smithay::input::keyboard::Keysym) -> bool {
    use smithay::input::keyboard::Keysym;
    !matches!(
        sym,
        Keysym::Shift_L | Keysym::Shift_R | Keysym::Control_L | Keysym::Control_R | Keysym::Alt_L | Keysym::Alt_R
            | Keysym::Super_L | Keysym::Super_R | Keysym::Meta_L | Keysym::Meta_R
            | Keysym::ISO_Level3_Shift | Keysym::Caps_Lock | Keysym::Num_Lock
    )
}

#[cfg(test)]
mod tests {
    use super::{arms_legend, spends_leader, taps_leader};
    use smithay::input::keyboard::Keysym;

    #[test]
    fn mod_let_go_before_the_legend_is_a_tap_and_after_it_is_not() {
        assert!(taps_leader(false, Keysym::Super_L, true, false, true), "let go while the wait runs: a tap");
        assert!(!taps_leader(false, Keysym::Super_L, false, true, true), "let go with the legend out: the hold ending");
        assert!(!taps_leader(false, Keysym::Super_L, false, false, true), "no wait and no legend: the wait was cancelled by a chord");
        assert!(!taps_leader(true, Keysym::Super_L, true, false, true), "Mod going down is not a tap");
        assert!(!taps_leader(false, Keysym::h, true, false, true), "nor is any other key");
        assert!(!taps_leader(false, Keysym::Super_L, true, false, false), "nothing is bound on the lock screen");
    }

    #[test]
    fn a_modifier_does_not_spend_the_leader_but_a_key_does() {
        assert!(!spends_leader(Keysym::Shift_L), "Mod tap, Shift, Left must reach Mod+Shift+Left");
        assert!(!spends_leader(Keysym::Control_R));
        assert!(!spends_leader(Keysym::Super_L));
        assert!(spends_leader(Keysym::h), "a key spends it");
        assert!(spends_leader(Keysym::Escape), "Escape spends it, and runs nothing");
        assert!(spends_leader(Keysym::Left));
    }

    #[test]
    fn mod_held_alone_arms_the_legend_and_anything_else_puts_it_away() {
        assert!(arms_legend(true, Keysym::Super_L, true), "Mod pressed starts the wait");
        assert!(arms_legend(true, Keysym::Alt_L, true), "nested, Mod is Alt");
        assert!(!arms_legend(false, Keysym::Super_L, true), "letting Mod go puts it away");
        assert!(!arms_legend(true, Keysym::h, true), "another key cancels: a chord is on its way");
        assert!(!arms_legend(true, Keysym::Shift_L, true), "the second modifier of a chord cancels too");
        assert!(!arms_legend(true, Keysym::Super_L, false), "not on the lock or login screen");
    }
}
