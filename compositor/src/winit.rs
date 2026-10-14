// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
use std::time::Duration;

use smithay::{
    backend::{
        renderer::{
            damage::OutputDamageTracker,
            element::{render_elements, surface::WaylandSurfaceRenderElement},
            gles::{element::PixelShaderElement, GlesRenderer},
        },
        winit::{self, WinitEvent},
    },
    desktop::space::{space_render_elements, SpaceRenderElements},
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::EventLoop,
    utils::{Rectangle, Transform},
};

use crate::{layout::Grid, sheet::SheetRenderer, theme::Theme, CalloopData, Draftpaper};

render_elements! {
    /// What an output shows: the windows (and layer surfaces), and the sheet under them.
    pub OutputElements<=GlesRenderer>;
    Space=SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Sheet=PixelShaderElement,
}

/// Pixels per millimetre for the nested window: DRAFTPAPER_MM, else about 160 PPI (the i3 edition's fallback)
fn nested_mm() -> f32 {
    std::env::var("DRAFTPAPER_MM").ok().and_then(|v| v.parse().ok()).unwrap_or(6.2)
}

pub fn init_winit(
    event_loop: &mut EventLoop<CalloopData>,
    data: &mut CalloopData,
) -> Result<(), Box<dyn std::error::Error>> {
    let display_handle = &mut data.display_handle;
    let state = &mut data.state;

    let (mut backend, winit) = winit::init()?;

    let mode = Mode {
        size: backend.window_size(),
        refresh: 60_000,
    };

    let output = Output::new(
        "winit".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
        },
    );
    let _global = output.create_global::<Draftpaper>(display_handle);
    output.change_current_state(Some(mode), Some(Transform::Flipped180), None, Some((0, 0).into()));
    output.set_preferred(mode);

    state.space.map_output(&output, (0, 0));

    let mut damage_tracker = OutputDamageTracker::from_output(&output);
    let theme = Theme::load();
    eprintln!("draftpaper: sheet in theme {}, margin {} mm, {} px/mm", theme.name, theme.margin_mm, nested_mm());
    let margin_mm = theme.margin_mm as f64;
    let mut sheet = SheetRenderer::new(theme, nested_mm());
    state.grid = Some(Grid::new(mode.size.to_logical(1), nested_mm() as f64, margin_mm));

    std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);

    event_loop.handle().insert_source(winit, move |event, _, data| {
        let display = &mut data.display_handle;
        let state = &mut data.state;

        match event {
            WinitEvent::Resized { size, .. } => {
                state.grid = Some(Grid::new(size.to_logical(1), nested_mm() as f64, margin_mm));
                state.relayout();
                output.change_current_state(
                    Some(Mode {
                        size,
                        refresh: 60_000,
                    }),
                    None,
                    None,
                    None,
                );
            }
            WinitEvent::Input(event) => state.process_input_event(event),
            WinitEvent::Redraw => {
                let size = backend.window_size();
                let damage = Rectangle::from_size(size);

                {
                    let (renderer, mut framebuffer) = backend.bind().unwrap();
                    // front to back: the windows, then the sheet at the bottom
                    let mut elements: Vec<OutputElements> = space_render_elements(renderer, [&state.space], &output, 1.0)
                        .unwrap()
                        .into_iter()
                        .map(OutputElements::Space)
                        .collect();
                    let logical = size.to_logical(1);
                    elements.push(OutputElements::Sheet(sheet.element(renderer, logical)));
                    damage_tracker
                        .render_output(renderer, &mut framebuffer, 0, &elements, [0.1, 0.1, 0.1, 1.0])
                        .unwrap();
                }
                backend.submit(Some(&[damage])).unwrap();

                state.space.elements().for_each(|window| {
                    window.send_frame(
                        &output,
                        state.start_time.elapsed(),
                        Some(Duration::ZERO),
                        |_, _| Some(output.clone()),
                    )
                });

                state.space.refresh();
                state.popups.cleanup();
                let _ = display.flush_clients();

                // Ask for redraw to schedule new frame.
                backend.window().request_redraw();
            }
            WinitEvent::CloseRequested => {
                state.loop_signal.stop();
            }
            _ => (),
        };
    })?;

    Ok(())
}
