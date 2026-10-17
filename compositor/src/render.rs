//! What an output shows, for every backend, in front-to-back order: the pointer (on the TTY, where
//! nobody else draws it), the low layers' menus, the overlay and top layer surfaces, the windows (each
//! through a rescale, a shift and a crop, so a parking window can fly and a workspace slide; at rest
//! a tile is only moved onto its own physical pixels, layout.rs), their borders (a logical pixel's worth
//! of whole physical pixels, in the gutters between tiles, so they never cover content), the bottom
//! and background layer surfaces (the shell's margins and title block), and the sheet.

use std::collections::HashMap;
use std::{cell::RefCell, time::Instant};

use smithay::{
    backend::renderer::{
        element::{
            memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
            render_elements,
            solid::{SolidColorBuffer, SolidColorRenderElement},
            surface::WaylandSurfaceRenderElement,
            utils::{CropRenderElement, Relocate, RelocateRenderElement, RescaleRenderElement},
            AsRenderElements, Kind,
        },
        gles::{element::PixelShaderElement, GlesRenderer},
    },
    desktop::{layer_map_for_output, Window},
    output::Output,
    utils::{Logical, Physical, Point, Rectangle, Scale, Size},
    wayland::shell::wlr_layer::Layer,
};

use crate::{sheet::SheetRenderer, state::Draftpaper, theme::Theme};

render_elements! {
    pub OutputElements<=GlesRenderer>;
    /// the pointer
    Buffer=MemoryRenderBufferRenderElement<GlesRenderer>,
    Window=CropRenderElement<RelocateRenderElement<RescaleRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>>>,
    Border=SolidColorRenderElement,
    /// the lock surface, and the low layers' menus drawn above the windows
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Sheet=PixelShaderElement,
    /// the magnifier (lens.rs)
    Lens=crate::lens::LensElement,
    /// the blurred scene (blur.rs), the opening picture (opening.rs)
    Texture=crate::blur::TextureElement,
    /// the wallpaper's textures, cut to their pixels
    Picture=CropRenderElement<MemoryRenderBufferRenderElement<GlesRenderer>>,
}

/// Below this blur (millimetres) the blurred picture fades out over the sharp scene (output_elements).
const BLUR_FADE_MM: f64 = 2.0;

/// The sheet of one output, kept with the output.
pub struct OutputSheet(pub RefCell<SheetRenderer>);

/// The four 1 px edges of a window's border, kept with the window.
struct Border(RefCell<[SolidColorBuffer; 4]>);

/// The title block's rules (title.rs), kept with the output.
struct TitleRules(RefCell<Vec<SolidColorBuffer>>);

/// The parked cells' hatching (title.rs), per cell: what it was made for (the cell, the step, the ink, the
/// strength, the spans left clear) and its picture, kept with the output.
type HatchKey = (Rectangle<i32, smithay::utils::Physical>, i32, [u32; 3], u32, Vec<(i32, i32)>);
struct Hatches(RefCell<HashMap<usize, (HatchKey, MemoryRenderBuffer)>>);

/// The revision block's rules (title.rs), kept with the output.
struct RevisionRules(RefCell<Vec<SolidColorBuffer>>);

/// The scale bar's rules and fills (title.rs), kept with the output.
struct ScaleRules(RefCell<Vec<SolidColorBuffer>>);

/// The output's size in physical pixels: its mode, as it is turned. The sheet is measured from these
/// edges, not from the logical size scaled, which a scale such as 1.75 rounds past them.
pub fn mode_size(output: &Output) -> (i32, i32) {
    match output.current_mode() {
        Some(mode) => {
            let s = output.current_transform().transform_size(mode.size);
            (s.w, s.h)
        }
        None => (0, 0),
    }
}

/// Give `output` its sheet, at `mm` physical pixels per millimetre.
pub fn attach_sheet(output: &Output, theme: Theme, mm: f32) {
    output.user_data().insert_if_missing(|| OutputSheet(RefCell::new(SheetRenderer::new(theme, mm))));
}

