// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
use std::time::Duration;

use smithay::{
    backend::{
        renderer::damage::OutputDamageTracker,
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::EventLoop,
    utils::{Rectangle, Transform},
};

use crate::{
    render::{attach_sheet, output_elements},
    theme::Theme,
    Draftpaper,
};

/// Pixels per millimetre for the nested window: DRAFTPAPER_MM, else about 160 PPI (the i3 edition's fallback)
fn nested_mm() -> f32 {
    std::env::var("DRAFTPAPER_MM").ok().and_then(|v| v.parse().ok()).unwrap_or(6.2)
}

pub fn init_winit(
    event_loop: &mut EventLoop<'static, Draftpaper>,
    data: &mut Draftpaper,
) -> Result<(), Box<dyn std::error::Error>> {
    let display_handle = data.display_handle.clone();
    let state = data;

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
    let _global = output.create_global::<Draftpaper>(&display_handle);
    output.change_current_state(Some(mode), Some(Transform::Flipped180), None, Some((0, 0).into()));
    output.set_preferred(mode);

    state.space.map_output(&output, (0, 0));

    let mut damage_tracker = OutputDamageTracker::from_output(&output);
    let theme = Theme::load();
    eprintln!("draftpaper: sheet in theme {}, margin {} mm, {} px/mm", theme.name, theme.margin_mm, nested_mm());
    let margin_mm = theme.margin_mm as f64;
    attach_sheet(&output, theme, nested_mm());
    state.add_screen(output.clone(), nested_mm() as f64, margin_mm);

    std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);

    event_loop.handle().insert_source(winit, move |event, _, data| {
        let state = data;

        match event {
            WinitEvent::Resized { size, .. } => {
                output.change_current_state(
                    Some(Mode {
                        size,
                        refresh: 60_000,
                    }),
                    None,
                    None,
                    None,
                );
                state.update_screen(&output);
            }
            WinitEvent::Input(event) => state.process_input_event(event),
            WinitEvent::Redraw => {
                let size = backend.window_size();
                let damage = Rectangle::from_size(size);

                {
                    let (renderer, mut framebuffer) = backend.bind().unwrap();
                    // the host desktop draws the pointer
                    let elements = output_elements(state, renderer, &output, None);
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
                let _ = state.display_handle.flush_clients();

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
