//! wlr-output-management (v4): tools such as kanshi, wlr-randr and wdisplays see the displays (heads),
//! with their modes, and arrange them: mode, position, scale, transform, on or off. The backend says what
//! the heads are (tty::heads, winit::heads) and carries out a configuration (tty::configure,
//! winit::configure); this module keeps every client's view in step, sending only what changed and then
//! `done` with a new serial, and turns a client's configuration into a list of changes. A configuration
//! made against an old serial is cancelled.

use std::{collections::HashMap, sync::Mutex};

use smithay::{
    reexports::{
        wayland_protocols_wlr::output_management::v1::server::{
            zwlr_output_configuration_head_v1::{self, ZwlrOutputConfigurationHeadV1},
            zwlr_output_configuration_v1::{self, ZwlrOutputConfigurationV1},
            zwlr_output_head_v1::{self, ZwlrOutputHeadV1},
            zwlr_output_manager_v1::{self, ZwlrOutputManagerV1},
            zwlr_output_mode_v1::{self, ZwlrOutputModeV1},
        },
        wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, WEnum},
    },
    utils::Transform,
};

use crate::Draftpaper;

/// A mode of a head: size in pixels, refresh in mHz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModeInfo {
    pub w: i32,
    pub h: i32,
    pub refresh: i32,
    pub preferred: bool,
}

impl ModeInfo {
    fn same(&self, other: &ModeInfo) -> bool {
        self.w == other.w && self.h == other.h && self.refresh == other.refresh
    }
}

/// A display as the backend sees it, on or off.
#[derive(Debug, Clone, PartialEq)]
pub struct HeadInfo {
    pub name: String,
    pub make: String,
    pub model: String,
    pub serial: String,
    /// millimetres
    pub physical: (i32, i32),
    pub modes: Vec<ModeInfo>,
    pub enabled: bool,
    pub current: Option<ModeInfo>,
    pub position: (i32, i32),
    pub transform: Transform,
    pub scale: f64,
}

impl HeadInfo {
    fn description(&self) -> String {
        let parts: Vec<&str> = [self.make.as_str(), self.model.as_str(), self.serial.as_str()].into_iter().filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            self.name.clone()
        } else {
            format!("{} ({})", parts.join(" "), self.name)
        }
    }
}

/// What a configuration asks of one enabled head; None where it leaves things as they are.
#[derive(Debug, Clone, Default)]
pub struct HeadConfig {
    pub mode: Option<ModeInfo>,
    /// set_custom_mode: any size and refresh; the backend takes it only if the display has it
    pub custom: bool,
    pub position: Option<(i32, i32)>,
    pub transform: Option<Transform>,
    pub scale: Option<f64>,
    pub adaptive_sync: Option<bool>,
}

/// One head of a configuration: on with these settings, or off (None).
#[derive(Debug, Clone)]
pub struct HeadChange {
    pub name: String,
    pub config: Option<HeadConfig>,
}

struct HeadEntry {
    head: ZwlrOutputHeadV1,
    modes: Vec<(ModeInfo, ZwlrOutputModeV1)>,
    last: HeadInfo,
}

struct ManagerEntry {
    manager: ZwlrOutputManagerV1,
    heads: HashMap<String, HeadEntry>,
}

#[derive(Default)]
pub struct OutputManagement {
    managers: Vec<ManagerEntry>,
    serial: u32,
    last: Vec<HeadInfo>,
}

/// The head object's data: the name of its display.
pub struct HeadData(String);
/// The mode object's data: its head and the mode.
pub struct ModeData(String, ModeInfo);

pub struct ConfigData {
    serial: u32,
    heads: Vec<(String, Option<ZwlrOutputConfigurationHeadV1>)>,
    used: bool,
}

pub struct ConfigHeadData {
    name: String,
    config: Mutex<HeadConfig>,
}

pub fn init(dh: &DisplayHandle) {
    dh.create_global::<Draftpaper, ZwlrOutputManagerV1, ()>(4, ());
}

/// The heads as they are now, from the backend.
fn heads(data: &Draftpaper) -> Vec<HeadInfo> {
    if data.tty {
        crate::tty::heads(data)
    } else {
        crate::winit::heads(data)
    }
}

