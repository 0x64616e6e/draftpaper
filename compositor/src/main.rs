// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
#![allow(irrefutable_let_patterns)]

mod handlers;

mod cursor;
mod input;
mod layout;
mod render;
mod screens;
mod sheet;
mod state;
mod theme;
mod tty;
mod winit;

use smithay::reexports::{
    calloop::EventLoop,
    wayland_server::{Display, DisplayHandle},
};
pub use state::Draftpaper;

pub struct CalloopData {
    state: Draftpaper,
    display_handle: DisplayHandle,
    /// the TTY backend's devices and outputs (None when nested)
    tty: Option<tty::TtyData>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;

    let display: Display<Draftpaper> = Display::new()?;
    let display_handle = display.handle();
    let state = Draftpaper::new(&mut event_loop, display);

    let mut data = CalloopData { state, display_handle, tty: None };

    // draftpaper-compositor [--tty] [--command PROGRAM]
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tty = args.iter().any(|a| a == "--tty");
    let command = args.iter().position(|a| a == "-c" || a == "--command").and_then(|i| args.get(i + 1)).cloned();

    if tty {
        if let Err(e) = crate::tty::init_tty(&mut event_loop, &mut data) {
            eprintln!("draftpaper: {e}");
            std::process::exit(1);
        }
        std::env::set_var("WAYLAND_DISPLAY", &data.state.socket_name);
    } else {
        crate::winit::init_winit(&mut event_loop, &mut data)?;
    }

    std::process::Command::new(command.as_deref().unwrap_or("alacritty")).spawn().ok();

    event_loop.run(None, &mut data, move |_| {
        // Draftpaper is running
    })?;

    Ok(())
}
