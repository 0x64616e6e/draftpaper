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

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
pub use state::Draftpaper;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    // the event loop's data is the compositor's state (Smithay's idle notifier needs that)
    let mut event_loop: EventLoop<Draftpaper> = EventLoop::try_new()?;

    let display: Display<Draftpaper> = Display::new()?;
    let mut data = Draftpaper::new(&mut event_loop, display);

    // draftpaper-compositor [--tty] [--command PROGRAM]
    let args: Vec<String> = std::env::args().skip(1).collect();
    let tty = args.iter().any(|a| a == "--tty");
    let command = args.iter().position(|a| a == "-c" || a == "--command").and_then(|i| args.get(i + 1)).cloned();

    if tty {
        if let Err(e) = crate::tty::init_tty(&mut event_loop, &mut data) {
            eprintln!("draftpaper: {e}");
            std::process::exit(1);
        }
        std::env::set_var("WAYLAND_DISPLAY", &data.socket_name);
    } else {
        crate::winit::init_winit(&mut event_loop, &mut data)?;
    }

    std::process::Command::new(command.as_deref().unwrap_or("alacritty")).spawn().ok();

    event_loop.run(None, &mut data, move |_| {
        // Draftpaper is running
    })?;

    Ok(())
}
