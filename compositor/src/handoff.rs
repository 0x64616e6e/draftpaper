//! Leaving the displays to the next compositor without a flicker (the login screen to the session, the
//! session back to the login screen). On the way out every output draws its last frame (the session:
//! the bare sheet; the login screen: the sheet blurred, the name in the title block) without the
//! pointer, and that frame stays on screen, for the next compositor to open on (opening.rs):
//!
//! - its framebuffer is let go of with CLOSEFB (Linux 6.8) instead of being removed (RMFB, which takes
//!   the plane down: black), and the device state is left as it is (Smithay's surfaces would clear it);
//! - a child, `draftpaper-compositor hold`, keeps the device open until another compositor shows a
//!   frame of its own (at most 30 s): were it closed by everyone, the kernel's console emulation would
//!   put its own picture up (drm_lastclose). The child opens the device while this compositor is still
//!   its master, so it never becomes master itself.
//!
//! The next compositor takes the displays over as they are (tty.rs: no reset, the same CRTC; in the same
//! mode its first commit needs no modeset). DRAFTPAPER_LAST_FRAME=DIR also writes the last frame of each
//! output to DIR/last-<output>.png (for tests: compared with the next compositor's first).

use std::{
    collections::HashSet,
    os::fd::{AsFd, BorrowedFd},
    time::{Duration, Instant},
};

use smithay::reexports::{
    calloop::EventLoop,
    drm::control::{crtc, framebuffer, Device as ControlDevice},
};

use crate::Draftpaper;

/// How long the holder waits for the next compositor at most.
const HOLD: Duration = Duration::from_secs(30);

/// Draw the last frame on every output and keep it there (see above). After the event loop has ended.
pub fn leave(event_loop: &mut EventLoop<'static, Draftpaper>, data: &mut Draftpaper) {
    let last = std::env::var_os("DRAFTPAPER_LAST_FRAME").map(std::path::PathBuf::from);
    if data.headless.is_some() {
        data.leaving = Some(HashSet::new());
        if let Some(dir) = last {
            crate::headless::capture_into(data, &dir);
        }
        return;
    }
    let Some(tty) = data.tty_data.as_ref() else { return };
    // on another VT, or the displays off: nothing to keep
    if !smithay::backend::session::Session::is_active(&tty.session) || data.screens_off {
        eprintln!("draftpaper: leaving, nothing to keep (another VT, or the displays off)");
        return;
    }
    let began = Instant::now();
    let crtcs: Vec<crtc::Handle> = crate::tty::shown(tty).into_iter().map(|(c, _)| c).collect();
    data.leaving = Some(crtcs.iter().copied().collect());
    for &crtc in &crtcs {
        crate::tty::render_crtc(data, crtc);
    }
    // until every output has its last frame on screen (tty.rs takes them off `leaving`), then a few
    // frames more for a flip still on its way
    let deadline = Instant::now() + Duration::from_secs(1);
    while data.leaving.as_ref().is_some_and(|l| !l.is_empty()) && Instant::now() < deadline {
        let _ = event_loop.dispatch(Some(Duration::from_millis(16)), data);
    }
    match data.leaving.as_ref().filter(|l| !l.is_empty()) {
        Some(l) => eprintln!("draftpaper: leaving: {} output(s) not settled after {} ms", l.len(), began.elapsed().as_millis()),
        None => eprintln!("draftpaper: leaving: last frames on screen after {} ms", began.elapsed().as_millis()),
    }
    let settle = Instant::now() + Duration::from_millis(50);
    while Instant::now() < settle {
        let _ = event_loop.dispatch(Some(Duration::from_millis(16)), data);
    }
    if let Some(dir) = last {
        crate::tty::capture_into(data, &dir);
    }
    keep(data);
}

