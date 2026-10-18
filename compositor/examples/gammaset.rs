//! The tests' stand-in for gammastep (compositor/tests/gamma.sh): takes the first display's gamma through
//! wlr-gamma-control, prints `size N` (or `failed`), sets ramps that scale red, green and blue by the
//! factors given (`gammaset 1 0.9 0.7`), and holds them until killed; the compositor puts the display's
//! own back when it goes.
//!
//!     gammaset R G B

use std::io::{Seek, Write};
use std::os::fd::AsFd;

use wayland_client::{
    protocol::{wl_output::WlOutput, wl_registry::WlRegistry},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols_wlr::gamma_control::v1::client::{
    zwlr_gamma_control_manager_v1::ZwlrGammaControlManagerV1,
    zwlr_gamma_control_v1::{self, ZwlrGammaControlV1},
};

#[derive(Default)]
struct State {
    output: Option<WlOutput>,
    manager: Option<ZwlrGammaControlManagerV1>,
    size: Option<usize>,
    failed: bool,
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(state: &mut Self, registry: &WlRegistry, event: wayland_client::protocol::wl_registry::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let wayland_client::protocol::wl_registry::Event::Global { name, interface, .. } = event {
            match interface.as_str() {
                "wl_output" if state.output.is_none() => state.output = Some(registry.bind(name, 1, qh, ())),
                "zwlr_gamma_control_manager_v1" => state.manager = Some(registry.bind(name, 1, qh, ())),
                _ => {}
            }
        }
    }
}
impl Dispatch<WlOutput, ()> for State {
    fn event(_: &mut Self, _: &WlOutput, _: wayland_client::protocol::wl_output::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwlrGammaControlManagerV1, ()> for State {
    fn event(_: &mut Self, _: &ZwlrGammaControlManagerV1, _: wayland_protocols_wlr::gamma_control::v1::client::zwlr_gamma_control_manager_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwlrGammaControlV1, ()> for State {
    fn event(state: &mut Self, _: &ZwlrGammaControlV1, event: zwlr_gamma_control_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match event {
            zwlr_gamma_control_v1::Event::GammaSize { size } => state.size = Some(size as usize),
            zwlr_gamma_control_v1::Event::Failed => state.failed = true,
            _ => {}
        }
    }
}

fn main() {
    let f: Vec<f64> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let factor = |i: usize| f.get(i).copied().unwrap_or(1.0);
    let conn = Connection::connect_to_env().expect("a Wayland display");
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    conn.display().get_registry(&qh, ());
    let mut state = State::default();
    queue.roundtrip(&mut state).unwrap();
    let (Some(manager), Some(output)) = (state.manager.clone(), state.output.clone()) else {
        println!("failed: no gamma control");
        std::process::exit(1);
    };
    let control = manager.get_gamma_control(&output, &qh, ());
    queue.roundtrip(&mut state).unwrap();
    let Some(size) = state.size.filter(|_| !state.failed) else {
        println!("failed");
        std::process::exit(1);
    };
    println!("size {size}");
    let mut bytes = Vec::with_capacity(size * 6);
    for ch in 0..3 {
        for i in 0..size {
            let v = (i as f64 / (size - 1) as f64 * factor(ch) * 65535.0).round() as u16;
            bytes.extend_from_slice(&v.to_ne_bytes());
        }
    }
    let mut file = std::fs::File::from(std::os::fd::OwnedFd::from(tempfile()));
    file.write_all(&bytes).unwrap();
    file.rewind().unwrap();
    control.set_gamma(file.as_fd());
    queue.roundtrip(&mut state).unwrap();
    println!("{}", if state.failed { "failed" } else { "set" });
    std::io::stdout().flush().unwrap();
    loop {
        if queue.blocking_dispatch(&mut state).is_err() {
            break;
        }
    }
}

/// An unlinked file in the runtime directory, for the ramps.
fn tempfile() -> std::fs::File {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let path = dir.join(format!("gammaset-{}", std::process::id()));
    let file = std::fs::OpenOptions::new().read(true).write(true).create_new(true).open(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    file
}
