//! What an output shows, for every backend, in front-to-back order: the pointer (on the TTY, where
//! nobody else draws it), the low layers' menus, the overlay and top layer surfaces, the windows (each
//! through a rescale, a shift and a crop, so a parking window can fly and a workspace slide; at rest
//! they are the identity), their 1 px borders (in the gutters between tiles, so they never cover
//! content), the bottom and background layer surfaces (the shell's margins and title block), and the
//! sheet.

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
    /// the wallpaper's textures, cut to their pixels
    Picture=CropRenderElement<MemoryRenderBufferRenderElement<GlesRenderer>>,
}

/// The sheet of one output, kept with the output.
pub struct OutputSheet(pub RefCell<SheetRenderer>);

/// The four 1 px edges of a window's border, kept with the window.
struct Border(RefCell<[SolidColorBuffer; 4]>);

/// The title block's rules (title.rs), kept with the output.
struct TitleRules(RefCell<Vec<SolidColorBuffer>>);

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
    // an output not in the space (gone, or not placed yet) has nothing to show
    let Some(geometry) = state.space.output_geometry(output) else { return Vec::new() };
    let mut elements = Vec::new();

    // locked: the lock surface (if the locker has made one yet) on black, nothing else
    if state.locked {
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
        return elements;
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

    // the windows: each drawn at `at` (its geometry, global logical), then mapped onto `to`, faded by
    // `alpha`. A window's own surfaces are clipped to its rectangle (a client that draws more than its
    // tile, as Firefox can while it catches up with a new size, does not spill over its neighbours, even
    // when its buffer goes straight to a display plane); its popups (menus) are not, and are in front.
    // Everything stays inside `crop`.
    let draw = |elements: &mut Vec<OutputElements>, renderer: &mut GlesRenderer, window: &Window, at: Rectangle<i32, Logical>, to: Rectangle<f64, Logical>, alpha: f32, crop: Rectangle<i32, Physical>| {
        use smithay::{backend::renderer::element::surface::render_elements_from_surface_tree, desktop::{PopupManager, WindowSurface}};
        let g = to_phys(at.to_f64());
        let t = to_phys(to);
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
        let rescale = Scale { x: t.size.w as f64 / g.size.w.max(1) as f64, y: t.size.h as f64 / g.size.h.max(1) as f64 };
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
            draw(&mut elements, renderer, &f.window, f.from, crate::animation::lerp(f.from, to, e), (1.0 - 0.85 * e) as f32, whole);
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
    let border = |elements: &mut Vec<OutputElements>, window: &Window| {
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
        let rects = [
            ((loc.x - 1, loc.y - 1), (size.w + 2, 1)),
            ((loc.x - 1, loc.y + size.h), (size.w + 2, 1)),
            ((loc.x - 1, loc.y), (1, size.h)),
            ((loc.x + size.w, loc.y), (1, size.h)),
        ];
        for (edge, (at, wh)) in edges.iter_mut().zip(rects) {
            edge.update(wh, colour);
            let at: Point<i32, Logical> = at.into();
            elements.push(OutputElements::Border(SolidColorRenderElement::from_buffer(
                edge,
                at.to_physical_precise_round(output_scale),
                output_scale,
                1.0,
                Kind::Unspecified,
            )));
        }
    };

    // the windows, front first, each followed by its border
    for window in state.space.elements_for_output(output).rev() {
        let Some(loc) = state.space.element_location(window) else { continue };
        let at = Rectangle::new(loc, window.geometry().size);
        match (state.anim.flight(window), shift) {
            // unparking: from its cell into its place, appearing
            (Some(f), _) if f.to.is_none() => {
                let e = f.progress(now);
                draw(&mut elements, renderer, window, at, crate::animation::lerp(f.from, at, e), (0.15 + 0.85 * e) as f32, whole);
            }
            (_, Some((sl, e, w))) => {
                let to = Rectangle::new(at.loc.to_f64() + Point::from((sl.dir as f64 * (1.0 - e) * w, 0.0)), at.size.to_f64());
                draw(&mut elements, renderer, window, at, to, 1.0, frame);
            }
            _ => draw(&mut elements, renderer, window, at, at.to_f64(), 1.0, whole),
        }
        border(&mut elements, window);
    }
    if let Some((sl, e, w)) = shift {
        for (window, loc) in &sl.outgoing {
            let at = Rectangle::new(*loc, window.geometry().size);
            let to = Rectangle::new(at.loc.to_f64() - Point::from((sl.dir as f64 * e * w, 0.0)), at.size.to_f64());
            draw(&mut elements, renderer, window, at, to, 1.0, frame);
        }
    }

    elements.extend(layer_elements(renderer, &[Layer::Bottom, Layer::Background]));

    if let Some(sheet) = output.user_data().get::<OutputSheet>() {
        let size: Size<i32, Logical> = geometry.size;
        let scale = output.current_scale().fractional_scale();
        let screen = state.screens.iter().find(|s| &s.output == output);
        let hidden = screen.map(|s| s.hidden_ticks).unwrap_or_default();
        let mut sheet = sheet.0.borrow_mut();
        sheet.set_hidden(hidden);
        // the wallpaper, prepared for this output (wallpaper.rs): the picture in the frame, the ambient
        // margins, the frosted title block, under the sheet's lines
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
            let block = crate::title::title_block(physical, margin, mm);
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
            draw(&w.frosted, (w.title.size.w, w.title.size.h), w.title);
            draw(&w.picture, (w.interior.size.w, w.interior.size.h), w.interior);
            if let Some((ambient, px)) = &w.ambient {
                draw(ambient, *px, Rectangle::from_size(physical.into()));
            }
        }
    }
    elements
}
