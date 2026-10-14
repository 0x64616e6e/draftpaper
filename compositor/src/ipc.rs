//! The IPC: a Unix socket at $XDG_RUNTIME_DIR/draftpaper-<wayland display>.sock (in DRAFTPAPER_SOCKET for
//! the session's programs), newline-delimited JSON. A client writes one request per line and reads one
//! reply per line, in order:
//!
//!   {"request": "version" | "outputs" | "workspaces" | "windows"}
//!   {"request": "action", "action": "workspace 3"}        any binding action (config.rs)
//!   {"request": "subscribe", "events": ["workspaces", "outputs", "windows"]}
//!   {"request": "sheet", "output": "eDP-1", "merged": {"top": [[7, 9]]}}   cells the shell merged
//!
//! Replies are {"ok": true, "reply": ...} or {"ok": false, "error": "..."}. After a subscribe the client
//! gets, right away and then whenever it changes, the whole state of each subscribed kind as
//! {"event": "workspaces", "workspaces": [...]} (likewise outputs, windows): clients replace their model
//! and never apply deltas. Outputs carry the sheet's geometry (frame, margin, zones, parking cells), for
//! the shell; windows say whether they are parked, and in which cell.

use std::{
    collections::{HashMap, HashSet},
    io::{ErrorKind, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::{json, Value};
use smithay::{
    desktop::{Window, WindowSurface},
    reexports::calloop::{
        generic::Generic,
        timer::{TimeoutAction, Timer},
        Interest, Mode, PostAction,
    },
    wayland::{compositor::with_states, shell::xdg::XdgToplevelSurfaceData},
};

use crate::Draftpaper;

const KINDS: [&str; 3] = ["workspaces", "outputs", "windows"];

struct Client {
    /// the write half (the read half belongs to the event loop's source)
    stream: UnixStream,
    input: Vec<u8>,
    output: Vec<u8>,
    events: HashSet<String>,
}

#[derive(Default)]
pub struct Ipc {
    pub path: Option<PathBuf>,
    clients: HashMap<u64, Client>,
    next: u64,
    /// the state last sent, per kind
    last: HashMap<&'static str, Value>,
    flush_pending: bool,
}

/// A window's IPC id, given on first sight and kept for its life.
struct WindowId(u64);
static NEXT_WINDOW: AtomicU64 = AtomicU64::new(1);

pub(crate) fn window_id(window: &Window) -> u64 {
    window.user_data().get_or_insert(|| WindowId(NEXT_WINDOW.fetch_add(1, Ordering::Relaxed))).0
}

impl Draftpaper {
    /// Listen on the socket; its path goes into DRAFTPAPER_SOCKET for the programs started from here.
    pub fn start_ipc(&mut self) {
        let dir = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into()));
        let path = dir.join(format!("draftpaper-{}.sock", self.socket_name.to_string_lossy()));
        // the name follows the Wayland socket, which Smithay made sure was free: a file here is stale
        let _ = std::fs::remove_file(&path);
        let listener = match UnixListener::bind(&path).and_then(|l| l.set_nonblocking(true).map(|_| l)) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("draftpaper: no IPC socket at {}: {e}", path.display());
                return;
            }
        };
        std::env::set_var("DRAFTPAPER_SOCKET", &path);
        // nested in i3 or sway, the session's programs must not drive the outer window manager
        std::env::remove_var("I3SOCK");
        std::env::remove_var("SWAYSOCK");
        self.ipc.path = Some(path);
        let _ = self.loop_handle.insert_source(Generic::new(listener, Interest::READ, Mode::Level), |_, listener, data| {
            while let Ok((stream, _)) = listener.accept() {
                data.ipc_accept(stream);
            }
            Ok(PostAction::Continue)
        });
    }

    fn ipc_accept(&mut self, stream: UnixStream) {
        let Ok(writer) = stream.set_nonblocking(true).and_then(|_| stream.try_clone()) else { return };
        let id = self.ipc.next;
        self.ipc.next += 1;
        self.ipc.clients.insert(id, Client { stream: writer, input: Vec::new(), output: Vec::new(), events: HashSet::new() });
        let _ = self.loop_handle.insert_source(Generic::new(stream, Interest::READ, Mode::Level), move |_, stream, data| {
            let mut buf = [0u8; 4096];
            loop {
                // SAFETY of `get_mut`: reading does not close or replace the fd the source watches
                match unsafe { stream.get_mut() }.read(&mut buf) {
                    Ok(0) => {
                        data.ipc.clients.remove(&id);
                        return Ok(PostAction::Remove);
                    }
                    Ok(n) => {
                        if let Some(c) = data.ipc.clients.get_mut(&id) {
                            c.input.extend_from_slice(&buf[..n]);
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => {
                        data.ipc.clients.remove(&id);
                        return Ok(PostAction::Remove);
                    }
                }
            }
            data.ipc_requests(id);
            Ok(PostAction::Continue)
        });
    }

    /// Answer every complete line the client has sent.
    fn ipc_requests(&mut self, id: u64) {
        loop {
            let Some(c) = self.ipc.clients.get_mut(&id) else { return };
            let Some(end) = c.input.iter().position(|b| *b == b'\n') else { return };
            let line: Vec<u8> = c.input.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line);
            if line.trim().is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(&line) {
                Ok(request) => self.ipc_handle(id, &request),
                Err(e) => Err(format!("not JSON: {e}")),
            };
            let reply = match reply {
                Ok(v) => json!({ "ok": true, "reply": v }),
                Err(e) => json!({ "ok": false, "error": e }),
            };
            self.ipc_send(id, &reply);
            // a subscription is answered with the current state, after the reply
            if let Some(kinds) = request_kinds(&line) {
                for kind in kinds {
                    let state = self.ipc_state(kind);
                    self.ipc_send(id, &json!({ "event": kind, kind: state.clone() }));
                    // the other subscribers are sent a change by the next refresh; this one has it now
                    if self.ipc.last.get(kind) != Some(&state) {
                        let others: Vec<u64> = self.ipc.clients.iter().filter(|(c, cl)| **c != id && cl.events.contains(kind)).map(|(c, _)| *c).collect();
                        for other in others {
                            self.ipc_send(other, &json!({ "event": kind, kind: state.clone() }));
                        }
                        self.ipc.last.insert(kind, state);
                    }
                }
            }
        }
    }

    fn ipc_handle(&mut self, id: u64, request: &Value) -> Result<Value, String> {
        match request.get("request").and_then(Value::as_str).ok_or("no \"request\"")? {
            "version" => Ok(json!({ "name": "draftpaper", "version": env!("CARGO_PKG_VERSION") })),
            "outputs" => Ok(self.ipc_outputs()),
            "workspaces" => Ok(self.ipc_workspaces()),
            "windows" => Ok(self.ipc_windows()),
            "action" => {
                let text = request.get("action").and_then(Value::as_str).ok_or("no \"action\"")?;
                match crate::config::parse_action(text)? {
                    Some(action) => {
                        self.act(action);
                        Ok(Value::Null)
                    }
                    None => Ok(Value::Null),
                }
            }
            "sheet" => {
                // the shell's merged cells: {"output": NAME, "merged": {"top": [[FROM, TO], ...], ...}},
                // zone ranges counted from the frame's corner; the ticks inside each are not drawn
                let name = request.get("output").and_then(Value::as_str).ok_or("no \"output\"")?;
                let screen = self.screens.iter_mut().find(|s| s.output.name() == name).ok_or(format!("no output {name}"))?;
                let merged = request.get("merged").and_then(Value::as_object).ok_or("no \"merged\"")?;
                let mut hidden = [0u32; 4];
                for (side, mask) in ["top", "bottom", "left", "right"].iter().zip(hidden.iter_mut()) {
                    for range in merged.get(*side).and_then(Value::as_array).into_iter().flatten() {
                        let (from, to) = (range.get(0).and_then(Value::as_u64), range.get(1).and_then(Value::as_u64));
                        let (Some(from), Some(to)) = (from, to) else { return Err(format!("{side}: ranges are [FROM, TO]")) };
                        for k in (from + 1)..=to.min(23) {
                            *mask |= 1 << k;
                        }
                    }
                }
                screen.hidden_ticks = hidden;
                Ok(Value::Null)
            }
            "subscribe" => {
                let kinds = request.get("events").and_then(Value::as_array).ok_or("no \"events\"")?;
                let mut wanted = Vec::new();
                for k in kinds {
                    match k.as_str().filter(|k| KINDS.contains(k)) {
                        Some(k) => wanted.push(k.to_string()),
                        None => return Err(format!("no such event {k}; there are {}", KINDS.join(", "))),
                    }
                }
                if let Some(c) = self.ipc.clients.get_mut(&id) {
                    c.events.extend(wanted);
                }
                Ok(Value::Null)
            }
            other => Err(format!("no such request \"{other}\"")),
        }
    }

    fn ipc_state(&self, kind: &str) -> Value {
        match kind {
            "outputs" => self.ipc_outputs(),
            "workspaces" => self.ipc_workspaces(),
            _ => self.ipc_windows(),
        }
    }

    fn ipc_outputs(&self) -> Value {
        let focused = self.active;
        Value::Array(
            self.screens
                .iter()
                .enumerate()
                .filter_map(|(i, s)| {
                    let g = self.space.output_geometry(&s.output)?;
                    let margin = (s.margin_mm * s.mm).round();
                    let frame = (s.grid.origin.x, s.grid.origin.y, g.size.w as f64 - 2.0 * margin, g.size.h as f64 - 2.0 * margin);
                    Some(json!({
                        "name": s.output.name(),
                        "focused": i == focused,
                        "x": g.loc.x, "y": g.loc.y, "width": g.size.w, "height": g.size.h,
                        "scale": s.output.current_scale().fractional_scale(),
                        "workspace": s.workspace().number,
                        // the sheet, in logical pixels
                        "px_per_mm": s.mm,
                        "margin": margin,
                        "frame": { "x": frame.0, "y": frame.1, "width": frame.2, "height": frame.3 },
                        "zones": { "across": s.grid.zones.0, "down": s.grid.zones.1, "width": s.grid.zone.0, "height": s.grid.zone.1 },
                        // parking cells: down the left margin, then the right, one per zone
                        "cells": self.cells(i),
                    }))
                })
                .collect(),
        )
    }

    fn ipc_workspaces(&self) -> Value {
        let mut all: Vec<Value> = self
            .screens
            .iter()
            .enumerate()
            .flat_map(|(i, s)| {
                s.workspaces.iter().enumerate().map(move |(k, w)| {
                    json!({
                        "number": w.number,
                        "output": s.output.name(),
                        "shown": k == s.current,
                        "focused": k == s.current && i == self.active,
                        "windows": w.layout.windows().count() + w.floating.len(),
                        "parked": w.parked.len(),
                    })
                })
            })
            .collect();
        all.sort_by_key(|w| w["number"].as_u64());
        Value::Array(all)
    }

    fn ipc_windows(&self) -> Value {
        let focused = self.focused_window();
        let mut all = Vec::new();
        for s in &self.screens {
            for (k, w) in s.workspaces.iter().enumerate() {
                let tiled = w.layout.windows().map(|x| (x, false, None));
                let floating = w.floating.iter().map(|x| (x, true, None));
                let parked = w.parked.iter().map(|p| (&p.window, false, Some(p.cell)));
                for (window, floats, cell) in tiled.chain(floating).chain(parked) {
                    let (app_id, title, x11) = names(window);
                    let geometry = self.space.element_location(window).filter(|_| k == s.current && cell.is_none()).map(|loc| {
                        let size = window.geometry().size;
                        json!({ "x": loc.x, "y": loc.y, "width": size.w, "height": size.h })
                    });
                    // what the client draws (all its surfaces), relative to its geometry's corner; larger
                    // than the geometry when it draws more than its tile
                    let (b, g) = (window.bbox(), window.geometry());
                    let surface = json!({ "x": b.loc.x - g.loc.x, "y": b.loc.y - g.loc.y, "width": b.size.w, "height": b.size.h });
                    all.push(json!({
                        "id": window_id(window),
                        "app_id": app_id,
                        "title": title,
                        "xwayland": x11,
                        "workspace": w.number,
                        "output": s.output.name(),
                        "focused": focused.as_ref() == Some(window),
                        "floating": floats,
                        "fullscreen": w.fullscreen.as_ref() == Some(window),
                        "parked": cell.is_some(),
                        "cell": cell,
                        "geometry": geometry,
                        "surface": surface,
                    }));
                }
            }
        }
        all.sort_by_key(|w| w["id"].as_u64());
        Value::Array(all)
    }

    fn ipc_send(&mut self, id: u64, message: &Value) {
        let Some(c) = self.ipc.clients.get_mut(&id) else { return };
        let mut line = message.to_string();
        line.push('\n');
        c.output.extend_from_slice(line.as_bytes());
        if !flush(c) {
            self.ipc.clients.remove(&id);
        }
        self.ipc_flush_later();
    }

    /// After every turn of the event loop: send what changed to its subscribers.
    pub fn ipc_refresh(&mut self) {
        if self.ipc.clients.values().all(|c| c.events.is_empty()) {
            self.ipc.last.clear();
            return;
        }
        for kind in KINDS {
            let state = self.ipc_state(kind);
            if self.ipc.last.get(kind) == Some(&state) {
                continue;
            }
            let ids: Vec<u64> = self.ipc.clients.iter().filter(|(_, c)| c.events.contains(kind)).map(|(id, _)| *id).collect();
            let event = json!({ "event": kind, kind: state.clone() });
            for id in ids {
                self.ipc_send(id, &event);
            }
            self.ipc.last.insert(kind, state);
        }
    }

    /// A client that reads slowly keeps what it has not read; try again shortly.
    fn ipc_flush_later(&mut self) {
        if self.ipc.flush_pending || self.ipc.clients.values().all(|c| c.output.is_empty()) {
            return;
        }
        self.ipc.flush_pending = true;
        let _ = self.loop_handle.insert_source(Timer::from_duration(std::time::Duration::from_millis(20)), |_, _, data| {
            data.ipc.flush_pending = false;
            data.ipc.clients.retain(|_, c| flush(c));
            data.ipc_flush_later();
            TimeoutAction::Drop
        });
    }

    /// The socket goes with the compositor.
    pub fn stop_ipc(&mut self) {
        if let Some(path) = self.ipc.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Write what the socket takes; false when the client is gone or a megabyte behind.
fn flush(c: &mut Client) -> bool {
    while !c.output.is_empty() {
        match c.stream.write(&c.output) {
            Ok(0) => return false,
            Ok(n) => {
                c.output.drain(..n);
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => return c.output.len() < 1 << 20,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => return false,
        }
    }
    true
}

/// The kinds a subscribe line asks for (to send their state after the reply).
fn request_kinds(line: &str) -> Option<Vec<&'static str>> {
    let v: Value = serde_json::from_str(line).ok()?;
    (v.get("request")?.as_str()? == "subscribe").then_some(())?;
    let asked = v.get("events")?.as_array()?;
    let kinds: Vec<&'static str> = KINDS.into_iter().filter(|k| asked.iter().any(|a| a.as_str() == Some(k))).collect();
    // only a subscribe that succeeded: every name known
    (kinds.len() == asked.len()).then_some(kinds)
}

/// app_id (X11: the class), title, and whether it is an X11 window.
fn names(window: &Window) -> (String, String, bool) {
    match window.underlying_surface() {
        WindowSurface::Wayland(t) => with_states(t.wl_surface(), |states| {
            let data = states.data_map.get::<XdgToplevelSurfaceData>().map(|d| d.lock().unwrap());
            let app_id = data.as_ref().and_then(|d| d.app_id.clone()).unwrap_or_default();
            let title = data.as_ref().and_then(|d| d.title.clone()).unwrap_or_default();
            (app_id, title, false)
        }),
        WindowSurface::X11(x) => (x.class(), x.title(), true),
    }
}

/// `draftpaper-compositor msg ...`: one request, its reply printed; `msg subscribe ...` prints events
/// until the compositor goes. Requests: version, outputs, workspaces, windows, subscribe KIND...,
/// or an action (`msg workspace 3`, `msg spawn foot`).
pub fn msg(args: &[String]) -> i32 {
    let Some(path) = std::env::var_os("DRAFTPAPER_SOCKET") else {
        eprintln!("draftpaper: DRAFTPAPER_SOCKET is not set (run inside a Draftpaper session)");
        return 1;
    };
    let request = match args.first().map(String::as_str) {
        None => {
            eprintln!("usage: draftpaper-compositor msg version|outputs|focused-output|workspaces|windows|subscribe KIND...|ACTION");
            return 2;
        }
        Some(r @ ("version" | "outputs" | "workspaces" | "windows" | "focused-output")) => {
            json!({ "request": if r == "focused-output" { "outputs" } else { r } })
        }
        Some("subscribe") => json!({ "request": "subscribe", "events": args[1..] }),
        Some(_) => json!({ "request": "action", "action": args.join(" ") }),
    };
    let mut stream = match UnixStream::connect(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("draftpaper: {}: {e}", PathBuf::from(&path).display());
            return 1;
        }
    };
    if writeln!(stream, "{request}").is_err() {
        return 1;
    }
    let subscribing = args[0] == "subscribe";
    let mut lines = std::io::BufRead::lines(std::io::BufReader::new(stream));
    let Some(Ok(reply)) = lines.next() else { return 1 };
    let reply: Value = serde_json::from_str(&reply).unwrap_or(Value::Null);
    if reply["ok"] != true {
        eprintln!("draftpaper: {}", reply["error"].as_str().unwrap_or("no reply"));
        return 1;
    }
    if args[0] == "focused-output" {
        // the name alone, for scripts: grim -o "$(draftpaper-compositor msg focused-output)"
        let name = reply["reply"].as_array().and_then(|o| o.iter().find(|o| o["focused"] == true)).and_then(|o| o["name"].as_str());
        return match name {
            Some(n) => {
                println!("{n}");
                0
            }
            None => 1,
        };
    }
    if !subscribing {
        if !reply["reply"].is_null() {
            println!("{}", serde_json::to_string_pretty(&reply["reply"]).unwrap());
        }
        return 0;
    }
    for line in lines.map_while(Result::ok) {
        println!("{line}");
    }
    0
}
