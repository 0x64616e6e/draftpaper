// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
#![allow(irrefutable_let_patterns)]

mod handlers;

mod config;
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

    // draftpaper-compositor [--tty] [--session] [--command PROGRAM]
    let args: Vec<String> = std::env::args().skip(1).collect();
    // the TTY backend when asked for, or when there is no display to nest in
    let tty = args.iter().any(|a| a == "--tty")
        || (std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none());
    // --session: started as the login session (draftpaper-session): hand the environment to systemd
    // and D-Bus and start draftpaper-session.target, stopped again on exit
    let session = args.iter().any(|a| a == "--session");
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

    if session {
        let vars = "WAYLAND_DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE XDG_SESSION_DESKTOP";
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!(
                "systemctl --user import-environment {vars}; dbus-update-activation-environment --systemd {vars}; \
                 systemctl --user start draftpaper-session.target"
            ))
            .status();
    }

    // config.toml: read now, and again whenever it changes
    data.reload_config();
    event_loop.handle().insert_source(
        smithay::reexports::calloop::timer::Timer::from_duration(std::time::Duration::from_secs(2)),
        |_, _, data| {
            data.reload_config();
            smithay::reexports::calloop::timer::TimeoutAction::ToDuration(std::time::Duration::from_secs(2))
        },
    )?;

    std::process::Command::new(command.as_deref().unwrap_or("alacritty")).spawn().ok();

    event_loop.run(None, &mut data, move |_| {
        // Draftpaper is running
    })?;

    if session {
        let _ = std::process::Command::new("systemctl").args(["--user", "stop", "draftpaper-session.target"]).status();
    }

    Ok(())
}
