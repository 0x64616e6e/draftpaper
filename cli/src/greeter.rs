//! The login screen's copy of the user's sheet. The greeter (draftpaper-greeter, under greetd) runs as a
//! user of its own and cannot read anyone's home, so `draftpaper apply` (every login, every theme
//! change) copies what the sheet looks like to /var/lib/draftpaper/greeter/<user>/, laid out as a home
//! the compositor reads: the look's part of config.toml (theme, margin, wallpaper and how it is mounted,
//! the outputs' scales), the user's own themes, the theme for the card, and the wallpaper. The login
//! screen then draws exactly the sheet the session will, and the handoff shows no change.
//!
//! The directory is world-writable and sticky (the package's tmpfiles.d); each user's copy is theirs
//! (0755, files 0644: the wallpaper becomes readable to every local user). Nothing is written when the
//! directory does not exist (greetd not set up), or when <user> there is not this user's own directory.

use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

use serde_json::{json, Value};

use crate::util::Paths;

/// Keys of config.toml that change how the sheet looks (the rest stays home).
const LOOK: &[&str] = &[
    "theme", "margin", "margins", "title_tint", "wallpaper_fit", "wallpaper_position", "wallpaper_brightness",
    "wallpaper_wash", "wallpaper_blur", "outputs", "animations",
];

fn base() -> PathBuf {
    std::env::var_os("DRAFTPAPER_GREETER_DIR").map(PathBuf::from).unwrap_or_else(|| "/var/lib/draftpaper/greeter".into())
}

/// Copy the sheet's look for the login screen (see above); quietly nothing when there is no place for it.
pub fn sync(p: &Paths) {
    let base = base();
    if !base.is_dir() {
        return;
    }
    let Some(user) = std::env::var("USER").ok().or_else(|| std::env::var("LOGNAME").ok()).filter(|u| !u.is_empty() && !u.contains('/')) else { return };
    // this user's uid: the owner of their own cache directory
    p.ensure();
    let Ok(uid) = fs::metadata(&p.cache).map(|m| m.uid()) else { return };
    let dir = base.join(&user);
    match fs::symlink_metadata(&dir) {
        Ok(m) if m.is_dir() && m.uid() == uid => {}
        Ok(_) => return,
        Err(_) => {
            if fs::create_dir(&dir).is_err() {
                return;
            }
        }
    }
    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o755));
    if let Err(e) = copy(p, &dir) {
        eprintln!("draftpaper: the login screen's copy of the sheet: {e}");
    }
}

fn copy(p: &Paths, dir: &Path) -> std::io::Result<()> {
    let conf = dir.join(".config/draftpaper");
    let cache = dir.join(".cache/draftpaper");
    for d in [dir.join(".config"), conf.clone(), dir.join(".cache"), cache.clone()] {
        fs::create_dir_all(&d)?;
        fs::set_permissions(&d, fs::Permissions::from_mode(0o755))?;
    }

    // config.toml: the look's keys, the wallpaper copied next to it
    let user: toml::Table = fs::read_to_string(&p.config).ok().and_then(|t| t.parse().ok()).unwrap_or_default();
    let mut look = toml::Table::new();
    for (k, v) in &user {
        if LOOK.contains(&k.as_str()) {
            look.insert(k.clone(), v.clone());
        }
    }
    let wallpaper = user.get("wallpaper").and_then(|v| v.as_str()).filter(|w| !w.is_empty()).map(|w| match w.strip_prefix("~/") {
        Some(rest) => p.home.join(rest),
        None => PathBuf::from(w),
    });
    let mut kept = None;
    if let Some(src) = wallpaper.filter(|w| w.is_file()) {
        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("img").to_lowercase();
        let name = format!("wallpaper.{ext}");
        let dst = dir.join(&name);
        // copied again only when it changed
        let same = |a: &Path, b: &Path| match (fs::metadata(a), fs::metadata(b)) {
            (Ok(a), Ok(b)) => a.len() == b.len() && a.mtime() <= b.mtime(),
            _ => false,
        };
        if !same(&src, &dst) {
            fs::copy(&src, &dst)?;
        }
        fs::set_permissions(&dst, fs::Permissions::from_mode(0o644))?;
        look.insert("wallpaper".into(), toml::Value::String(format!("~/{name}")));
        kept = Some(name);
    }
    // a wallpaper no longer used goes
    for e in fs::read_dir(dir)?.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n.starts_with("wallpaper.") && Some(&n) != kept.as_ref() {
            let _ = fs::remove_file(e.path());
        }
    }
    write(&conf.join("config.toml"), &toml::to_string(&look).unwrap_or_default())?;

    // the user's own themes (a scheme writes one), as they are
    let themes = conf.join("themes");
    let _ = fs::remove_dir_all(&themes);
    let own = p.themes()[0].clone();
    if let Ok(entries) = fs::read_dir(&own) {
        fs::create_dir_all(&themes)?;
        fs::set_permissions(&themes, fs::Permissions::from_mode(0o755))?;
        for e in entries.flatten() {
            if e.path().extension().is_some_and(|x| x == "toml") {
                fs::copy(e.path(), themes.join(e.file_name()))?;
                fs::set_permissions(themes.join(e.file_name()), fs::Permissions::from_mode(0o644))?;
            }
        }
    }

    // the card's colours: theme.json's, without what only the session uses (commands, cells...)
    let shell: Value = fs::read_to_string(p.theme_json()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
    let mut card = serde_json::Map::new();
    for k in ["name", "label", "paper", "ink", "inkAlpha", "alert", "strong", "accent", "glyphs", "animations"] {
        if let Some(v) = shell.get(k) {
            card.insert(k.into(), v.clone());
        }
    }
    write(&cache.join("theme.json"), &Value::Object(card).to_string())?;
    // when it was written: draftpaper-greeter shows the copy written last
    write(&dir.join(".synced"), "")
}

fn write(path: &Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o644))?;
    fs::rename(&tmp, path)
}
