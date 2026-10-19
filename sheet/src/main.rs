//! `draftpaper-sheet`: the sheet's lettering, drawn by the sheet itself rather than by a toolkit
//! (docs/sheet-client.md). One layer surface per margin per display, below every window, drawn on the
//! processor into shared memory — no drawing context on the GPU, no scene graph, no threads a window.
//!
//! Stage 0: the surfaces, in their places, at exact physical pixels. Nothing is lettered yet; each
//! margin carries one rule a physical pixel wide, 50 mm from the frame's corner, which is what the test
//! measures (compositor/tests/sheet.sh). Fractional scale is the whole risk of this client and so is
//! proved first: the buffer is in physical pixels and a viewport maps it to the logical size, so a rule
//! meant to be one pixel wide is one pixel wide at 1.25 and 1.75 as much as at 1.
//!
//! The geometry is the compositor's (sheet.rs); this client works out none of it.

mod font;
mod graph;
mod margins;
mod notices;
mod schedule;
mod paint;
mod sheet;
mod title;

use std::collections::HashMap;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer},
        Capability, SeatHandler, SeatState,
    },
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{Anchor, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::{registry_queue_init, GlobalList},
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{wp_fractional_scale_manager_v1 as scale_manager, wp_fractional_scale_v1 as fractional},
    viewporter::client::{wp_viewport, wp_viewporter},
};

use sheet::Screen;
use smithay_client_toolkit::reexports::csd_frame::CursorIcon;

/// Linux's code for the left button, which is what a click on the sheet is.
const BTN_LEFT: u32 = 0x110;

/// Which margin a surface is, and how it lies along the sheet's edge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
    /// the block in the frame's bottom-right corner, lettered from the compositor's own cells
    Title,
    /// whichever schedule is out, in the title block's column
    Schedule,
    /// a year of a reading, level with the title block
    Graph,
}

impl Side {
    const ALL: [Side; 7] = [Side::Top, Side::Bottom, Side::Left, Side::Right, Side::Title, Side::Schedule, Side::Graph];

    fn anchor(self) -> Anchor {
        match self {
            // the top and the bottom run the whole width; the sides the height between them
            Side::Top => Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
            Side::Bottom => Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            Side::Left => Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM,
            Side::Right => Anchor::RIGHT | Anchor::TOP | Anchor::BOTTOM,
            Side::Title => Anchor::BOTTOM | Anchor::RIGHT,
            Side::Schedule => Anchor::TOP | Anchor::LEFT,
            Side::Graph => Anchor::TOP | Anchor::LEFT,
        }
    }

    pub fn vertical(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }

    fn namespace(self) -> &'static str {
        match self {
            Side::Title => "draftpaper-title-block",
            Side::Schedule => "draftpaper-schedule",
            Side::Graph => "draftpaper-load",
            _ => "draftpaper-margin",
        }
    }
}

/// One margin: its surface, what the compositor last asked for, and the scale it is shown at.
struct Band {
    side: Side,
    screen: String,
    layer: LayerSurface,
    viewport: wp_viewport::WpViewport,
    _scale: Option<fractional::WpFractionalScaleV1>,
    /// the size the compositor configured, in logical pixels
    size: (u32, u32),
    /// 120ths of a scale, as wp_fractional_scale counts it; 120 is scale 1
    scale120: u32,
    drawn: bool,
}

struct Sheet {
    registry: RegistryState,
    outputs: OutputState,
    shm: Shm,
    compositor: CompositorState,
    shell: LayerShell,
    viewporter: wp_viewporter::WpViewporter,
    scaling: Option<scale_manager::WpFractionalScaleManagerV1>,
    /// taken out of the way while a surface is drawn, so the lettering may borrow the rest
    pool: Option<SlotPool>,
    bands: Vec<Band>,
    /// the sheet as the compositor works it out, by display
    screens: HashMap<String, Screen>,
    /// the outputs Wayland knows, by the name the compositor gives them
    known: HashMap<String, wl_output::WlOutput>,
    theme: sheet::Theme,
    face: Option<font::Face>,
    /// what the machine last said of itself, and the processor's share since the reading before
    readings: serde_json::Value,
    cpu: draftpaper::metrics::Cpu,
    busy: Option<f64>,
    /// the workspaces (number, focused, shown, display) and what is parked on the one in view
    spaces: Vec<(u64, bool, bool, String)>,
    parked: Vec<(usize, String, u64)>,
    seats: SeatState,
    pointer: Option<ThemedPointer>,
    /// the surface that is showing a mark under the pointer, so it can be drawn plain again
    /// what the pointer is over: the surface and where in it, so the lettering under it can answer
    over: Option<(wl_surface::WlSurface, f64, f64)>,
    marked: Option<wl_surface::WlSurface>,
    levels: sheet::Levels,
    /// which schedule is out (Mod+i takes the next), and the one worked out for it
    showing: schedule::Which,
    schedule: Option<schedule::Schedule>,
    /// the reading plotted, and the store it is plotted from
    plotting: graph::Domain,
    readings_over_time: Option<graph::Readings>,
    notices: std::sync::Arc<notices::Notices>,
    /// when the backlight was last seen to move
    brightness_at: u64,
    exit: bool,
}

