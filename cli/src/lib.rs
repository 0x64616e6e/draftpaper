//! Draftpaper's command, as a library: the same modules the `draftpaper` program is built from, so that
//! the sheet's own client (sheet/, docs/sheet-client.md) can read the machine in its own process instead
//! of starting the program every few seconds.

pub mod apps;
pub mod config;
pub mod content;
pub mod dbus;
pub mod greeter;
pub mod machine;
pub mod metrics;
pub mod processes;
pub mod programs;
pub mod readings;
pub mod restore;
pub mod revisions;
pub mod scheme;
pub mod text;
pub mod theme;
pub mod tmux;
pub mod util;
