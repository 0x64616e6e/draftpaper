//! Window rules: `[[rule]]` tables in config.toml, matched against a new window, deciding whether it
//! floats, whether it follows you from workspace to workspace (sticky), where a floating window goes,
//! and whether it takes the keyboard when it opens:
//!
//!     [[rule]]
//!     app_id = "^pavucontrol$"         # a pattern (regex) for the app id (X11: the class)
//!     title = "Volume"                 # and/or one for the title
//!     float = true
//!     sticky = false                   # on every workspace of its screen
//!     place = "centre"                 # or "corner": the frame's bottom-right, above the title block
//!     focus = true                     # false: it opens without taking the keyboard
//!
//! A rule may also match `pip = true`: a browser's picture-in-picture window. One rule is built in, before
//! yours: picture-in-picture floats, sticky, in the corner, without taking the focus. Later rules win:
//! a rule of yours matching `pip = true` changes what you name in it.

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Centre,
    Corner,
}

#[derive(Debug, Clone, Default)]
pub struct Rule {
    app_id: Option<Regex>,
    title: Option<Regex>,
    pip: Option<bool>,
    float: Option<bool>,
    sticky: Option<bool>,
    place: Option<Place>,
    focus: Option<bool>,
}

/// What is known of a window when it opens.
pub struct Facts {
    pub app_id: String,
    pub title: String,
    /// a browser's picture-in-picture window
    pub pip: bool,
}

/// What the rules decide for a window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decision {
    pub float: bool,
    pub sticky: bool,
    pub place: Place,
    pub focus: bool,
}

/// Kept with a window that follows you between workspaces.
pub struct Sticky;
/// Kept with a floating window placed in the corner.
pub struct Cornered;

pub fn is_sticky(window: &smithay::desktop::Window) -> bool {
    window.user_data().get::<Sticky>().is_some()
}

pub fn is_cornered(window: &smithay::desktop::Window) -> bool {
    window.user_data().get::<Cornered>().is_some()
}

/// The rules every config starts with: picture-in-picture as an overlay.
pub fn defaults() -> Vec<Rule> {
    vec![Rule { pip: Some(true), float: Some(true), sticky: Some(true), place: Some(Place::Corner), focus: Some(false), ..Default::default() }]
}

/// The `[[rule]]` tables of config.toml.
pub fn parse(table: &toml::Table) -> Result<Vec<Rule>, String> {
    let Some(list) = table.get("rule") else { return Ok(Vec::new()) };
    let list = list.as_array().ok_or("rule: a list of [[rule]] tables")?;
    list.iter()
        .enumerate()
        .map(|(n, r)| {
            let r = r.as_table().ok_or(format!("rule {}: not a table", n + 1))?;
            let pattern = |k: &str| -> Result<Option<Regex>, String> {
                match r.get(k).map(|v| v.as_str().ok_or(format!("rule {}: {k} is a string", n + 1))) {
                    None => Ok(None),
                    Some(s) => Regex::new(s?).map(Some).map_err(|e| format!("rule {}: {k}: {e}", n + 1)),
                }
            };
            let flag = |k: &str| -> Result<Option<bool>, String> {
                r.get(k).map(|v| v.as_bool().ok_or(format!("rule {}: {k} is true or false", n + 1))).transpose()
            };
            let place = match r.get("place").map(|v| v.as_str()) {
                None => None,
                Some(Some("centre" | "center")) => Some(Place::Centre),
                Some(Some("corner")) => Some(Place::Corner),
                Some(_) => return Err(format!("rule {}: place is \"centre\" or \"corner\"", n + 1)),
            };
            for k in r.keys() {
                if !["app_id", "title", "pip", "float", "sticky", "place", "focus"].contains(&k.as_str()) {
                    return Err(format!("rule {}: unknown key {k}", n + 1));
                }
            }
            Ok(Rule {
                app_id: pattern("app_id")?,
                title: pattern("title")?,
                pip: flag("pip")?,
                float: flag("float")?,
                sticky: flag("sticky")?,
                place,
                focus: flag("focus")?,
            })
        })
        .collect()
}