fn main() {
    let conn = match Connection::connect_to_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("draftpaper-sheet: no Wayland display: {e}");
            std::process::exit(1);
        }
    };
    let (globals, mut queue) = registry_queue_init(&conn).expect("the Wayland registry");
    let qh = queue.handle();

    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let theme = sheet::Theme::load();
    let face = font::Face::load(theme.mono.as_deref());
    if face.is_none() {
        eprintln!("draftpaper-sheet: no lettering face found: the sheet will carry no words");
    }
    let mut state = Sheet {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        compositor: CompositorState::bind(&globals, &qh).expect("wl_compositor"),
        shell: LayerShell::bind(&globals, &qh).expect("wlr-layer-shell"),
        viewporter: bind(&globals, &qh).expect("wp_viewporter"),
        scaling: bind(&globals, &qh),
        pool: Some(SlotPool::new(1 << 20, &shm).expect("a pool of shared memory")),
        shm,
        bands: Vec::new(),
        screens: HashMap::new(),
        known: HashMap::new(),
        theme,
        face,
        readings: serde_json::Value::Null,
        cpu: draftpaper::metrics::Cpu::new(),
        busy: None,
        spaces: Vec::new(),
        parked: Vec::new(),
        seats: SeatState::new(&globals, &qh),
        pointer: None,
        over: None,
        marked: None,
        levels: sheet::Levels::default(),
        showing: schedule::Which::None,
        schedule: None,
        plotting: graph::Domain::None,
        readings_over_time: None,
        notices: std::sync::Arc::new(notices::Notices::default()),
        brightness_at: 0,
        exit: false,
    };
    if state.scaling.is_none() {
        eprintln!("draftpaper-sheet: no wp_fractional_scale: drawing at whole scales only");
    }

    // the displays Wayland already has, before the first round trip
    queue.roundtrip(&mut state).expect("the first round trip");
    state.reread(&qh);
    state.reread_machine();
    state.reread_margins(&qh);
    state.levels = sheet::Levels::read(state.readings.get("backlight").and_then(serde_json::Value::as_str));

    // the machine is read again every five seconds, as the shell read it, which is also what moves the
    // clock in the title block
    let mut events = calloop::EventLoop::<Sheet>::try_new().expect("an event loop");
    let handle = events.handle();
    handle
        .insert_source(calloop::timer::Timer::from_duration(READ_EVERY), |_, _, state: &mut Sheet| {
            state.reread_machine();
            calloop::timer::TimeoutAction::ToDuration(READ_EVERY)
        })
        .expect("the reading timer");
    // the volume and the backlight move between readings, and the sheet says so while they do
    handle
        .insert_source(calloop::timer::Timer::from_duration(LEVELS_EVERY), |_, _, state: &mut Sheet| {
            state.reread_levels();
            calloop::timer::TimeoutAction::ToDuration(LEVELS_EVERY)
        })
        .expect("the levels timer");
    // the desktop's notification server: what comes in is lettered along the top margin
    let (told, notified) = std::sync::mpsc::channel();
    let held = state.notices.clone();
    std::thread::spawn(move || notices::serve(held, told));
    let (noticed, notices_in) = calloop::channel::channel::<()>();
    std::thread::spawn(move || {
        while notified.recv().is_ok() {
            if noticed.send(()).is_err() {
                return;
            }
        }
    });
    handle
        .insert_source(notices_in, move |_, _, state: &mut Sheet| state.redraw_top())
        .expect("the notifications");
    // one that has had its while comes off the sheet
    handle
        .insert_source(calloop::timer::Timer::from_duration(NOTICE_EVERY), |_, _, state: &mut Sheet| {
            state.redraw_top();
            calloop::timer::TimeoutAction::ToDuration(NOTICE_EVERY)
        })
        .expect("the notice timer");

    // the compositor says when a workspace is switched or a window parked, so the margins follow at once
    let (say, heard) = std::sync::mpsc::channel();
    std::thread::spawn(move || sheet::watch(say));
    let (ping, pings) = calloop::channel::channel::<()>();
    std::thread::spawn(move || {
        while heard.recv().is_ok() {
            if ping.send(()).is_err() {
                return;
            }
        }
    });
    let qh2 = qh.clone();
    handle
        .insert_source(pings, move |_, _, state: &mut Sheet| state.reread_margins(&qh2))
        .expect("the compositor's own word");
    calloop_wayland_source::WaylandSource::new(conn, queue).insert(handle).expect("the Wayland source");
    let _ = events.run(None, &mut state, |state| {
        state.exit |= state.bands.is_empty() && state.screens.is_empty();
    });
}