/// The elements of one frame of `output`. `pointer` is the cursor image and its hotspot, when this
/// backend draws the cursor itself.
pub fn output_elements(
    state: &Draftpaper,
    renderer: &mut GlesRenderer,
    output: &Output,
    pointer: Option<(&MemoryRenderBuffer, Point<i32, Logical>)>,
) -> Vec<OutputElements> {
    // locked: the lock surface (the lock screen's card) over the sheet blurred, as on the login screen:
    // the sheet alone (no window, no surface of the shell), so nothing of the work shows through
    if state.locked {
        let (mut elements, _) = scene_elements(state, renderer, output, None, false);
        // the lock surface, without the black behind it
        elements.pop();
        let (sheet, _) = scene_elements(state, renderer, output, None, true);
        let mm = state.screens.iter().find(|s| &s.output == output).map(|s| s.phys_mm).unwrap_or(6.2);
        // inside the frame blurred, the margins sharp (the sheet behind)
        if let Some(blurred) = crate::blur::blur(renderer, output, mode_size(output), &sheet, crate::opening::BLUR_MM * mm, frame_inside(state, output)) {
            elements.push(OutputElements::Texture(blurred));
        }
        elements.extend(sheet);
        return elements;
    }
    // the opening (opening.rs): the picture the displays showed, alone until the desktop is up, then
    // fading over the desktop while its blur goes
    // and, with a session being restored, its windows back (session.rs, for a while)
    let up = state.shell_ready && state.session_settled();
    let opening = crate::opening::frame(output, up);
    let mut blur_mm = state.blur_mm();
    let picture = match opening {
        Some((picture, _, true)) => return vec![OutputElements::Texture(picture)],
        Some((picture, mm, false)) => {
            blur_mm = blur_mm.max(mm);
            Some(picture)
        }
        None => None,
    };
    // blurred (blur.rs): what is behind the pointer and the overlay and top layers
    if blur_mm > 0.05 && !state.locked {
        let (mut elements, front) = scene_elements(state, renderer, output, pointer, false);
        let mm = state.screens.iter().find(|s| &s.output == output).map(|s| s.phys_mm).unwrap_or(6.2);
        let back = elements.split_off(front.min(elements.len()));
        if let Some(mut blurred) = crate::blur::blur(renderer, output, mode_size(output), &back, blur_mm * mm, frame_inside(state, output)) {
            // the blur is drawn at a quarter of the resolution: light, it would still be softer than the
            // scene, and the step to the sharp scene at the end shows. Under BLUR_FADE_MM it is laid over the
            // sharp scene, ever fainter, so it goes to nothing without a step
            let k = (blur_mm / BLUR_FADE_MM).min(1.0) as f32;
            blurred.alpha = k;
            elements.push(OutputElements::Texture(blurred));
            // the scene behind: sharp in the margins (the blur covers only the inside of the frame), and
            // under the fading blur at the end
            elements.extend(back);
        } else {
            elements.extend(back);
        }
        if let Some(p) = picture {
            elements.insert(0, OutputElements::Texture(p));
        }
        return elements;
    }
    let mut elements = output_scene(state, renderer, output, pointer);
    if let Some(p) = picture {
        elements.insert(0, OutputElements::Texture(p));
    }
    elements
}

/// What `output` shows, with the magnifier but without the opening and the blur.
fn output_scene(
    state: &Draftpaper,
    renderer: &mut GlesRenderer,
    output: &Output,
    pointer: Option<(&MemoryRenderBuffer, Point<i32, Logical>)>,
) -> Vec<OutputElements> {
    // the magnifier (lens.rs), on the output under the pointer: the scene without the pointer, the lens
    // and its frame in front
    let at = state.seat.get_pointer().unwrap().current_location();
    let here = state.space.output_geometry(output).filter(|g| g.to_f64().contains(at));
    let (Some(lens), Some(geometry), false) = (&state.lens, here, state.locked) else {
        return scene_elements(state, renderer, output, pointer, false).0;
    };
    let scene = scene_elements(state, renderer, output, None, false).0;
    let scale = output.current_scale().fractional_scale();
    let mm = state.screens.iter().find(|s| &s.output == output).map(|s| s.phys_mm).unwrap_or(6.2);
    let hotspot = (at - geometry.loc.to_f64()).to_physical(scale);
    let Some((element, frame)) = crate::lens::lens(renderer, output, mode_size(output), &scene, lens, hotspot, mm) else { return scene };
    let buffers = output.user_data().get_or_insert(|| LensFrame(RefCell::new(Default::default())));
    let mut buffers = buffers.0.borrow_mut();
    let mut elements = Vec::with_capacity(scene.len() + 9);
    for (i, (buffer, r)) in buffers.iter_mut().zip(frame).enumerate() {
        let colour = if i < 4 { [0.0, 0.0, 0.0, 0.85] } else { [0.85, 0.85, 0.85, 0.85] };
        buffer.update((r.size.w, r.size.h), colour);
        elements.push(OutputElements::Border(SolidColorRenderElement::from_buffer(buffer, r.loc, 1.0, 1.0, Kind::Unspecified)));
    }
    elements.push(OutputElements::Lens(element));
    elements.extend(scene);
    elements
}