impl Draftpaper {
    /// Tell every output-management client what changed since the last time (call after any change to
    /// the outputs: plugged, unplugged, mode, position, scale, transform).
    pub fn refresh_output_management(&mut self) {
        let heads = heads(self);
        if heads == self.output_management.last {
            return;
        }
        self.output_management.serial = self.output_management.serial.wrapping_add(1);
        self.output_management.last = heads.clone();
        let dh = self.display_handle.clone();
        let serial = self.output_management.serial;
        self.output_management.managers.retain(|m| m.manager.is_alive());
        for m in &mut self.output_management.managers {
            update(&dh, m, &heads);
            m.manager.done(serial);
        }
    }
}

/// Bring one client's head objects in line with `heads`.
fn update(dh: &DisplayHandle, m: &mut ManagerEntry, heads: &[HeadInfo]) {
    let gone: Vec<String> = m.heads.keys().filter(|n| !heads.iter().any(|h| &h.name == *n)).cloned().collect();
    for name in gone {
        if let Some(entry) = m.heads.remove(&name) {
            for (_, mode) in &entry.modes {
                if mode.is_alive() {
                    mode.finished();
                }
            }
            if entry.head.is_alive() {
                entry.head.finished();
            }
        }
    }
    for info in heads {
        match m.heads.get_mut(&info.name) {
            Some(entry) => send_changes(dh, entry, info),
            None => {
                let Some(client) = m.manager.client() else { continue };
                let Ok(head) = client.create_resource::<ZwlrOutputHeadV1, HeadData, Draftpaper>(dh, m.manager.version(), HeadData(info.name.clone())) else {
                    continue;
                };
                m.manager.head(&head);
                let mut entry = HeadEntry { head, modes: Vec::new(), last: info.clone() };
                send_all(dh, &mut entry, info);
                m.heads.insert(info.name.clone(), entry);
            }
        }
    }
}

/// A new head: everything about it.
fn send_all(dh: &DisplayHandle, entry: &mut HeadEntry, info: &HeadInfo) {
    let head = entry.head.clone();
    head.name(info.name.clone());
    head.description(info.description());
    if info.physical.0 > 0 && info.physical.1 > 0 {
        head.physical_size(info.physical.0, info.physical.1);
    }
    for mode in &info.modes {
        add_mode(dh, entry, info, *mode);
    }
    if head.version() >= 2 {
        if !info.make.is_empty() {
            head.make(info.make.clone());
        }
        if !info.model.is_empty() {
            head.model(info.model.clone());
        }
        if !info.serial.is_empty() {
            head.serial_number(info.serial.clone());
        }
    }
    send_state(entry, info, None);
}

/// A head that was already known: what changed.
fn send_changes(dh: &DisplayHandle, entry: &mut HeadEntry, info: &HeadInfo) {
    if !entry.head.is_alive() {
        return;
    }
    // modes gone, modes new
    let gone: Vec<usize> = (0..entry.modes.len()).filter(|&i| !info.modes.iter().any(|m| *m == entry.modes[i].0)).collect();
    for i in gone.into_iter().rev() {
        let (_, mode) = entry.modes.remove(i);
        if mode.is_alive() {
            mode.finished();
        }
    }
    for mode in &info.modes {
        if !entry.modes.iter().any(|(m, _)| m == mode) {
            add_mode(dh, entry, info, *mode);
        }
    }
    let last = entry.last.clone();
    send_state(entry, info, Some(&last));
    entry.last = info.clone();
}

fn add_mode(dh: &DisplayHandle, entry: &mut HeadEntry, info: &HeadInfo, mode: ModeInfo) {
    let Some(client) = entry.head.client() else { return };
    let version = entry.head.version().min(3);
    let Ok(object) = client.create_resource::<ZwlrOutputModeV1, ModeData, Draftpaper>(dh, version, ModeData(info.name.clone(), mode)) else {
        return;
    };
    entry.head.mode(&object);
    object.size(mode.w, mode.h);
    if mode.refresh > 0 {
        object.refresh(mode.refresh);
    }
    if mode.preferred {
        object.preferred();
    }
    entry.modes.push((mode, object));
}