/// How often the machine is read again for the title block.
const READ_EVERY: std::time::Duration = std::time::Duration::from_secs(5);
/// How often a notification is looked at for having had its while.
const NOTICE_EVERY: std::time::Duration = std::time::Duration::from_secs(1);
/// How often the volume and the backlight are looked at: two small files, as the shell looked at them.
const LEVELS_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// A global, where the compositor has it.
fn bind<I, S>(globals: &GlobalList, qh: &QueueHandle<S>) -> Option<I>
where
    I: wayland_client::Proxy + 'static,
    S: Dispatch<I, ()> + 'static,
{
    globals.bind(qh, 1..=1, ()).ok()
}

impl Sheet {
    /// Ask the compositor for the sheet again, and put every margin where it now belongs. Called at the
    /// start and whenever the displays change.
    fn reread(&mut self, qh: &QueueHandle<Self>) {
        self.screens = sheet::screens().into_iter().map(|s| (s.name.clone(), s)).collect();
        // a display the compositor no longer draws a sheet on loses its margins
        let screens = &self.screens;
        self.bands.retain(|b| screens.contains_key(&b.screen));
        let names: Vec<String> = self.screens.keys().cloned().collect();
        for name in names {
            for side in Side::ALL {
                if !self.bands.iter().any(|b| b.screen == name && b.side == side) {
                    self.add(qh, &name, side);
                }
            }
        }
        self.lay_out();
    }

