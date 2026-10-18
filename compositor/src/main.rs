// Adapted from smallvil (Smithay v0.7.0, MIT; see compositor/NOTICE).
#![allow(irrefutable_let_patterns)]

mod handlers;

mod handoff;
mod config;
mod cursor;
mod focus;
mod headless;
mod input;
mod layout;
mod render;
mod rulers;
mod screens;
mod sheet;
mod state;
mod theme;
mod ipc;
mod parking;
mod animation;
mod screencopy;
mod output_management;
mod gamma;
mod icc;
mod wallpaper;
mod lens;
mod blur;
mod opening;
mod rules;
mod session;
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
    // draftpaper-compositor hold CRTC=FB...: keeps the device (stdin) open for the next compositor (handoff.rs)
    // draftpaper-compositor grab DEVICE DIR: what each display shows, as the opening reads it (opening.rs)
    if args.first().map(String::as_str) == Some("grab") {
        std::process::exit(opening::grab(&args[1..]));
    }
    if args.first().map(String::as_str) == Some("hold") {
        std::process::exit(handoff::holder(&args[1..]));
    }

    // draftpaper-compositor [--tty | --headless WxH[,WxH...] [--scale S]] [--session | --greeter] [--command PROGRAM] | msg REQUEST...
    // the TTY backend (the displays and input devices themselves) only when asked for: a compositor
    // started without it from inside a session, with no display to nest in, must not take the hardware
    let tty = args.iter().any(|a| a == "--tty");
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    // --headless WxH[,WxH...]: an output each, side by side
    let headless = value("--headless").map(|v| {
        let sizes: Option<Vec<(i32, i32)>> =
            v.split(',').map(|s| s.split_once('x').and_then(|(w, h)| Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?)))).collect();
        let sizes = sizes.filter(|s| !s.is_empty()).unwrap_or_else(|| {
            eprintln!("draftpaper: --headless WIDTHxHEIGHT[,WIDTHxHEIGHT...], as 3840x2160 or 1920x1200,2560x1440");
            std::process::exit(2);
        });
        (sizes, value("--scale").and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.0))
    });
    if !tty && headless.is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
        eprintln!("draftpaper: no display to nest in: --tty to run on the displays themselves (a login session), --headless WxH for tests");
        std::process::exit(2);
    }

    // DRAFTPAPER_LOG (the session's: not RUST_LOG, which every program started in the session would
    // inherit), else RUST_LOG
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_env("DRAFTPAPER_LOG").or_else(|_| tracing_subscriber::EnvFilter::try_from_default_env()) {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    // the event loop's data is the compositor's state (Smithay's idle notifier needs that)
    let mut event_loop: EventLoop<Draftpaper> = EventLoop::try_new()?;

    let display: Display<Draftpaper> = Display::new()?;
    let mut data = Draftpaper::new(&mut event_loop, display);
    // the socket's name, whatever the log level (the tests and scripts read it here)
    eprintln!("draftpaper: wayland socket {}", data.socket_name.to_string_lossy());

    // --session: started as the login session (draftpaper-session): hand the environment to systemd
    // and D-Bus and start draftpaper-session.target, stopped again on exit
    let session = args.iter().any(|a| a == "--session");
    let command = args.iter().position(|a| a == "-c" || a == "--command").and_then(|i| args.get(i + 1)).cloned();
    // --greeter: the login screen under greetd (draftpaper-greeter): the login card (qml/greeter.qml) on
    // the sheet, no bindings, no X11; when the card has started the session (IPC "leave"), the compositor
    // leaves its last frame on screen for it (handoff.rs)
    data.greeter = args.iter().any(|a| a == "--greeter");
    // the session saved and restored (session.rs): a login session's, or a test's
    data.saved.on = !data.greeter && (session || std::env::var("DRAFTPAPER_SESSION").is_ok_and(|v| v == "1"));

    if tty {
        if let Err(e) = crate::tty::init_tty(&mut event_loop, &mut data) {
            eprintln!("draftpaper: {e}");
            std::process::exit(1);
        }
        std::env::set_var("WAYLAND_DISPLAY", &data.socket_name);
    } else if let Some((sizes, scale)) = &headless {
        if let Err(e) = crate::headless::init_headless(&mut event_loop, &mut data, sizes, *scale) {
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

    // X11 applications (not for tests, not on the login screen)
    if headless.is_none() && !data.greeter {
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

    // the last session's applications started again (after the signals: the threads that reap them
    // must have them blocked), and the session read every so often to be saved
    data.session_restore();
    event_loop.handle().insert_source(smithay::reexports::calloop::timer::Timer::from_duration(crate::session::PERIOD), |_, _, data| {
        data.session_tick();
        smithay::reexports::calloop::timer::TimeoutAction::ToDuration(crate::session::PERIOD)
    })?;

    if data.greeter {
        // the card; when it quits (the session started, or it failed), so does the compositor. After the
        // signals: they are blocked only in threads started after that (any other would die of them)
        let root = std::env::var_os("DRAFTPAPER_ROOT").map(std::path::PathBuf::from).unwrap_or_else(|| "/usr/share/draftpaper".into());
        let mut card = std::process::Command::new("qs").arg("-p").arg(root.join("qml/greeter.qml")).arg("-n").spawn()?;
        let (done, gone) = smithay::reexports::calloop::channel::channel::<()>();
        std::thread::spawn(move || {
            let _ = card.wait();
            let _ = done.send(());
        });
        event_loop.handle().insert_source(gone, |_, _, data| data.loop_signal.stop())?;
    } else {
        // --command PROGRAM: started once up; nested without one, a terminal to try it in. A session starts
        // nothing itself: the shell and the rest come with draftpaper-session.target
        match command.as_deref() {
            Some(program) => drop(std::process::Command::new(program).spawn()),
            None if !session && !tty => drop(std::process::Command::new("alacritty").spawn()),
            None => {}
        }
    }

    event_loop.run(None, &mut data, move |data| data.ipc_refresh())?;
    // the last frame stays on screen for the next compositor
    handoff::leave(&mut event_loop, &mut data);
    data.stop_ipc();

    if session {
        let _ = std::process::Command::new("systemctl").args(["--user", "stop", "draftpaper-session.target"]).status();
    }

    Ok(())
}
