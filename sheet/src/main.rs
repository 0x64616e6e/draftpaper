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
mod paint;
mod sheet;
mod title;

use std::collections::HashMap;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
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
    protocol::{wl_output, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{wp_fractional_scale_manager_v1 as scale_manager, wp_fractional_scale_v1 as fractional},
    viewporter::client::{wp_viewport, wp_viewporter},
};

use sheet::Screen;

/// Which margin a surface is, and how it lies along the sheet's edge.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    Top,
    Bottom,
    Left,
    Right,
    /// the block in the frame's bottom-right corner, lettered from the compositor's own cells
    Title,
}

impl Side {
    const ALL: [Side; 5] = [Side::Top, Side::Bottom, Side::Left, Side::Right, Side::Title];

    fn anchor(self) -> Anchor {
        match self {
            // the top and the bottom run the whole width; the sides the height between them
            Side::Top => Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
            Side::Bottom => Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            Side::Left => Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM,
            Side::Right => Anchor::RIGHT | Anchor::TOP | Anchor::BOTTOM,
            Side::Title => Anchor::BOTTOM | Anchor::RIGHT,
        }
    }

    fn vertical(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }

    fn namespace(self) -> &'static str {
        match self {
            Side::Title => "draftpaper-title-block",
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
        exit: false,
    };
    if state.scaling.is_none() {
        eprintln!("draftpaper-sheet: no wp_fractional_scale: drawing at whole scales only");
    }

    // the displays Wayland already has, before the first round trip
    queue.roundtrip(&mut state).expect("the first round trip");
    state.reread(&qh);
    state.reread_machine();

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
    calloop_wayland_source::WaylandSource::new(conn, queue).insert(handle).expect("the Wayland source");
    let _ = events.run(None, &mut state, |state| {
        state.exit |= state.bands.is_empty() && state.screens.is_empty();
    });
}

/// How often the machine is read again for the title block.
const READ_EVERY: std::time::Duration = std::time::Duration::from_secs(5);

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
        if side == Side::Title {
            self.letter_title(&mut canvas, &s, scale);
        } else {
            // stage 0's rule, kept until the margins are lettered: 50 mm from the frame's corner, a
            // physical pixel wide, which tests/sheet.sh measures at every scale
            let along = (s.at(50.0, !side.vertical()) * scale).round() as i32 - if side.vertical() { (s.margin.ceil() * scale).round() as i32 } else { 0 };
            let ink = self.theme.ink.premultiplied();
            if side.vertical() {
                canvas.fill(0, along, pw, 1, ink);
            } else {
                canvas.fill(along, 0, 1, ph, ink);
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
    fn letter_title(&mut self, canvas: &mut paint::Canvas, s: &sheet::Screen, scale: f64) {
        let (Some(block), Some(face)) = (s.title.as_ref(), self.face.as_mut()) else { return };
        let cells = title::cells(&self.theme, s.workspace, std::time::SystemTime::now(), &self.readings, self.busy);
        // a millimetre of this display, in the physical pixels the buffer is in
        let mm = (s.mm * scale) as f32;
        let (ink, faint, alert) = (self.theme.ink, self.theme.faint(), self.theme.alert);
        for (cell, at) in cells.iter().zip(&block.cells) {
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

    /// The machine, read again: what the title block letters changes with it.
    fn reread_machine(&mut self) {
        self.readings = draftpaper::readings::json(&draftpaper::util::Paths::new());
        self.busy = self.cpu.busy();
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
    registry_handlers![OutputState];
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