    /// One margin's surface on one display.
    fn add(&mut self, qh: &QueueHandle<Self>, screen: &str, side: Side) {
        let Some(output) = self.known.get(screen) else { return };
        let surface = self.compositor.create_surface(qh);
        let viewport = self.viewporter.get_viewport(&surface, qh, ());
        // the scale this display shows it at, to the 120th: without it the surface draws at whole scales
        let scale = self.scaling.as_ref().map(|m| m.get_fractional_scale(&surface, qh, surface.clone()));
        let layer = self.shell.create_layer_surface(qh, surface, Layer::Bottom, Some(side.namespace()), Some(output));
        layer.set_anchor(side.anchor());
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity::None);
        // no commit yet: a surface anchored across without a size is a protocol error, and lay_out gives
        // it one straight after
        self.bands.push(Band { side, screen: screen.to_string(), layer, viewport, _scale: scale, size: (0, 0), scale120: 120, drawn: false });
    }

    /// Every margin asked for the size its sheet gives it.
    fn lay_out(&mut self) {
        for band in &mut self.bands {
            let Some(s) = self.screens.get(&band.screen) else { continue };
            // the band covers the margin; the frame's line is the compositor's to draw. Whole logical
            // pixels, rounded up so nothing of the paper shows through between the band and the frame.
            let thick = s.margin.ceil().max(1.0) as u32;
            if band.side == Side::Graph {
                let (plotting, Some(s)) = (self.plotting, self.screens.get(&band.screen)) else {
                    band.layer.set_size(1, 1);
                    band.layer.commit();
                    continue;
                };
                let at = (plotting != graph::Domain::None).then(|| graph::place(s)).flatten();
                match at {
                    Some(at) => {
                        band.layer.set_size((at.width * s.mm).ceil().max(1.0) as u32, (at.height * s.mm).ceil().max(1.0) as u32);
                        band.layer.set_margin(at.y.round() as i32, 0, 0, at.x.round() as i32);
                    }
                    None => band.layer.set_size(1, 1),
                }
                band.layer.commit();
                continue;
            }
            if band.side == Side::Schedule {
                let (Some(what), Some(s)) = (self.schedule.as_ref(), self.screens.get(&band.screen)) else {
                    band.layer.set_size(1, 1);
                    band.layer.commit();
                    continue;
                };
                let Some(at) = what.place(s) else {
                    band.layer.set_size(1, 1);
                    band.layer.commit();
                    continue;
                };
                band.layer.set_size((at.width * s.mm).ceil().max(1.0) as u32, (at.height * s.mm).ceil().max(1.0) as u32);
                band.layer.set_margin(at.y.round() as i32, 0, 0, at.x.round() as i32);
                band.layer.commit();
                continue;
            }
            if band.side == Side::Title {
                // the block sits in the frame's corner: the margin away from the display's edges, and as
                // large as the compositor measured it
                let Some(block) = s.title.as_ref() else { continue };
                let (w, h) = (block.rect.2.ceil().max(1.0) as u32, block.rect.3.ceil().max(1.0) as u32);
                band.layer.set_size(w, h);
                band.layer.set_margin(0, thick as i32, thick as i32, 0);
                band.layer.commit();
                continue;
            }
            let (w, h) = if band.side.vertical() { (thick, 0) } else { (0, thick) };
            band.layer.set_size(w, h);
            // the sides run between the top and the bottom
            if band.side.vertical() {
                band.layer.set_margin(thick as i32, 0, thick as i32, 0);
            }
            band.layer.commit();
        }
    }

    /// Draw one margin: nothing yet but the paper left clear and, 50 mm from the frame's corner, a rule
    /// one physical pixel wide — what stage 0 exists to measure.
    fn draw(&mut self, surface: &wl_surface::WlSurface) {
        let Some(i) = self.bands.iter().position(|b| b.layer.wl_surface() == surface) else { return };
        let (side, screen, size, scale120) = {
            let b = &self.bands[i];
            (b.side, b.screen.clone(), b.size, b.scale120)
        };
        let Some(s) = self.screens.get(&screen).cloned() else { return };
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        // the buffer is in physical pixels, and the viewport maps it onto the logical size: nothing is
        // resampled, so a rule a pixel wide stays a pixel wide at 1.25 as at 1
        let scale = scale120 as f64 / 120.0;
        let (pw, ph) = (((size.0 as f64) * scale).round() as i32, ((size.1 as f64) * scale).round() as i32);
        if pw <= 0 || ph <= 0 {
            return;
        }
        let Some(mut pool) = self.pool.take() else { return };
        let Ok((buffer, canvas)) = pool.create_buffer(pw, ph, pw * 4, wl_shm::Format::Argb8888) else {
            self.pool = Some(pool);
            return;
        };
        let mut canvas = paint::Canvas::new(canvas, pw, ph);
        let hover = self.over.as_ref().filter(|(o, _, _)| o == surface).map(|(_, x, y)| (*x, *y));
        if side == Side::Graph {
            if let (Some(face), Some(at)) = (self.face.as_mut(), graph::place(&s)) {
                if let Some(r) = self.readings_over_time.as_ref() {
                    graph::draw(&mut canvas, face, self.plotting, r, &at, &s, &self.theme, scale);
                }
            }
        } else if side == Side::Schedule {
            if let (Some(what), Some(face)) = (self.schedule.as_ref(), self.face.as_mut()) {
                if let Some(at) = what.place(&s) {
                    schedule::draw(&mut canvas, face, what, &at, &s, &self.theme, scale, None);
                }
            }
        } else if side == Side::Title {
            self.letter_title(&mut canvas, &s, scale, hover);
        } else if self.face.is_some() {
            let spaces: Vec<(u64, bool)> = self.spaces.iter().filter(|(_, _, _, on)| on == &s.name).map(|(n, f, _, _)| (*n, *f)).collect();
            let told = self.told();
            let face = self.face.as_mut().expect("a face, just looked for");
            margins::letter(&mut canvas, face, &s, &self.theme, side, scale, s.workspace, &spaces, &self.parked, hover, &told);
            // DRAFTPAPER_SHEET_RULE: a rule a physical pixel wide, 50 mm from the frame's corner, which
            // tests/sheet.sh measures at every scale. It proves that the buffer is in physical pixels and
            // nothing is resampled — the one thing that would quietly spoil every word on the sheet — and
            // it is drawn only when asked for, never on a sheet anyone is looking at.
            if std::env::var_os("DRAFTPAPER_SHEET_RULE").is_some() {
                let along = (s.at(50.0, !side.vertical()) * scale).round() as i32 - if side.vertical() { (s.margin.ceil() * scale).round() as i32 } else { 0 };
                let ink = self.theme.ink.premultiplied();
                if side.vertical() {
                    canvas.fill(0, along, pw, 1, ink);
                } else {
                    canvas.fill(along, 0, 1, ph, ink);
                }
            }
        }
        self.pool = Some(pool);
        let band = &mut self.bands[i];
        band.viewport.set_destination(size.0 as i32, size.1 as i32);
        surface.set_buffer_scale(1);
        surface.damage_buffer(0, 0, pw, ph);
        let _ = buffer.attach_to(surface);
        band.layer.commit();
        band.drawn = true;
    }
}