/// enabled, current mode, position, transform, scale: all of them, or those that differ from `last`.
fn send_state(entry: &HeadEntry, info: &HeadInfo, last: Option<&HeadInfo>) {
    let head = &entry.head;
    let differs = |f: &dyn Fn(&HeadInfo) -> bool| last.is_none_or(|l| f(l));
    if differs(&|l| l.enabled != info.enabled) {
        head.enabled(info.enabled as i32);
    }
    if !info.enabled {
        if head.version() >= 4 && last.is_none() {
            head.adaptive_sync(zwlr_output_head_v1::AdaptiveSyncState::Disabled);
        }
        return;
    }
    // after turning on, the state is sent whole
    let fresh = last.is_none_or(|l| !l.enabled);
    if let Some(current) = info.current {
        if fresh || last.is_some_and(|l| l.current.is_none_or(|c| !c.same(&current))) {
            if let Some((_, mode)) = entry.modes.iter().find(|(m, _)| m.same(&current)) {
                head.current_mode(mode);
            }
        }
    }
    if fresh || differs(&|l| l.position != info.position) {
        head.position(info.position.0, info.position.1);
    }
    if fresh || differs(&|l| l.transform != info.transform) {
        head.transform(info.transform.into());
    }
    if fresh || differs(&|l| l.scale != info.scale) {
        head.scale(info.scale);
    }
    if head.version() >= 4 && last.is_none() {
        head.adaptive_sync(zwlr_output_head_v1::AdaptiveSyncState::Disabled);
    }
}

impl GlobalDispatch<ZwlrOutputManagerV1, ()> for Draftpaper {
    fn bind(state: &mut Self, dh: &DisplayHandle, _: &Client, manager: New<ZwlrOutputManagerV1>, _: &(), init: &mut DataInit<'_, Self>) {
        let manager = init.init(manager, ());
        // the heads as they are now (refreshed first, so the serial is current)
        state.refresh_output_management();
        let heads = state.output_management.last.clone();
        let mut entry = ManagerEntry { manager: manager.clone(), heads: HashMap::new() };
        update(dh, &mut entry, &heads);
        manager.done(state.output_management.serial);
        state.output_management.managers.push(entry);
    }
}