/// The magnifier's frame (lens.rs), kept with the output.
struct LensFrame(RefCell<[SolidColorBuffer; 8]>);

/// What `output` shows, without the magnifier.
fn scene_elements(
    state: &Draftpaper,
    renderer: &mut GlesRenderer,
    output: &Output,
    pointer: Option<(&MemoryRenderBuffer, Point<i32, Logical>)>,
    bare: bool,
) -> (Vec<OutputElements>, usize) {
    // an output not in the space (gone, or not placed yet) has nothing to show
    let Some(geometry) = state.space.output_geometry(output) else { return (Vec::new(), 0) };
    let mut elements = Vec::new();

    // locked: the lock surface (if the locker has made one yet) on black, nothing else (output_elements
    // draws the blurred sheet in place of the black)
    if state.locked && !bare {
        let scale = output.current_scale().fractional_scale();
        if let Some(lock) = state.lock_surfaces.get(&output.name()) {
            let lock_elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                smithay::backend::renderer::element::surface::render_elements_from_surface_tree(
                    renderer, lock.wl_surface(), (0, 0), scale, 1.0, Kind::Unspecified,
                );
            elements.extend(lock_elements.into_iter().map(OutputElements::Surface));
        }
        elements.push(OutputElements::Border(SolidColorRenderElement::new(
            smithay::backend::renderer::element::Id::new(),
            smithay::utils::Rectangle::from_size(geometry.size.to_physical_precise_round(scale)),
            0,
            [0.0, 0.0, 0.0, 1.0],
            Kind::Unspecified,
        )));
        let n = elements.len();
        return (elements, n);
    }

    if let Some((buffer, hotspot)) = pointer {
        let at = state.seat.get_pointer().unwrap().current_location();
        if geometry.to_f64().contains(at) {
            let loc = (at - geometry.loc.to_f64()).to_physical(output.current_scale().fractional_scale()) - hotspot.to_f64().to_physical(1.0);
            if let Ok(element) =
                MemoryRenderBufferRenderElement::from_buffer(renderer, loc, buffer, None, None, None, Kind::Cursor)
            {
                elements.push(OutputElements::Buffer(element));
            }
        }
    }

    // menus of the bottom and background layers (the shell's tray menu), in front of the windows
    let scale = output.current_scale().fractional_scale();
    for (popup, loc) in state.low_layer_popups(output).into_iter().rev() {
        let at = (loc - geometry.loc).to_physical_precise_round(scale);
        let popup_elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
            smithay::backend::renderer::element::surface::render_elements_from_surface_tree(
                renderer, popup.wl_surface(), at, scale, 1.0, Kind::Unspecified,
            );
        elements.extend(popup_elements.into_iter().map(OutputElements::Surface));
    }

    let now = Instant::now();
    let to_phys = |r: Rectangle<f64, Logical>| -> Rectangle<i32, Physical> {
        Rectangle::new(
            (r.loc - geometry.loc.to_f64()).to_physical(scale).to_i32_round(),
            r.size.to_physical(scale).to_i32_round(),
        )
    };
    let whole = Rectangle::from_size(geometry.size.to_physical_precise_round(scale));

    // layer surfaces of `layers`, front first
    let layer_elements = |renderer: &mut GlesRenderer, layers: &[Layer]| -> Vec<OutputElements> {
        let map = layer_map_for_output(output);
        let mut out = Vec::new();
        for layer in layers {
            for surface in map.layers_on(*layer).rev() {
                let loc = map.layer_geometry(surface).map(|g| g.loc).unwrap_or_default().to_physical_precise_round(scale);
                let e: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = surface.render_elements(renderer, loc, Scale::from(scale), 1.0);
                out.extend(e.into_iter().map(OutputElements::Surface));
            }
        }
        out
    };
    elements.extend(layer_elements(renderer, &[Layer::Overlay, Layer::Top]));
    // what is in front of the windows (the pointer, menus, the overlay and top layers: the login card)
    // stays sharp when the rest is blurred
    let mut front = elements.len();

    // the windows: each drawn at `at` (its geometry, global logical), then mapped onto `to`, faded by
    // `alpha`. A window's own surfaces are clipped to its rectangle (a client that draws more than its
    // tile, as Firefox can while it catches up with a new size, does not spill over its neighbours, even
    // when its buffer goes straight to a display plane); its popups (menus) are not, and are in front.
    // Everything stays inside `crop`. A tile (`exact`, layout.rs) is moved onto its own physical pixels and
    // cut to them: whole logical pixels cannot always land on them at a scale such as 1.5 or 1.75.
    let draw = |elements: &mut Vec<OutputElements>, renderer: &mut GlesRenderer, window: &Window, at: Rectangle<i32, Logical>, to: Rectangle<f64, Logical>, alpha: f32, crop: Rectangle<i32, Physical>, exact: Option<Rectangle<i32, Physical>>| {
        use smithay::{backend::renderer::element::surface::render_elements_from_surface_tree, desktop::{PopupManager, WindowSurface}};
        let g = to_phys(at.to_f64());
        let t = exact.unwrap_or_else(|| to_phys(to));
        let loc = (at.loc - window.geometry().loc - geometry.loc).to_physical_precise_round(scale);
        let (own, popups): (Vec<WaylandSurfaceRenderElement<GlesRenderer>>, Vec<WaylandSurfaceRenderElement<GlesRenderer>>) =
            match window.underlying_surface() {
                WindowSurface::Wayland(toplevel) => {
                    let surface = toplevel.wl_surface();
                    let own = render_elements_from_surface_tree(renderer, surface, loc, scale, alpha, Kind::Unspecified);
                    let mut popups = Vec::new();
                    for (popup, offset) in PopupManager::popups_for_surface(surface) {
                        let at = loc + (window.geometry().loc + offset - popup.geometry().loc).to_physical_precise_round(scale);
                        popups.extend(render_elements_from_surface_tree(renderer, popup.wl_surface(), at, scale, alpha, Kind::Unspecified));
                    }
                    (own, popups)
                }
                WindowSurface::X11(_) => (window.render_elements(renderer, loc, Scale::from(scale), alpha), Vec::new()),
            };
        let rescale = match exact {
            Some(_) => Scale { x: 1.0, y: 1.0 },
            None => Scale { x: t.size.w as f64 / g.size.w.max(1) as f64, y: t.size.h as f64 / g.size.h.max(1) as f64 },
        };
        let clip = t.intersection(crop).unwrap_or_default();
        for (e, crop) in popups.into_iter().map(|e| (e, crop)).chain(own.into_iter().map(|e| (e, clip))) {
            let e = RescaleRenderElement::from_element(e, g.loc, rescale);
            let e = RelocateRenderElement::from_element(e, t.loc - g.loc, Relocate::Relative);
            if let Some(e) = CropRenderElement::from_element(e, scale, crop) {
                elements.push(OutputElements::Window(e));
            }
        }
    };
    let name = output.name();
    // parking windows (out of the space), in front: from their place into their cell, fading
    for f in state.anim.flights.iter().filter(|f| f.output == name) {
        if let Some(to) = f.to {
            let e = f.progress(now);
            draw(&mut elements, renderer, &f.window, f.from, crate::animation::lerp(f.from, to, e), (1.0 - 0.85 * e) as f32, whole, None);
        }
    }
    // a workspace switch: the new windows come in from `dir`, the old go out, inside the frame
    let slide = state.anim.slide(&name);
    let (shift, frame) = match (slide, state.screens.iter().find(|s| &s.output == output)) {
        (Some(sl), Some(sc)) => {
            let e = sl.progress(now);
            let area = sc.grid.area;
            let frame = to_phys(Rectangle::new(area.loc - Point::from((1, 1)), area.size + Size::from((2, 2))).to_f64());
            (Some((sl, e, geometry.size.w as f64)), frame)
        }
        _ => (None, whole),
    };
    let animated = |w: &Window| state.anim.flight(w).is_some() || shift.is_some();

    // a window's border, in the theme's window colours, just behind the window itself: in front of the
    // windows under it, so a floating window's border shows over the tiles (a tile's runs in the gutter);
    // a fullscreen window, and one moving, have none
    let colours = output.user_data().get::<OutputSheet>().map(|sheet| {
        let theme = &sheet.0.borrow().theme;
        (theme.border_focused, theme.border)
    });
    let focused = state.focused_window();
    let fullscreen: Vec<_> = state.screens.iter().filter_map(|s| s.workspace().fullscreen.clone()).collect();
    let output_scale = output.current_scale().fractional_scale();
    let border = |elements: &mut Vec<OutputElements>, window: &Window, exact: Option<Rectangle<i32, Physical>>| {
        let Some((focused_colour, colour)) = colours else { return };
        if fullscreen.contains(window) || animated(window) {
            return;
        }
        let Some(loc) = state.space.element_location(window) else { return };
        let (loc, size) = (loc - geometry.loc, window.geometry().size);
        let c = if Some(window) == focused.as_ref() { focused_colour } else { colour };
        let colour = [c[0], c[1], c[2], 1.0];
        let border = window.user_data().get_or_insert(|| Border(RefCell::new(Default::default())));
        let mut edges = border.0.borrow_mut();
        // in physical pixels, around the pixels the window covers (its tile's, or its own rounded): a
        // logical pixel's worth wide, whole pixels
        let p = exact.unwrap_or_else(|| to_phys(Rectangle::new(loc + geometry.loc, size).to_f64()));
        let b = (output_scale.round() as i32).max(1);
        let (x, y, w, h) = (p.loc.x, p.loc.y, p.size.w, p.size.h);
        let rects = [((x - b, y - b), (w + 2 * b, b)), ((x - b, y + h), (w + 2 * b, b)), ((x - b, y), (b, h)), ((x + w, y), (b, h))];
        for (edge, (at, wh)) in edges.iter_mut().zip(rects) {
            edge.update(wh, colour);
            elements.push(OutputElements::Border(SolidColorRenderElement::from_buffer(
                edge,
                Point::<i32, Physical>::from(at),
                1.0,
                1.0,
                Kind::Unspecified,
            )));
        }
    };

    // the windows, front first, each followed by its border
    for window in state.space.elements_for_output(output).rev() {
        let Some(loc) = state.space.element_location(window) else { continue };
        let at = Rectangle::new(loc, window.geometry().size);
        let tile = crate::layout::tile_of(window, at);
        match (state.anim.flight(window), shift) {
            // unparking: from its cell into its place, appearing
            (Some(f), _) if f.to.is_none() => {
                let e = f.progress(now);
                draw(&mut elements, renderer, window, at, crate::animation::lerp(f.from, at, e), (0.15 + 0.85 * e) as f32, whole, None);
            }
            (_, Some((sl, e, w))) => {
                let dx = sl.dir as f64 * (1.0 - e) * w;
                let to = Rectangle::new(at.loc.to_f64() + Point::from((dx, 0.0)), at.size.to_f64());
                let exact = tile.map(|t| Rectangle::new(t.pixels.loc + Point::from(((dx * scale).round() as i32, 0)), t.pixels.size));
                draw(&mut elements, renderer, window, at, to, 1.0, frame, exact);
            }
            _ => draw(&mut elements, renderer, window, at, at.to_f64(), 1.0, whole, tile.map(|t| t.pixels)),
        }
        border(&mut elements, window, tile.map(|t| t.pixels));
    }
    if let Some((sl, e, w)) = shift {
        for (window, loc) in &sl.outgoing {
            let at = Rectangle::new(*loc, window.geometry().size);
            let dx = -(sl.dir as f64) * e * w;
            let to = Rectangle::new(at.loc.to_f64() + Point::from((dx, 0.0)), at.size.to_f64());
            let exact = crate::layout::tile_of(window, at).map(|t| Rectangle::new(t.pixels.loc + Point::from(((dx * scale).round() as i32, 0)), t.pixels.size));
            draw(&mut elements, renderer, window, at, to, 1.0, frame, exact);
        }
    }

    elements.extend(layer_elements(renderer, &[Layer::Bottom, Layer::Background]));

    // leaving (handoff.rs): the bare sheet, as the next compositor will first draw it; the login screen
    // leaves what it shows (the sheet blurred, the name in the title block), taken over as a picture
    // bare: the sheet alone (behind the lock surface, blurred)
    if (state.leaving.is_some() && !state.greeter) || bare {
        elements.clear();
        front = 0;
    }

    if let Some(sheet) = output.user_data().get::<OutputSheet>() {
        let size: Size<i32, Logical> = geometry.size;
        let scale = output.current_scale().fractional_scale();
        let screen = state.screens.iter().find(|s| &s.output == output);
        let hidden = screen.map(|s| s.hidden_ticks).unwrap_or_default();
        let mut sheet = sheet.0.borrow_mut();
        sheet.set_hidden(hidden);
        // the wallpaper, prepared for this output (wallpaper.rs): the picture in the frame, the ambient
        // margins, the frosted title block (when lettered), under the sheet's lines
        output.user_data().insert_if_missing(crate::wallpaper::WallpaperCache::default);
        let cache = output.user_data().get::<crate::wallpaper::WallpaperCache>().unwrap();
        let physical = mode_size(output);
        // the margin and the zones, for the shader, the wallpaper and the title block alike
        // physical pixels per millimetre, as the screen has them (and the IPC sends them)
        let mm = screen.map(|s| s.phys_mm as f32).unwrap_or(sheet.mm);
        let (margin, zones) = match screen {
            Some(s) => (crate::title::margin(s.margin_mm, s.mm, scale), s.grid.zones),
            None => (crate::title::margin(sheet.theme.margin_mm as f64, sheet.mm as f64 / scale, scale), (2, 2)),
        };
        let wall = screen.is_some() && crate::wallpaper::prepare(cache, &sheet.theme, physical, margin, zones, mm);
        let prepared = cache.0.borrow();
        let wallpaper = prepared.as_ref().filter(|_| wall);
        let mode = match wallpaper {
            None => 0,
            Some(w) if w.ambient.is_some() => 2,
            Some(_) => 1,
        };
        let ink = wallpaper.and_then(|w| w.margin_ink).unwrap_or(sheet.theme.ink);
        sheet.set_wall(mode, ink);
        // the title block's rules (title.rs), when the shell letters it: over the sheet and the frost,
        // whole pixels, none over another or over the frame's line
        let lettered = layer_map_for_output(output).layers().any(|l| l.namespace() == "draftpaper-title-block");
        if lettered {
            let block = crate::title::title_block(physical, margin, mm, zones);
            let t = &sheet.theme;
            let colour = |alpha: f32| {
                let a = alpha.min(1.0);
                [t.ink[0] * a, t.ink[1] * a, t.ink[2] * a, a]
            };
            // as the margins' lettering: the outer rule nearly the frame's weight, the cells' lighter
            let (heavy, light) = (colour(0.85 * t.sheet.pattern_strength + 0.1), colour(0.6 * t.sheet.pattern_strength + 0.1));
            // buffers of physical pixels (scale 1), kept with the output so that damage follows their changes
            let buffers = output.user_data().get_or_insert(|| TitleRules(RefCell::new(Vec::new())));
            let mut buffers = buffers.0.borrow_mut();
            buffers.resize_with(block.rules.len(), SolidColorBuffer::default);
            for (buffer, rule) in buffers.iter_mut().zip(&block.rules) {
                buffer.update((rule.rect.size.w, rule.rect.size.h), if rule.heavy { heavy } else { light });
                elements.push(OutputElements::Border(SolidColorRenderElement::from_buffer(buffer, rule.rect.loc, 1.0, 1.0, Kind::Unspecified)));
            }
        }
        // the revision block's rules (title.rs), while the shell letters it (in a repository, switched on):
        // as the title block's, the outer rule heavy, the cells' light
        let revised = layer_map_for_output(output).layers().any(|l| l.namespace() == "draftpaper-revisions");
        let block = revised.then(|| crate::title::revision_block(physical, margin, mm, zones)).flatten();
        let buffers = output.user_data().get_or_insert(|| RevisionRules(RefCell::new(Vec::new())));
        let mut buffers = buffers.0.borrow_mut();
        let rules = block.map(|b| b.rules).unwrap_or_default();
        buffers.resize_with(rules.len(), SolidColorBuffer::default);
        {
            let t = &sheet.theme;
            let colour = |alpha: f32| {
                let a = alpha.min(1.0);
                [t.ink[0] * a, t.ink[1] * a, t.ink[2] * a, a]
            };
            let (heavy, light) = (colour(0.85 * t.sheet.pattern_strength + 0.1), colour(0.6 * t.sheet.pattern_strength + 0.1));
            for (buffer, rule) in buffers.iter_mut().zip(&rules) {
                buffer.update((rule.rect.size.w, rule.rect.size.h), if rule.heavy { heavy } else { light });
                elements.push(OutputElements::Border(SolidColorRenderElement::from_buffer(buffer, rule.rect.loc, 1.0, 1.0, Kind::Unspecified)));
            }
        }
        drop(buffers);
        // the scale bar (title.rs), when the shell letters the margins (it letters the numbers): in the
        // frame's ink at its weight, true to size on screens that report theirs
        let margins = layer_map_for_output(output).layers().any(|l| l.namespace() == "draftpaper-margin");
        let bar = (margins && state.config.scale_bar)
            .then(|| screen.and_then(|s| crate::title::scale_bar(physical, margin, s.phys_mm, zones, s.measured)))
            .flatten()
            .and_then(|s| s.bar);
        let buffers = output.user_data().get_or_insert(|| ScaleRules(RefCell::new(Vec::new())));
        let mut buffers = buffers.0.borrow_mut();
        let ink = bar.map(|b| b.ink).unwrap_or_default();
        buffers.resize_with(ink.len(), SolidColorBuffer::default);
        let t = &sheet.theme;
        let a = (0.85 * t.sheet.pattern_strength + 0.1).min(1.0);
        for (buffer, r) in buffers.iter_mut().zip(&ink) {
            buffer.update((r.size.w, r.size.h), [t.ink[0] * a, t.ink[1] * a, t.ink[2] * a, a]);
            elements.push(OutputElements::Border(SolidColorRenderElement::from_buffer(buffer, r.loc, 1.0, 1.0, Kind::Unspecified)));
        }
        drop(buffers);
        // the parked windows' cells hatched (issue #46), as a drawing hatches a cut: one-pixel lines at 45°
        // (title.rs's hatched), in the margins' ink, leaving clear what the shell letters there; each cell's
        // picture made once and kept until its cell, lettering or colour change
        if let Some(s) = screen {
            let st = &sheet.theme.sheet;
            let step = crate::title::hatch_step(st.hatch_spacing as f64, s.phys_mm);
            let a = st.hatch_strength.clamp(0.0, 1.0);
            let ink = wallpaper.and_then(|w| w.margin_ink).unwrap_or(sheet.theme.ink);
            // none on the bare sheet (behind the lock screen, the last frame left to the next compositor)
            let cells: Vec<usize> = if step > 0 && a > 0.0 && !bare && state.leaving.is_none() { s.workspace().parked.iter().map(|p| p.cell).collect() } else { Vec::new() };
            let cache = output.user_data().get_or_insert(|| Hatches(RefCell::new(HashMap::new())));
            let mut cache = cache.0.borrow_mut();
            cache.retain(|cell, _| cells.contains(cell));
            for cell in cells {
                let Some(r) = crate::title::side_cell(physical, margin, zones, cell) else { continue };
                let side = usize::from(r.loc.x > 0);
                let clear: Vec<(i32, i32)> = s.clear[side].iter().copied().filter(|&(y0, y1)| y1 > r.loc.y && y0 < r.loc.y + r.size.h).collect();
                let key = (r, step, ink.map(f32::to_bits), a.to_bits(), clear.clone());
                if cache.get(&cell).is_none_or(|(k, _)| *k != key) {
                    let mut img = image::RgbaImage::new(r.size.w.max(1) as u32, r.size.h.max(1) as u32);
                    let px = [ink[0] * a, ink[1] * a, ink[2] * a, a].map(|v| (v * 255.0).round() as u8);
                    for (x, y, p) in img.enumerate_pixels_mut() {
                        let (ox, oy) = (r.loc.x + x as i32, r.loc.y + y as i32);
                        if crate::title::hatched(ox, oy, step) && !clear.iter().any(|&(y0, y1)| oy >= y0 && oy < y1) {
                            *p = image::Rgba(px);
                        }
                    }
                    cache.insert(cell, (key, crate::wallpaper::buffer(&img)));
                }
                let (_, buffer) = &cache[&cell];
                let src = Rectangle::<f64, Logical>::from_size((r.size.w as f64, r.size.h as f64).into());
                let logical = Size::<i32, Logical>::from(((r.size.w as f64 / scale).ceil() as i32, (r.size.h as f64 / scale).ceil() as i32));
                if let Ok(e) = MemoryRenderBufferRenderElement::from_buffer(renderer, r.loc.to_f64(), buffer, None, Some(src), Some(logical), Kind::Unspecified) {
                    if let Some(e) = CropRenderElement::from_element(e, scale, r) {
                        elements.push(OutputElements::Picture(e));
                    }
                }
            }
        }
        elements.push(OutputElements::Sheet(sheet.element(renderer, size, scale, physical, margin, zones)));
        if let Some(w) = wallpaper {
            // whole logical pixels, rounded up: the element is cropped to the rectangle's pixels after
            let logical = |r: Rectangle<i32, smithay::utils::Physical>| {
                Size::<i32, Logical>::from(((r.size.w as f64 / scale).ceil() as i32, (r.size.h as f64 / scale).ceil() as i32))
            };
            // drawn whole (`src`, the buffer's own pixels: its scale is 1) at the rectangle's size; without a
            // src Smithay takes the logical size for one, and on a scaled output showed the buffer's top-left
            // corner, magnified. An element's size is whole logical pixels, which at a scale such as 1.75 is not
            // whole physical ones: drawn a little larger, then cut to the rectangle, it covers it exactly
            let mut draw = |buffer: &MemoryRenderBuffer, px: (i32, i32), r: Rectangle<i32, smithay::utils::Physical>| {
                let src = Rectangle::<f64, Logical>::from_size((px.0 as f64, px.1 as f64).into());
                if let Ok(e) = MemoryRenderBufferRenderElement::from_buffer(renderer, r.loc.to_f64(), buffer, None, Some(src), Some(logical(r)), Kind::Unspecified) {
                    if let Some(e) = CropRenderElement::from_element(e, scale, r) {
                        elements.push(OutputElements::Picture(e));
                    }
                }
            };
            // the frost under the title block only when the shell letters it, as its rules (the login and
            // lock screens, the session before its shell: no block, no frost)
            if lettered {
                draw(&w.frosted, (w.title.size.w, w.title.size.h), w.title);
            }
            if let (true, Some((place, frosted))) = (revised, &w.revisions) {
                draw(frosted, (place.size.w, place.size.h), *place);
            }
            draw(&w.picture, (w.interior.size.w, w.interior.size.h), w.interior);
            if let Some((ambient, px)) = &w.ambient {
                draw(ambient, *px, Rectangle::from_size(physical.into()));
            }
        }
    }
    (elements, front)
}

/// The inside of `output`'s frame (inside its line), in physical pixels: what the blur covers, the
/// margins and the frame left sharp. The whole output when it has no screen.
fn frame_inside(state: &Draftpaper, output: &Output) -> Rectangle<i32, Physical> {
    let (w, h) = mode_size(output);
    let Some(s) = state.screens.iter().find(|s| &s.output == output) else { return Rectangle::from_size((w, h).into()) };
    let inset = crate::title::margin(s.margin_mm, s.mm, output.current_scale().fractional_scale()) + crate::title::LINE;
    Rectangle::new((inset, inset).into(), ((w - 2 * inset).max(1), (h - 2 * inset).max(1)).into())
}