impl Sheet {
    /// The title block's cells, lettered as the shell lettered them (qml/TitleBlock.qml): the label 2 mm
    /// in and 1.2 mm down from the cell's corner, the reading 2 mm in and standing 1.6 mm above its foot.
    /// A reading too wide for its cell is lettered smaller before it is cut, as the shell does.
    fn letter_title(&mut self, canvas: &mut paint::Canvas, s: &sheet::Screen, scale: f64, hover: Option<(f64, f64)>) {
        let (Some(block), Some(face)) = (s.title.as_ref(), self.face.as_mut()) else { return };
        let cells = title::cells(&self.theme, s.workspace, std::time::SystemTime::now(), &self.readings, self.busy);
        // a millimetre of this display, in the physical pixels the buffer is in
        let mm = (s.mm * scale) as f32;
        let (ink, faint, alert) = (self.theme.ink, self.theme.faint(), self.theme.alert);
        // the cell under the pointer, where a click on it does something: a faint tint, as the shell's
        let lit = hover.and_then(|(x, y)| title::under(block, x, y)).filter(|(_, live)| *live).map(|(i, _)| i);
        for (i, (cell, at)) in cells.iter().zip(&block.cells).enumerate() {
            if Some(i) == lit {
                let tint = paint::Ink { a: 0.07, ..ink };
                let (rx, ry) = ((((at.0 - block.rect.0) * scale) as i32), (((at.1 - block.rect.1) * scale) as i32));
                canvas.fill(rx, ry, (at.2 * scale) as i32, (at.3 * scale) as i32, tint.premultiplied());
            }
            // the cell's corner, from the block's own
            let (x, y) = (((at.0 - block.rect.0) * scale) as f32, ((at.1 - block.rect.1) * scale) as f32);
            let (w, h) = ((at.2 * scale) as f32, (at.3 * scale) as f32);
            // the label
            let label_px = 2.2 * mm;
            face.draw(canvas, x + 2.0 * mm, y + 1.2 * mm + face.ascent(label_px), cell.label, label_px, 0.3 * mm, faint.premultiplied());
            // the reading: the tall cell (the clock) letters it large, the rest at reading size
            if cell.value.is_empty() {
                continue;
            }
            let mut px = if at.3 / s.mm > 12.0 { 9.0 * mm } else { 3.6 * mm };
            let room = w - 4.0 * mm;
            let least = 2.4 * mm;
            while px > least && face.width(&cell.value, px, 0.0) > room {
                px -= 0.2 * mm;
            }
            let colour = if cell.alert { alert } else { ink };
            face.draw(canvas, x + 2.0 * mm, y + h - 1.6 * mm, &cell.value, px.max(least), 0.0, colour.premultiplied());
        }
    }