impl Rule {
    fn matches(&self, f: &Facts) -> bool {
        self.app_id.as_ref().is_none_or(|re| re.is_match(&f.app_id))
            && self.title.as_ref().is_none_or(|re| re.is_match(&f.title))
            && self.pip.is_none_or(|p| p == f.pip)
    }
}

/// Every matching rule in turn, each setting what it names.
pub fn decide(rules: &[Rule], f: &Facts) -> Decision {
    let mut d = Decision { float: false, sticky: false, place: Place::Centre, focus: true };
    for r in rules.iter().filter(|r| r.matches(f)) {
        d.float = r.float.unwrap_or(d.float);
        d.sticky = r.sticky.unwrap_or(d.sticky);
        d.place = r.place.unwrap_or(d.place);
        d.focus = r.focus.unwrap_or(d.focus);
    }
    d
}

/// Whether a window is a browser's picture-in-picture: titled so (Chromium's video one, "Picture in
/// picture", which has no app id; Firefox's "Picture-in-Picture"), or Chromium's document one (Meet's),
/// titled after its page: it alone sets a maximum size, and its title lacks the " - Chromium" its
/// browser's other windows carry.
pub fn looks_like_pip(title: &str, bounded: bool, sibling_titles: &[String]) -> bool {
    let browser = |t: &str| t.ends_with(" - Chromium") || t.ends_with(" - Google Chrome");
    matches!(title, "Picture in picture" | "Picture-in-Picture")
        || (bounded && !browser(title) && sibling_titles.iter().any(|t| browser(t)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(app_id: &str, title: &str, pip: bool) -> Facts {
        Facts { app_id: app_id.into(), title: title.into(), pip }
    }

    #[test]
    fn pip_floats_in_the_corner_by_default() {
        let d = decide(&defaults(), &facts("chromium", "Meet - Daily", true));
        assert_eq!(d, Decision { float: true, sticky: true, place: Place::Corner, focus: false });
        let d = decide(&defaults(), &facts("foot", "foot", false));
        assert_eq!(d, Decision { float: false, sticky: false, place: Place::Centre, focus: true });
    }

    #[test]
    fn later_rules_win_and_patterns_match() {
        let t: toml::Table = toml::from_str(
            "[[rule]]\napp_id = '^pavu'\nfloat = true\n[[rule]]\npip = true\nsticky = false\n",
        )
        .unwrap();
        let mut rules = defaults();
        rules.extend(parse(&t).unwrap());
        assert!(decide(&rules, &facts("pavucontrol", "Volume", false)).float);
        let d = decide(&rules, &facts("chromium", "Picture in picture", true));
        assert!(d.float && !d.sticky && d.place == Place::Corner);
        assert!(parse(&toml::from_str("[[rule]]\nplace = 'left'\n").unwrap()).is_err());
        assert!(parse(&toml::from_str("[[rule]]\ntitle = '('\n").unwrap()).is_err());
        assert!(parse(&toml::from_str("[[rule]]\nfloats = true\n").unwrap()).is_err());
    }

    #[test]
    fn picture_in_picture_is_recognised() {
        let browser = vec!["Opener page - Chromium".to_string()];
        assert!(looks_like_pip("Picture in picture", true, &[]));
        assert!(looks_like_pip("Picture-in-Picture", false, &[]));
        assert!(looks_like_pip("Meet - Daily", true, &browser));
        assert!(!looks_like_pip("Meet - Daily", false, &browser), "no maximum size: an ordinary window");
        assert!(!looks_like_pip("Opener page - Chromium", true, &browser));
        assert!(!looks_like_pip("Some app", true, &["Other app".to_string()]), "not from a browser");
    }
}
