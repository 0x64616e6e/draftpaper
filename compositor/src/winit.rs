// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).

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
                state.refresh_output_management();
            }
            WinitEvent::Input(event) => state.process_input_event(event),
            WinitEvent::Redraw => {
                state.anim.prune(std::time::Instant::now());
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
                if !state.screencopies.is_empty() {
                    crate::screencopy::finish(state, backend.renderer(), None);
                }
                if std::mem::take(&mut state.screenshot_requested) {
                    let path = crate::tty::screenshot_path(&output);
                    match crate::tty::capture(state, backend.renderer(), &output, &path) {
                        Ok(()) => eprintln!("draftpaper: screenshot {}", path.display()),
                        Err(e) => eprintln!("draftpaper: screenshot: {e}"),
                    }
                }

                state.send_frames(&output, state.start_time.elapsed());

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

/// The nested window as the one head of output management: its size is the window's.
pub fn heads(data: &Draftpaper) -> Vec<crate::output_management::HeadInfo> {
    data.space
        .outputs()
        .map(|o| {
            let mode = o.current_mode().map(|m| crate::output_management::ModeInfo { w: m.size.w, h: m.size.h, refresh: m.refresh, preferred: true });
            let loc = o.current_location();
            crate::output_management::HeadInfo {
                name: o.name(),
                make: "Smithay".into(),
                model: "Winit".into(),
                serial: String::new(),
                physical: (0, 0),
                modes: mode.into_iter().collect(),
                enabled: true,
                current: mode,
                position: (loc.x, loc.y),
                // drawn flipped into the window; to clients it is upright
                transform: Transform::Normal,
                scale: o.current_scale().fractional_scale(),
            }
        })
        .collect()
}

/// Nested, only the scale and the position change; the size is the window's.
pub fn configure(data: &mut Draftpaper, changes: &[crate::output_management::HeadChange], test: bool) -> Result<(), String> {
    let outputs: Vec<Output> = data.space.outputs().cloned().collect();
    for change in changes {
        let output = outputs.iter().find(|o| o.name() == change.name).ok_or_else(|| format!("no output {}", change.name))?;
        let config = change.config.as_ref().ok_or("the nested window cannot be turned off")?;
        if let Some(mode) = &config.mode {
            let current = output.current_mode().ok_or("no mode")?;
            if (mode.w, mode.h) != (current.size.w, current.size.h) {
                return Err("the nested window's size is the window's own".into());
            }
        }
        if config.transform.is_some_and(|t| t != Transform::Normal) {
            return Err("the nested window is not rotated".into());
        }
        if config.scale.is_some_and(|s| !(0.25..=4.0).contains(&s)) {
            return Err("scale out of range (0.25 to 4)".into());
        }
        if config.adaptive_sync == Some(true) {
            return Err("adaptive sync is not supported".into());
        }
    }
    if test {
        return Ok(());
    }
    for change in changes {
        let Some(config) = &change.config else { continue };
        let output = outputs.iter().find(|o| o.name() == change.name).unwrap();
        let position = config.position.map(smithay::utils::Point::from);
        output.change_current_state(None, None, config.scale.map(smithay::output::Scale::Fractional), position);
        data.space.map_output(output, output.current_location());
        data.update_screen(output);
    }
    Ok(())
}