    /// What the margins have to say: the readings past their alarm, the levels just moved, Bluetooth.
    /// A level is lettered for four seconds after it moves, as the shell lettered it.
    fn told(&self) -> margins::Told {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let r = &self.readings;
        let temps = r.get("temps").and_then(serde_json::Value::as_array).cloned().unwrap_or_default();
        let named = |which: &str| {
            temps
                .iter()
                .find(|t| t.get("label").and_then(serde_json::Value::as_str) == Some(which))
                .and_then(|t| t.get("value").and_then(serde_json::Value::as_str))
                .map(|v| format!("{which} {v}"))
        };
        margins::Told {
            hot: r.get("hot").and_then(serde_json::Value::as_bool).unwrap_or(false),
            temperature: named("CPU").unwrap_or_default(),
            battery_low: r.get("battery_low").and_then(serde_json::Value::as_bool).unwrap_or(false),
            battery: r.get("battery").and_then(serde_json::Value::as_f64),
            volume: self.levels.volume,
            muted: self.levels.muted,
            volume_lit: self.levels.volume_at + 4 >= now && self.levels.volume_at > 0,
            brightness: self.levels.brightness,
            brightness_lit: self.brightness_at + 4 >= now && self.brightness_at > 0,
            notices: self.notices.due().iter().map(|n| n.text()).collect(),
            bluetooth: r
                .get("bt")
                .and_then(serde_json::Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        }
    }

    /// The top margin again: what it says changes without the sheet itself changing.
    fn redraw_top(&mut self) {
        let surfaces: Vec<wl_surface::WlSurface> = self
            .bands
            .iter_mut()
            .filter(|b| b.side == Side::Top)
            .map(|b| {
                b.drawn = false;
                b.layer.wl_surface().clone()
            })
            .collect();
        for surface in surfaces {
            self.draw(&surface);
        }
    }

    /// The volume and the backlight, read again: both are files, so this costs two small reads. When
    /// either has moved, the margins say so for a few seconds.
    fn reread_levels(&mut self) {
        let backlight = self.readings.get("backlight").and_then(serde_json::Value::as_str).map(str::to_string);
        let now = sheet::Levels::read(backlight.as_deref());
        let moved = now.volume_at != self.levels.volume_at || now.muted != self.levels.muted;
        if now.brightness != self.levels.brightness && self.levels.brightness.is_some() {
            self.brightness_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        }
        let lit = self.told();
        let was = (lit.volume_lit, lit.brightness_lit);
        self.levels = now;
        let lit = self.told();
        // redraw only while it has something to say, or has just stopped saying it
        if moved || was != (lit.volume_lit, lit.brightness_lit) {
            self.redraw_top();
        }
    }

    /// The schedule the sheet carries, worked out again where the compositor says it has changed. It is
    /// read fresh each time it is called up: a schedule is a note of how things stand, not a live meter.
    fn reread_schedule(&mut self) {
        let wanted = self.screens.values().map(|s| s.schedule.as_str()).find(|w| !w.is_empty()).unwrap_or("");
        let which = schedule::Which::of(wanted);
        if which == self.showing && self.schedule.is_some() == (which != schedule::Which::None) {
            return;
        }
        self.showing = which;
        let sheet_number = self.screens.values().map(|s| s.workspace).next().unwrap_or(1);
        let displays: Vec<(String, String, String)> = self
            .screens
            .values()
            .map(|s| {
                (
                    s.name.clone(),
                    format!("{} × {} · scale {}", (s.width * s.scale).round(), (s.height * s.scale).round(), s.scale),
                    format!("{:.1} px/mm · {:.1} logical px a mm", s.mm * s.scale, s.mm),
                )
            })
            .collect();
        let sheet_of = self
            .screens
            .values()
            .next()
            .map(|s| format!("{} · margin {} mm", self.theme.label, (s.margin / s.mm).round()))
            .unwrap_or_default();
        self.schedule = match which {
            schedule::Which::None => None,
            schedule::Which::Equipment => Some(schedule::equipment(&draftpaper::util::Paths::new(), &displays, &sheet_of)),
            schedule::Which::Processes => Some(schedule::processes()),
            schedule::Which::Revisions => Some(schedule::revisions(sheet_number)),
        };
    }

    /// The reading the sheet plots, and the store it is plotted from. The store is read again every
    /// time, since it is a file the keeper rewrites every five seconds.
    fn reread_graph(&mut self) {
        let wanted = self.screens.values().map(|s| s.graph.as_str()).find(|w| !w.is_empty()).unwrap_or("");
        self.plotting = graph::Domain::of(wanted);
        self.readings_over_time = (self.plotting != graph::Domain::None).then(graph::Readings::read).flatten();
    }

    /// The workspaces and the windows, read again: what the margins letter changes with them.
    fn reread_margins(&mut self, qh: &QueueHandle<Self>) {
        self.spaces = sheet::workspaces();
        let shown: Vec<u64> = self.screens.values().map(|s| s.workspace).collect();
        self.parked = shown.into_iter().flat_map(sheet::parked).collect();
        self.reread(qh);
        self.reread_schedule();
        self.reread_graph();
        self.lay_out();
        // the compositor hatches a parked cell, and breaks that hatching around what is lettered there
        if let Some(face) = self.face.as_mut() {
            for s in self.screens.values() {
                let (left, right) = margins::clear_spans(s, face, &self.parked);
                sheet::set_clear(&s.name, &left, &right);
            }
        }
        let surfaces: Vec<wl_surface::WlSurface> =
            self.bands.iter_mut().filter(|b| b.side != Side::Title).map(|b| { b.drawn = false; b.layer.wl_surface().clone() }).collect();
        for surface in surfaces {
            self.draw(&surface);
        }
    }

    /// The machine, read again: what the title block letters changes with it.
    fn reread_machine(&mut self) {
        self.readings = draftpaper::readings::json(&draftpaper::util::Paths::new());
        self.busy = self.cpu.busy();
        if self.plotting != graph::Domain::None {
            self.readings_over_time = graph::Readings::read();
            let surfaces: Vec<wl_surface::WlSurface> =
                self.bands.iter_mut().filter(|b| b.side == Side::Graph).map(|b| { b.drawn = false; b.layer.wl_surface().clone() }).collect();
            for surface in surfaces {
                self.draw(&surface);
            }
        }
        let surfaces: Vec<wl_surface::WlSurface> =
            self.bands.iter_mut().filter(|b| b.side == Side::Title).map(|b| { b.drawn = false; b.layer.wl_surface().clone() }).collect();
        for surface in surfaces {
            self.draw(&surface);
        }
    }
}

impl CompositorHandler for Sheet {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, factor: i32) {
        // whole scales only: a display with wp_fractional_scale tells us more exactly, and wins
        if let Some(b) = self.bands.iter_mut().find(|b| b.layer.wl_surface() == surface) {
            if self.scaling.is_none() {
                b.scale120 = (factor.max(1) as u32) * 120;
                b.drawn = false;
            }
        }
    }

    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, _: u32) {
        let redraw = self.bands.iter().any(|b| b.layer.wl_surface() == surface && !b.drawn);
        if redraw {
            self.draw(&surface.clone());
        }
    }

    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}

    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for Sheet {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }

    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if let Some(name) = self.outputs.info(&output).and_then(|i| i.name) {
            self.known.insert(name, output);
        }
        self.reread(qh);
    }

    fn update_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if let Some(name) = self.outputs.info(&output).and_then(|i| i.name) {
            self.known.insert(name, output);
        }
        // a display's scale or mode changed: the sheet is worked out again, and every margin with it
        for b in &mut self.bands {
            b.drawn = false;
        }
        self.reread(qh);
    }

    fn output_destroyed(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if let Some(name) = self.outputs.info(&output).and_then(|i| i.name) {
            self.known.remove(&name);
            self.bands.retain(|b| b.screen != name);
        }
        self.reread(qh);
    }
}

