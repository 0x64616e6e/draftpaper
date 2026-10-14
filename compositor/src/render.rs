//! What an output shows, for every backend: the pointer (on the TTY, where nobody else draws it), the
//! windows and layer surfaces, and the sheet at the bottom, in front-to-back order.

use std::cell::RefCell;

use smithay::{
    backend::renderer::{
        element::{
            memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
            render_elements,
            surface::WaylandSurfaceRenderElement,
            Kind,
        },
        gles::{element::PixelShaderElement, GlesRenderer},
    },
    desktop::space::{space_render_elements, SpaceRenderElements},
    output::Output,
    utils::{Logical, Point, Size},
};

use crate::{sheet::SheetRenderer, state::Draftpaper, theme::Theme};

render_elements! {
    pub OutputElements<=GlesRenderer>;
    Pointer=MemoryRenderBufferRenderElement<GlesRenderer>,
    Space=SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Sheet=PixelShaderElement,
}

/// The sheet of one output, kept with the output.
pub struct OutputSheet(pub RefCell<SheetRenderer>);

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
    let geometry = state.space.output_geometry(output).unwrap();
    let mut elements = Vec::new();

    if let Some((buffer, hotspot)) = pointer {
        let at = state.seat.get_pointer().unwrap().current_location();
        if geometry.to_f64().contains(at) {
            let loc = (at - geometry.loc.to_f64()).to_physical(output.current_scale().fractional_scale()) - hotspot.to_f64().to_physical(1.0);
            if let Ok(element) =
                MemoryRenderBufferRenderElement::from_buffer(renderer, loc, buffer, None, None, None, Kind::Cursor)
            {
                elements.push(OutputElements::Pointer(element));
            }
        }
    }

    elements.extend(
        space_render_elements(renderer, [&state.space], output, 1.0)
            .unwrap_or_default()
            .into_iter()
            .map(OutputElements::Space),
    );

    if let Some(sheet) = output.user_data().get::<OutputSheet>() {
        let size: Size<i32, Logical> = geometry.size;
        let scale = output.current_scale().fractional_scale();
        elements.push(OutputElements::Sheet(sheet.0.borrow_mut().element(renderer, size, scale)));
    }
    elements
}
