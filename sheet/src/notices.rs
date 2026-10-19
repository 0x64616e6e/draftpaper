//! The notifications the sheet letters along its top margin. This is the desktop's notification server:
//! it takes the name org.freedesktop.Notifications on the session bus, as the shell did through
//! Quickshell, and keeps what has come in until each one's time is up.
//!
//! Only what the margin shows is kept — who sent it, its summary and body, and when it should go. The
//! actions and the icons a notification may carry are not drawn on a drafting sheet, so they are taken
//! and dropped, and GetCapabilities says as much.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// One notification, as the margin letters it.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub id: u32,
    pub summary: String,
    pub body: String,
    /// when it should come off the sheet
    pub until: Instant,
}

impl Notice {
    /// What the margin letters: the summary, and the body after a dash where there is one.
    pub fn text(&self) -> String {
        match (self.summary.trim(), self.body.trim()) {
            ("", body) => body.to_string(),
            (summary, "") => summary.to_string(),
            (summary, body) => format!("{summary} — {body}"),
        }
    }
}

/// What has come in and not yet gone, newest last.
#[derive(Default)]
pub struct Notices {
    held: Mutex<Vec<Notice>>,
    next: Mutex<u32>,
}

impl Notices {
    /// The ones still due, newest last; the rest are dropped as they are asked for.
    pub fn due(&self) -> Vec<Notice> {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        held.retain(|n| n.until > now);
        held.clone()
    }

    fn add(&self, replaces: u32, summary: String, body: String, timeout: i32) -> u32 {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        let id = if replaces > 0 {
            replaces
        } else {
            let mut next = self.next.lock().unwrap_or_else(|e| e.into_inner());
            *next = next.wrapping_add(1).max(1);
            *next
        };
        // what the sender asked for, a sensible while where it asked for nothing, and never for ever:
        // the sheet is a drawing, not a place to pile messages up
        let secs = match timeout {
            t if t > 0 => (t as u64).min(60_000) as f64 / 1000.0,
            _ => 5.0,
        };
        held.retain(|n| n.id != id);
        held.push(Notice { id, summary, body, until: Instant::now() + Duration::from_secs_f64(secs) });
        id
    }

    fn close(&self, id: u32) {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        held.retain(|n| n.id != id);
    }
}

struct Server {
    notices: Arc<Notices>,
    say: std::sync::mpsc::Sender<()>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        _app_name: String,
        replaces_id: u32,
        _app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        _hints: HashMap<String, zbus::zvariant::OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let id = self.notices.add(replaces_id, summary, body, expire_timeout);
        let _ = self.say.send(());
        id
    }

    fn close_notification(&self, id: u32) {
        self.notices.close(id);
        let _ = self.say.send(());
    }

    /// What this server does with a notification: letters one line of it on the sheet. No actions, no
    /// icons, no markup — a drawing's margin carries words.
    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".to_string()]
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        ("draftpaper".into(), "0x64616e6e".into(), env!("CARGO_PKG_VERSION").into(), "1.2".into())
    }
}

/// Take the name on the session bus and serve it until the process ends. Where another server already
/// holds the name (the shell, while both are running), this gives up quietly and the margin stays bare.
pub fn serve(notices: Arc<Notices>, say: std::sync::mpsc::Sender<()>) {
    let server = Server { notices, say };
    let built = zbus::blocking::connection::Builder::session()
        .and_then(|b| b.name("org.freedesktop.Notifications"))
        .and_then(|b| b.serve_at("/org/freedesktop/Notifications", server))
        .and_then(|b| b.build());
    match built {
        Ok(_connection) => std::thread::park(),
        Err(e) => eprintln!("draftpaper-sheet: no notifications: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_comes_in_is_lettered_until_its_time_is_up() {
        let n = Notices::default();
        let a = n.add(0, "One".into(), "the first".into(), 10_000);
        let b = n.add(0, "Two".into(), String::new(), 10_000);
        assert_ne!(a, b);
        let due = n.due();
        assert_eq!(due.len(), 2);
        assert_eq!(due[1].text(), "Two", "the newest is last");
        assert_eq!(due[0].text(), "One — the first");
        // one that replaces another takes its place rather than piling on
        n.add(a, "Again".into(), String::new(), 10_000);
        assert_eq!(n.due().len(), 2);
        assert_eq!(n.due().last().unwrap().text(), "Again");
        // and one closed is gone
        n.close(b);
        assert_eq!(n.due().len(), 1);
    }

    #[test]
    fn one_whose_time_is_up_goes() {
        let n = Notices::default();
        n.add(0, "Brief".into(), String::new(), 1);
        assert_eq!(n.due().len(), 1);
        std::thread::sleep(Duration::from_millis(20));
        assert!(n.due().is_empty(), "a notification does not stay on the sheet");
    }

    #[test]
    fn a_sender_that_asks_for_ever_does_not_get_it() {
        let n = Notices::default();
        // 0 and -1 both mean "you decide": five seconds, as the shell gave them
        n.add(0, "A".into(), String::new(), 0);
        n.add(0, "B".into(), String::new(), -1);
        let due = n.due();
        assert_eq!(due.len(), 2);
        for notice in due {
            assert!(notice.until <= Instant::now() + Duration::from_secs(6));
        }
    }
}