impl Dispatch<ZwlrOutputManagerV1, ()> for Draftpaper {
    fn request(
        state: &mut Self,
        _: &Client,
        manager: &ZwlrOutputManagerV1,
        request: zwlr_output_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwlr_output_manager_v1::Request::CreateConfiguration { id, serial } => {
                init.init(id, Mutex::new(ConfigData { serial, heads: Vec::new(), used: false }));
            }
            zwlr_output_manager_v1::Request::Stop => {
                state.output_management.managers.retain(|m| &m.manager != manager);
                manager.finished();
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrOutputHeadV1, HeadData> for Draftpaper {
    fn request(_: &mut Self, _: &Client, _: &ZwlrOutputHeadV1, _: zwlr_output_head_v1::Request, _: &HeadData, _: &DisplayHandle, _: &mut DataInit<'_, Self>) {
        // release: the object is destroyed; it is no longer sent anything (is_alive)
    }
}

impl Dispatch<ZwlrOutputModeV1, ModeData> for Draftpaper {
    fn request(_: &mut Self, _: &Client, _: &ZwlrOutputModeV1, _: zwlr_output_mode_v1::Request, _: &ModeData, _: &DisplayHandle, _: &mut DataInit<'_, Self>) {}
}

impl Dispatch<ZwlrOutputConfigurationV1, Mutex<ConfigData>> for Draftpaper {
    fn request(
        state: &mut Self,
        _: &Client,
        config: &ZwlrOutputConfigurationV1,
        request: zwlr_output_configuration_v1::Request,
        data: &Mutex<ConfigData>,
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        use zwlr_output_configuration_v1::{Error, Request};
        let mut d = data.lock().unwrap();
        let head_name = |head: &ZwlrOutputHeadV1| head.data::<HeadData>().map(|h| h.0.clone()).unwrap_or_default();
        match request {
            Request::EnableHead { id, head } => {
                let name = head_name(&head);
                // the new object exists whatever happens next
                let object = init.init(id, ConfigHeadData { name: name.clone(), config: Mutex::new(HeadConfig::default()) });
                if d.used {
                    config.post_error(Error::AlreadyUsed, "the configuration was already applied or tested");
                    return;
                }
                if d.heads.iter().any(|(n, _)| *n == name) {
                    config.post_error(Error::AlreadyConfiguredHead, format!("{name} is already in the configuration"));
                    return;
                }
                d.heads.push((name, Some(object)));
            }
            Request::DisableHead { head } => {
                let name = head_name(&head);
                if d.used {
                    config.post_error(Error::AlreadyUsed, "the configuration was already applied or tested");
                    return;
                }
                if d.heads.iter().any(|(n, _)| *n == name) {
                    config.post_error(Error::AlreadyConfiguredHead, format!("{name} is already in the configuration"));
                    return;
                }
                d.heads.push((name, None));
            }
            Request::Apply | Request::Test => {
                let test = matches!(request, Request::Test);
                if d.used {
                    config.post_error(Error::AlreadyUsed, "the configuration was already applied or tested");
                    return;
                }
                d.used = true;
                if d.serial != state.output_management.serial {
                    config.cancelled();
                    return;
                }
                let current = state.output_management.last.clone();
                if let Some(missing) = current.iter().find(|h| !d.heads.iter().any(|(n, _)| *n == h.name)) {
                    config.post_error(Error::UnconfiguredHead, format!("{} is not in the configuration", missing.name));
                    return;
                }
                let changes: Vec<HeadChange> = d
                    .heads
                    .iter()
                    .filter(|(n, _)| current.iter().any(|h| &h.name == n))
                    .map(|(name, object)| HeadChange {
                        name: name.clone(),
                        config: object.as_ref().map(|o| o.data::<ConfigHeadData>().map(|c| c.config.lock().unwrap().clone()).unwrap_or_default()),
                    })
                    .collect();
                drop(d);
                let result = if state.tty {
                    crate::tty::configure(state, &changes, test)
                } else {
                    crate::winit::configure(state, &changes, test)
                };
                match result {
                    Ok(()) => config.succeeded(),
                    Err(e) => {
                        eprintln!("draftpaper: output configuration {}: {e}", if test { "test" } else { "failed" });
                        config.failed();
                    }
                }
                state.refresh_output_management();
            }
            Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<ZwlrOutputConfigurationHeadV1, ConfigHeadData> for Draftpaper {
    fn request(
        _: &mut Self,
        _: &Client,
        object: &ZwlrOutputConfigurationHeadV1,
        request: zwlr_output_configuration_head_v1::Request,
        data: &ConfigHeadData,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        use zwlr_output_configuration_head_v1::{Error, Request};
        let mut c = data.config.lock().unwrap();
        let already = |object: &ZwlrOutputConfigurationHeadV1, what: &str| object.post_error(Error::AlreadySet, format!("{what} is already set"));
        match request {
            Request::SetMode { mode } => {
                if c.mode.is_some() {
                    return already(object, "the mode");
                }
                match mode.data::<ModeData>() {
                    Some(ModeData(head, m)) if *head == data.name => c.mode = Some(*m),
                    _ => object.post_error(Error::InvalidMode, "the mode is not one of this head's"),
                }
            }
            Request::SetCustomMode { width, height, refresh } => {
                if c.mode.is_some() {
                    return already(object, "the mode");
                }
                if width <= 0 || height <= 0 || refresh < 0 {
                    return object.post_error(Error::InvalidCustomMode, "the size must be positive and the refresh not negative");
                }
                c.mode = Some(ModeInfo { w: width, h: height, refresh, preferred: false });
                c.custom = true;
            }
            Request::SetPosition { x, y } => {
                if c.position.is_some() {
                    return already(object, "the position");
                }
                c.position = Some((x, y));
            }
            Request::SetTransform { transform } => {
                if c.transform.is_some() {
                    return already(object, "the transform");
                }
                match transform {
                    WEnum::Value(t) => c.transform = Some(t.into()),
                    WEnum::Unknown(_) => object.post_error(Error::InvalidTransform, "no such transform"),
                }
            }
            Request::SetScale { scale } => {
                if c.scale.is_some() {
                    return already(object, "the scale");
                }
                if scale <= 0.0 {
                    return object.post_error(Error::InvalidScale, "the scale must be positive");
                }
                c.scale = Some(scale);
            }
            Request::SetAdaptiveSync { state } => {
                if c.adaptive_sync.is_some() {
                    return already(object, "adaptive sync");
                }
                match state {
                    WEnum::Value(s) => c.adaptive_sync = Some(s == zwlr_output_head_v1::AdaptiveSyncState::Enabled),
                    WEnum::Unknown(_) => object.post_error(Error::InvalidAdaptiveSyncState, "no such adaptive sync state"),
                }
            }
            _ => {}
        }
    }
}
