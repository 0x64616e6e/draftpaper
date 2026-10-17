//! The programs a terminal (or a tmux pane) runs again when it comes back: editors, pagers, file
//! managers, monitors. Programs that change things (a build, a script, ssh) are never run again on
//! their own. The compositor keeps the same list (compositor/src/session.rs, RESUMED).

pub const RESUMED: &[&str] = &[
    "nvim", "vim", "vi", "view", "nano", "micro", "hx", "helix", "kak", "emacs", "mg", "less", "more", "man", "htop", "btop", "top", "ranger", "lf",
    "nnn", "yazi", "mc", "ncdu", "cmus",
];

fn argv(pid: u32) -> Vec<String> {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|b| b.split(|c| *c == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect())
        .unwrap_or_default()
}

/// Whether a command line is one to run again (Emacs only in the terminal: a graphical one comes back
/// on its own).
pub fn resumable(argv: &[String]) -> bool {
    let Some(program) = argv.first().map(|a| a.rsplit('/').next().unwrap_or(a)) else { return false };
    RESUMED.contains(&program) && (program != "emacs" || argv.iter().any(|a| a == "-nw" || a == "--no-window-system" || a == "-t" || a == "--tty"))
}

/// The program `pid` (a shell, or the program itself) runs, as its command line, if one to run again.
pub fn resumed(pid: u32) -> Option<Vec<String>> {
    let own = argv(pid);
    if resumable(&own) {
        return Some(own);
    }
    let children = std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")).ok()?;
    children.split_whitespace().filter_map(|c| c.parse::<u32>().ok()).map(argv).find(|a| resumable(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_come_back() {
        let v = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        assert!(resumable(&v("nvim notes.txt")));
        assert!(resumable(&v("/usr/bin/less README")));
        assert!(resumable(&v("emacs -nw x.el")));
        assert!(!resumable(&v("emacs x.el")));
        assert!(!resumable(&v("make deploy")));
        assert!(!resumable(&v("ssh host")));
    }
}
