//! Colour correction (issue #50): each display's gamma table holds its ramps, red, green and blue. They are
//! the ICC profile's calibration curves when config.toml names one for the display (`[outputs."<model>"]
//! icc = "~/.local/share/icc/t430s.icc"`, icc.rs), else a straight line; and while a colour tool such as
//! gammastep or wlsunset holds the display through wlr-gamma-control (v1), the tool's ramps, until it
//! lets go or quits. Loaded again when the display comes, when config.toml changes and when the session
//! comes back (a VT switch).
//!
//! The backends carry them out: the TTY into the CRTC's gamma table (tty::gamma_size, tty::set_gamma);
//! headless keeps them, for the tests and `msg gamma`; nested in a window there is no table.

use std::collections::HashMap;
use std::io::Read;

use smithay::{
    output::Output,
    reexports::{
        wayland_protocols_wlr::gamma_control::v1::server::{
            zwlr_gamma_control_manager_v1::{self, ZwlrGammaControlManagerV1},
            zwlr_gamma_control_v1::{self, ZwlrGammaControlV1},
        },
        wayland_server::{backend::ClientId, Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New},
    },
};

use crate::Draftpaper;

pub type Ramps = [Vec<u16>; 3];

/// Who holds each display's gamma, and what headless keeps in place of a table.
#[derive(Default)]
pub struct Gamma {
    /// by output name: the tool's control, and its ramps once it has set them
    controls: HashMap<String, (ZwlrGammaControlV1, Option<Ramps>)>,
    /// each output's ramps as last loaded, and where they came from: icc, client, identity (`msg gamma`)
    pub kept: HashMap<String, (Ramps, &'static str)>,
}

pub fn init(dh: &DisplayHandle) {
    dh.create_global::<Draftpaper, ZwlrGammaControlManagerV1, ()>(1, ());
}

/// The size of the output's gamma table (entries per channel); None where there is none.
pub fn size(data: &Draftpaper, output: &Output) -> Option<usize> {
    if data.tty_data.is_some() {
        crate::tty::gamma_size(data, output)
    } else if data.headless.is_some() {
        Some(256)
    } else {
        None
    }
}

fn set(data: &mut Draftpaper, output: &Output, ramps: &Ramps, source: &'static str) -> bool {
    let done = if data.tty_data.is_some() { crate::tty::set_gamma(data, output, ramps) } else { data.headless.is_some() };
    // as loaded, for `msg gamma` (and, headless, in place of a table)
    if done {
        data.gamma.kept.insert(output.name(), (ramps.clone(), source));
    }
    done
}

/// The output's own ramps: its ICC profile's curves, else a straight line.
fn own(output: &Output, size: usize) -> (Ramps, &'static str) {
    let settings = crate::theme::output_settings(&output.name(), &output.physical_properties().model);
    if let Some(path) = settings.icc {
        match std::fs::read(&path) {
            Ok(bytes) => match crate::icc::ramps(&bytes, size) {
                Some(r) => return (r, "icc"),
                None => eprintln!("draftpaper: {}: no calibration curves (vcgt) in it: {}'s gamma left straight", path.display(), output.name()),
            },
            Err(e) => eprintln!("draftpaper: {}: {e}", path.display()),
        }
    }
    (crate::icc::identity(size), "identity")
}

/// Load the output's ramps: a tool's while it holds them, else the output's own.
pub fn refresh(data: &mut Draftpaper, output: &Output) {
    let Some(size) = size(data, output) else { return };
    if let Some((_, Some(r))) = data.gamma.controls.get(&output.name()) {
        let r = r.clone();
        set(data, output, &r, "client");
        return;
    }
    let (r, source) = own(output, size);
    if set(data, output, &r, source) && source == "icc" {
        eprintln!("draftpaper: {}: the ICC profile's calibration curves loaded", output.name());
    }
}

/// Every output's, as `refresh`.
pub fn refresh_all(data: &mut Draftpaper) {
    for output in data.space.outputs().cloned().collect::<Vec<_>>() {
        refresh(data, &output);
    }
}

impl GlobalDispatch<ZwlrGammaControlManagerV1, ()> for Draftpaper {
    fn bind(_: &mut Self, _: &DisplayHandle, _: &Client, manager: New<ZwlrGammaControlManagerV1>, _: &(), init: &mut DataInit<'_, Self>) {
        init.init(manager, ());
    }
}

impl Dispatch<ZwlrGammaControlManagerV1, ()> for Draftpaper {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ZwlrGammaControlManagerV1,
        request: zwlr_gamma_control_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let zwlr_gamma_control_manager_v1::Request::GetGammaControl { id, output } = request {
            let output = Output::from_resource(&output);
            let name = output.as_ref().map(|o| o.name()).unwrap_or_default();
            let control = init.init(id, name.clone());
            // one tool at a time per display, and only a display with a table
            let size = output.as_ref().and_then(|o| size(state, o));
            match size {
                Some(size) if !state.gamma.controls.contains_key(&name) => {
                    control.gamma_size(size as u32);
                    state.gamma.controls.insert(name, (control, None));
                }
                _ => control.failed(),
            }
        }
    }
}

impl Dispatch<ZwlrGammaControlV1, String> for Draftpaper {
    fn request(
        state: &mut Self,
        _: &Client,
        control: &ZwlrGammaControlV1,
        request: zwlr_gamma_control_v1::Request,
        name: &String,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_gamma_control_v1::Request::SetGamma { fd } => {
                let Some(output) = state.space.outputs().find(|o| &o.name() == name).cloned() else { return };
                let held = state.gamma.controls.get(name).is_some_and(|(c, _)| c == control);
                let Some(size) = size(state, &output).filter(|_| held) else { return };
                // the ramps: red, then green, then blue, `size` u16 each, in the client's byte order
                let mut bytes = Vec::new();
                let read = std::fs::File::from(fd).take((size * 6 + 1) as u64).read_to_end(&mut bytes);
                if read.is_err() || bytes.len() != size * 6 {
                    control.failed();
                    state.gamma.controls.remove(name);
                    refresh(state, &output);
                    return;
                }
                let v: Vec<u16> = bytes.chunks_exact(2).map(|b| u16::from_ne_bytes([b[0], b[1]])).collect();
                let ramps: Ramps = [v[..size].to_vec(), v[size..2 * size].to_vec(), v[2 * size..].to_vec()];
                if let Some(entry) = state.gamma.controls.get_mut(name) {
                    entry.1 = Some(ramps);
                }
                refresh(state, &output);
            }
            zwlr_gamma_control_v1::Request::Destroy => {}
            _ => {}
        }
    }

    // the tool let go or quit: the display's own ramps again
    fn destroyed(state: &mut Self, _: ClientId, control: &ZwlrGammaControlV1, name: &String) {
        if state.gamma.controls.get(name).is_some_and(|(c, _)| c == control) {
            state.gamma.controls.remove(name);
            let output = state.space.outputs().find(|o| &o.name() == name).cloned();
            if let Some(output) = output {
                refresh(state, &output);
            }
        }
    }
}