/// Let go of the framebuffers on screen without taking them down, start the holder, and leave the device
/// state alone: the TTY data is never dropped.
fn keep(data: &mut Draftpaper) {
    let Some(tty) = data.tty_data.take() else { return };
    let shown = crate::tty::shown(&tty);
    let device = crate::tty::device(&tty);
    let mut kept = Vec::new();
    for (crtc, fb) in shown {
        match closefb(device.as_fd(), fb) {
            Ok(()) => {
                eprintln!("draftpaper: crtc {}: framebuffer {} kept on screen", u32::from(crtc), u32::from(fb));
                kept.push((crtc, fb))
            }
            Err(e) => eprintln!("draftpaper: keeping the last frame: {e} (Linux 6.8 or later keeps it)"),
        }
    }
    if !kept.is_empty() {
        hold(&crate::tty::device_path(&tty), &kept);
    }
    eprintln!("draftpaper: left {} output(s) showing the sheet", kept.len());
    // Smithay's surfaces would clear the displays when dropped, and its framebuffers remove themselves
    std::mem::forget(tty);
}

/// struct drm_mode_closefb
#[repr(C)]
struct CloseFb {
    fb_id: u32,
    pad: u32,
}

/// DRM_IOCTL_MODE_CLOSEFB: drop this file's hold on the framebuffer without disabling the planes that
/// show it.
fn closefb(fd: BorrowedFd<'_>, fb: framebuffer::Handle) -> std::io::Result<()> {
    use smithay::reexports::rustix::ioctl::{ioctl, opcode, Updater};
    let mut arg = CloseFb { fb_id: fb.into(), pad: 0 };
    unsafe { ioctl(fd, Updater::<{ opcode::read_write::<CloseFb>(b'd', 0xD0) }, CloseFb>::new(&mut arg)) }.map_err(std::io::Error::from)
}

/// Start the holder with a descriptor of the device of its own, opened now (this compositor is master,
/// so the new one is not).
fn hold(path: &std::path::Path, kept: &[(crtc::Handle, framebuffer::Handle)]) {
    let file = match std::fs::OpenOptions::new().read(true).write(true).open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("draftpaper: holding {}: {e}", path.display());
            return;
        }
    };
    let Ok(exe) = std::env::current_exe() else { return };
    let mut command = std::process::Command::new(exe);
    command.arg("hold");
    for (crtc, fb) in kept {
        command.arg(format!("{}={}", u32::from(*crtc), u32::from(*fb)));
    }
    use std::os::unix::process::CommandExt;
    // its own process group: not taken along by a signal to this one's
    command.process_group(0).stdin(file).stdout(std::process::Stdio::null());
    match command.spawn() {
        Ok(child) => eprintln!("draftpaper: holder {} keeps {} open", child.id(), path.display()),
        Err(e) => eprintln!("draftpaper: holder: {e}"),
    }
}

/// The device on stdin, for the holder.
struct Card(std::fs::File);

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl smithay::reexports::drm::Device for Card {}
impl ControlDevice for Card {}

/// `draftpaper-compositor hold CRTC=FB...`, the device on stdin: wait until none of the CRTCs shows its
/// framebuffer any more (another compositor's first frame is up), at most 30 s, then close it.
pub fn holder(args: &[String]) -> i32 {
    use std::os::fd::{FromRawFd, OwnedFd};
    let card = Card(std::fs::File::from(unsafe { OwnedFd::from_raw_fd(0) }));
    let pairs: Vec<(crtc::Handle, u32)> = args
        .iter()
        .filter_map(|a| {
            let (c, f) = a.split_once('=')?;
            let c = smithay::reexports::drm::control::from_u32::<crtc::Handle>(c.parse().ok()?)?;
            Some((c, f.parse().ok()?))
        })
        .collect();
    // what each CRTC shows instead, and when (0: nothing, the display dark), for the log
    let began = Instant::now();
    let deadline = began + HOLD;
    let mut seen: Vec<Option<u32>> = vec![None; pairs.len()];
    while Instant::now() < deadline {
        let mut shown = false;
        for (i, (c, f)) in pairs.iter().enumerate() {
            let now = card.get_crtc(*c).ok().map(|info| info.framebuffer().map(u32::from).unwrap_or(0));
            if now == Some(*f) {
                shown = true;
            } else if seen[i] != now {
                seen[i] = now;
                eprintln!("draftpaper-hold: crtc {} shows framebuffer {} after {} ms", u32::from(*c), now.unwrap_or(0), began.elapsed().as_millis());
            }
        }
        if !shown {
            eprintln!("draftpaper-hold: taken over after {} ms", began.elapsed().as_millis());
            return 0;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    eprintln!("draftpaper-hold: nobody took over in {} s", HOLD.as_secs());
    0
}