impl LayerShellHandler for Sheet {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.bands.retain(|b| &b.layer != layer);
        if self.bands.is_empty() && self.screens.is_empty() {
            self.exit = true;
        }
    }

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, configure: LayerSurfaceConfigure, _: u32) {
        let Some(b) = self.bands.iter_mut().find(|b| &b.layer == layer) else { return };
        let size = configure.new_size;
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        b.size = size;
        b.drawn = false;
        let surface = layer.wl_surface().clone();
        self.draw(&surface);
    }
}

impl ShmHandler for Sheet {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for Sheet {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

// the scale a display shows a surface at, to the 120th (wp_fractional_scale_v1)
impl Dispatch<fractional::WpFractionalScaleV1, wl_surface::WlSurface> for Sheet {
    fn event(
        state: &mut Self,
        _: &fractional::WpFractionalScaleV1,
        event: fractional::Event,
        surface: &wl_surface::WlSurface,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let fractional::Event::PreferredScale { scale } = event {
            if let Some(b) = state.bands.iter_mut().find(|b| b.layer.wl_surface() == surface) {
                if b.scale120 != scale {
                    b.scale120 = scale;
                    b.drawn = false;
                    let s = surface.clone();
                    state.draw(&s);
                }
            }
        }
    }
}

impl Dispatch<scale_manager::WpFractionalScaleManagerV1, ()> for Sheet {
    fn event(_: &mut Self, _: &scale_manager::WpFractionalScaleManagerV1, _: scale_manager::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wp_viewporter::WpViewporter, ()> for Sheet {
    fn event(_: &mut Self, _: &wp_viewporter::WpViewporter, _: wp_viewporter::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<wp_viewport::WpViewport, ()> for Sheet {
    fn event(_: &mut Self, _: &wp_viewport::WpViewport, _: wp_viewport::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

delegate_registry!(Sheet);
smithay_client_toolkit::delegate_dispatch2!(Sheet);

/// What a click on the sheet does.
enum Act {
    /// show a workspace
    Workspace(u64),
    /// bring a parked window back
    Unpark(u64),
    /// put the focused window in a cell of the margin
    Park(usize, u64),
    /// the next power profile
    Profile,
    /// a schedule, or a reading's year plotted
    Schedule(&'static str),
    Graph(&'static str),
}

impl Sheet {
    /// The band the pointer is over, and where in it.
    fn pointed(&self) -> Option<(usize, f64, f64)> {
        let (surface, x, y) = self.over.as_ref()?;
        let i = self.bands.iter().position(|b| b.layer.wl_surface() == surface)?;
        Some((i, *x, *y))
    }

    /// What a click where the pointer is would do, if anything.
    fn under_pointer(&self) -> Option<Act> {
        let (i, x, y) = self.pointed()?;
        let band = &self.bands[i];
        let s = self.screens.get(&band.screen)?;
        if band.side == Side::Title {
            let block = s.title.as_ref()?;
            let (cell, _) = title::under(block, x, y)?;
            // the cells that open something: the project its equipment, the processes theirs, a reading
            // its year plotted, the profile the next profile
            return match cell {
                3 => Some(Act::Schedule("equipment")),
                4 => Some(Act::Graph("power")),
                5 => Some(Act::Graph("battery")),
                6 => Some(Act::Profile),
                7 => Some(Act::Graph("load")),
                8 => Some(Act::Graph("network")),
                9 => Some(Act::Graph("temperature")),
                10 => Some(Act::Schedule("processes")),
                _ => None,
            };
        }
        let spaces: Vec<(u64, bool)> = self.spaces.iter().filter(|(_, _, _, on)| on == &s.name).map(|(n, f, _, _)| (*n, *f)).collect();
        match margins::under(s, band.side, x, y, &spaces, &self.parked)? {
            margins::Spot::Workspace(n) => Some(Act::Workspace(n)),
            margins::Spot::Cell { parked: Some(id), .. } => Some(Act::Unpark(id)),
            margins::Spot::Cell { cell, parked: None } => Some(Act::Park(cell, s.workspace)),
        }
    }

    /// Do it.
    fn act(&mut self) {
        match self.under_pointer() {
            Some(Act::Workspace(n)) => sheet::action(&format!("workspace {n}")),
            Some(Act::Unpark(id)) => sheet::action(&format!("unpark {id}")),
            Some(Act::Park(cell, workspace)) => sheet::action(&format!("park_at {cell} {workspace}")),
            Some(Act::Schedule(which)) => sheet::ask(&serde_json::json!({ "request": "schedule", "which": which }).to_string()),
            Some(Act::Graph(which)) => sheet::ask(&serde_json::json!({ "request": "graph", "which": which }).to_string()),
            Some(Act::Profile) => {
                let now = self.readings.get("profile").and_then(serde_json::Value::as_str).unwrap_or("");
                let next = title::next_profile(now);
                if let Some(conn) = draftpaper::dbus::system() {
                    draftpaper::dbus::set_power_profile(&conn, next);
                }
                self.reread_machine();
            }
            None => {}
        }
    }

    /// Draw the mark under the pointer, and take it off whatever had it.
    fn redraw_hover(&mut self) {
        let now = self.pointed().map(|(i, _, _)| self.bands[i].layer.wl_surface().clone());
        let was = self.marked.take();
        self.marked = now.clone();
        for surface in [was, now].into_iter().flatten() {
            if let Some(b) = self.bands.iter_mut().find(|b| b.layer.wl_surface() == &surface) {
                b.drawn = false;
            }
            self.draw(&surface);
        }
    }
}

impl SeatHandler for Sheet {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self.seats.get_pointer_with_theme::<Self, ()>(qh, &seat, self.shm.wl_shm(), self.compositor.create_surface(qh), ThemeSpec::default()).ok();
        }
    }

    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Pointer {
            self.pointer = None;
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for Sheet {
    fn pointer_frame(&mut self, conn: &Connection, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for event in events {
            let (x, y) = event.position;
            match event.kind {
                PointerEventKind::Leave { .. } => {
                    self.over = None;
                    self.redraw_hover();
                }
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    let before = self.over.as_ref().map(|(s, _, _)| s.clone());
                    self.over = Some((event.surface.clone(), x, y));
                    // the pointer says what it is over: a hand where a click does something
                    let hand = self.under_pointer().is_some();
                    if let Some(pointer) = self.pointer.as_ref() {
                        let icon = if hand { CursorIcon::Pointer } else { CursorIcon::Default };
                        let _ = pointer.set_cursor(conn, icon);
                    }
                    if before.as_ref() != Some(&event.surface) || hand {
                        self.redraw_hover();
                    }
                }
                PointerEventKind::Press { button, .. } if button == BTN_LEFT => {
                    self.over = Some((event.surface.clone(), x, y));
                    self.act();
                }
                _ => {}
            }
        }
    }
}
