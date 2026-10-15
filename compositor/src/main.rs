// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
#![allow(irrefutable_let_patterns)]

mod handlers;

mod config;
mod cursor;
mod focus;
mod headless;
mod input;
mod layout;
mod render;
mod screens;
mod sheet;
mod state;
mod theme;
mod ipc;
mod parking;
mod animation;
mod screencopy;
mod output_management;
mod wallpaper;
mod lens;
mod rules;
mod title;
mod tty;
mod winit;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
pub use state::Draftpaper;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // draftpaper-compositor msg ...: a client of a running compositor
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("msg") {
        std::process::exit(ipc::msg(&args[1..]));
    }

    // draftpaper-compositor [--tty | --headless WxH [--scale S]] [--session] [--command PROGRAM] | msg REQUEST...
    // the TTY backend (the displays and input devices themselves) only when asked for: a compositor
    // started without it from inside a session, with no display to nest in, must not take the hardware
    let tty = args.iter().any(|a| a == "--tty");
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let headless = value("--headless").map(|v| {
        let (w, h) = v.split_once('x').and_then(|(w, h)| Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?))).unwrap_or_else(|| {
            eprintln!("draftpaper: --headless WIDTHxHEIGHT, as 3840x2160");
            std::process::exit(2);
        });
        (w, h, value("--scale").and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0))
    });
    if !tty && headless.is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
        eprintln!("draftpaper: no display to nest in: --tty to run on the displays themselves (a login session), --headless WxH for tests");
        std::process::exit(2);
    }

    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    // the event loop's data is the compositor's state (Smithay's idle notifier needs that)
    let mut event_loop: EventLoop<Draftpaper> = EventLoop::try_new()?;

    let display: Display<Draftpaper> = Display::new()?;
    let mut data = Draftpaper::new(&mut event_loop, display);

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
    } else if let Some((w, h, scale)) = headless {
        if let Err(e) = crate::headless::init_headless(&mut event_loop, &mut data, (w, h), scale) {
            eprintln!("draftpaper: headless: {e}");
            std::process::exit(1);
        }
    } else {
        crate::winit::init_winit(&mut event_loop, &mut data)?;
    }

    // the IPC socket (DRAFTPAPER_SOCKET), before anything is started that might talk to it
    data.start_ipc();

    if session {
        let vars = "WAYLAND_DISPLAY DRAFTPAPER_SOCKET XDG_CURRENT_DESKTOP XDG_SESSION_TYPE XDG_SESSION_DESKTOP";
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!(
                "systemctl --user import-environment {vars}; dbus-update-activation-environment --systemd {vars}; \
                 systemctl --user start draftpaper-session.target"
            ))
            .status();
    }

    // X11 applications (not for tests)
    if headless.is_none() {
        data.start_xwayland();
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

    // SIGTERM, SIGINT, SIGHUP: leave the loop, so Xwayland's lock and sockets and the session target are
    // cleaned up; SIGUSR1: a screenshot (as the Print key). Children get a clean signal mask (std's Command).
    use smithay::reexports::calloop::signals::{Signal, Signals};
    event_loop.handle().insert_source(
        Signals::new(&[Signal::SIGTERM, Signal::SIGINT, Signal::SIGHUP, Signal::SIGUSR1])?,
        |event, _, data| match event.signal() {
            Signal::SIGUSR1 => crate::tty::screenshot(data),
            _ => data.loop_signal.stop(),
        },
    )?;

    event_loop.run(None, &mut data, move |data| data.ipc_refresh())?;
    data.stop_ipc();

    if session {
        let _ = std::process::Command::new("systemctl").args(["--user", "stop", "draftpaper-session.target"]).status();
    }

    Ok(())
}
