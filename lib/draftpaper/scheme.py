"""Colour schemes built from a few choices, and the contrast rules every theme is checked against.

Display perception research (Legge 1990 and after): luminance contrast between text and its background
carries legibility; hue adds nothing. So a scheme is not painted value by value. It is built from a
few discrete choices, the hues taste and the lightnesses solved: every colour used for words reaches at
least 4.5:1 against what it sits on (7:1 or 10:1 for the main text, by the contrast choice), dimmed text
3:1, the focused window's border 3:1 against the window it frames (against the paper its hue sets it
apart: a light sheet and dark windows leave no colour 3:1 from both). The colours are worked out in OKLCH, so a step in
lightness looks like the same step whatever the hue.

    paper     light | dark            the sheet
    tone      greige sand clay sage graphite frost slate teal indigo blueprint (warm to cold)
    accent    amber orange red rose violet blue teal green
    windows   dark | light            the applications: terminals, GTK, Qt, Firefox
    shade     deep dark charcoal slate    the windows' lightness on their side: from the deepest to a
                                      mid grey (light windows: white paper mist grey)
    terminal  windows neutral tone blue green warm plum    the terminal's background, on the windows'
                                      side (dark or light): as the windows, or tinted
    terminal_shade  auto deep dark charcoal slate    its lightness: auto, as the windows (a tinted one a
                                      step deeper); or a shade of its own
    text      neutral cream lime amber ice    the terminal's text
    contrast  standard | high         main text at about 7:1 (softer), or 10:1 and more

config.toml keeps the choices ([scheme]); `draftpaper scheme` writes the theme they make to
~/.config/draftpaper/themes/custom.toml, a theme like any other ("Custom").
"""
import math

# ---- choices: (hue in degrees, chroma) ----
TONES = {      # from warm to cold
    "greige": (70, 0.020), "sand": (80, 0.045), "clay": (45, 0.040), "sage": (145, 0.030), "graphite": (0, 0.0),
    "frost": (225, 0.014), "slate": (245, 0.025), "teal": (205, 0.040), "indigo": (275, 0.045), "blueprint": (250, 0.075),
}
ACCENTS = {
    "amber": (70, 0.13), "orange": (50, 0.15), "red": (27, 0.15), "rose": (0, 0.12),
    "violet": (300, 0.12), "blue": (255, 0.13), "teal": (195, 0.10), "green": (145, 0.12),
}
TEXTS = {"neutral": None, "cream": (85, 0.03), "lime": (125, 0.08), "amber": (75, 0.10), "ice": (230, 0.03)}
# the terminal's background: None, as the windows; "tone", the paper's hue; else (hue, chroma)
TERMINALS = {"windows": None, "neutral": (0, 0.0), "tone": "tone", "blue": (255, 0.045), "green": (155, 0.035),
             "warm": (60, 0.03), "plum": (320, 0.035)}
# the lightness (OKLCH L) of each shade, on the dark side and on the light; "dark" is the windows' as
# they were before shades were a choice
SHADES = {"deep": (0.16, 0.995), "dark": (0.22, 0.975), "charcoal": (0.30, 0.92), "slate": (0.38, 0.86)}
# what each shade is called on the light side
LIGHT_NAMES = {"deep": "white", "dark": "paper", "charcoal": "mist", "slate": "grey"}
CHOICES = {
    "paper": ["light", "dark"], "tone": list(TONES), "accent": list(ACCENTS), "windows": ["dark", "light"],
    "shade": list(SHADES), "terminal": list(TERMINALS), "terminal_shade": ["auto"] + list(SHADES),
    "text": list(TEXTS), "contrast": ["standard", "high"],
}
DEFAULT = {"paper": "light", "tone": "greige", "accent": "amber", "windows": "dark", "shade": "dark", "terminal": "windows",
           "terminal_shade": "auto", "text": "cream", "contrast": "standard"}
MAIN = {"standard": 7.0, "high": 10.0}
WORDS, DIM, BORDER = 4.5, 3.0, 3.0


# ---- colour ----
def _lin(x):
    return x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4


def _gam(x):
    return 12.92 * x if x <= 0.0031308 else 1.055 * x ** (1 / 2.4) - 0.055


def rgb(hexa):
    h = hexa.lstrip("#")
    return [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]


def hexa(c):
    return "#" + "".join(f"{round(min(1, max(0, x)) * 255):02X}" for x in c)


def luminance(hx):
    r, g, b = (_lin(x) for x in rgb(hx))
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a, b):
    la, lb = sorted((luminance(a), luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def _oklch_to_rgb(L, C, h):
    a, b = C * math.cos(math.radians(h)), C * math.sin(math.radians(h))
    l_ = L + 0.3963377774 * a + 0.2158037573 * b
    m_ = L - 0.1055613458 * a - 0.0638541728 * b
    s_ = L - 0.0894841775 * a - 1.2914855480 * b
    l, m, s = l_ ** 3, m_ ** 3, s_ ** 3
    return [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
            -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
            -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s]


def to_oklch(hx):
    """(L, C, h) of a colour."""
    r, g, b = (_lin(x) for x in rgb(hx))
    l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b) ** (1 / 3)
    m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b) ** (1 / 3)
    s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b) ** (1 / 3)
    L = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s
    A = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s
    B = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s
    return L, math.hypot(A, B), math.degrees(math.atan2(B, A)) % 360


def mend(hx, against):
    """`hx` with its hue and chroma kept and its lightness moved only as far as `against` asks."""
    L, C, h = to_oklch(hx)
    return hx if all(contrast(hx, bg) >= m for bg, m in against) else solve(h, C, L, against)


def oklch(L, C, h):
    """A colour from OKLCH, its chroma reduced until it fits in sRGB."""
    L = min(1.0, max(0.0, L))
    lo, hi = 0.0, C
    lin = _oklch_to_rgb(L, C, h)
    if all(-1e-4 <= x <= 1 + 1e-4 for x in lin):
        return hexa([_gam(min(1, max(0, x))) for x in lin])
    for _ in range(24):
        mid = (lo + hi) / 2
        if all(-1e-4 <= x <= 1 + 1e-4 for x in _oklch_to_rgb(L, mid, h)):
            lo = mid
        else:
            hi = mid
    return hexa([_gam(min(1, max(0, x))) for x in _oklch_to_rgb(L, lo, h)])


def solve(h, c, prefer, against):
    """The colour of hue `h` and chroma `c` nearest lightness `prefer` that reaches every (background,
    minimum contrast) in `against`, going away from the backgrounds (darker on light ones)."""
    dark = sum(luminance(bg) for bg, _ in against) / len(against) > 0.18
    L = prefer
    while 0 <= L <= 1:
        col = oklch(L, c, h)
        if all(contrast(col, bg) >= m for bg, m in against):
            return col
        L += -0.005 if dark else 0.005
    return "#000000" if dark else "#FFFFFF"


# ---- the scheme ----
def build(choices):
    ch = {**DEFAULT, **{k: v for k, v in (choices or {}).items() if k in CHOICES and v in CHOICES[k]}}
    th, tc = TONES[ch["tone"]]
    ah, ac = ACCENTS[ch["accent"]]
    main = MAIN[ch["contrast"]]
    light_paper, dark_windows = ch["paper"] == "light", ch["windows"] == "dark"

    # the sheet
    if light_paper:
        paper, paper_light, paper_shade = oklch(0.86, tc, th), oklch(0.92, tc * 0.8, th), oklch(0.76, tc * 1.1, th)
        glow = oklch(0.93, 0.05, ah)
        ink_pref, strong_pref = 0.30, 0.12
    else:
        paper, paper_light, paper_shade = oklch(0.31, tc, th), oklch(0.37, tc, th), oklch(0.24, tc, th)
        glow = oklch(0.40, 0.05, ah)
        ink_pref, strong_pref = 0.85, 0.97
    sheet_bgs = [(paper, WORDS), (paper_light, WORDS), (glow, WORDS), (paper_shade, DIM)]
    ink = solve(th, min(tc, 0.04), ink_pref, sheet_bgs)
    strong = solve(ah if not light_paper else th, 0.02 if light_paper else 0.12, strong_pref, [(paper, main), (paper_light, WORDS)])
    alert = solve(27, 0.17, 0.50 if light_paper else 0.72, sheet_bgs)

    # the windows
    nh, nc = th, min(tc, 0.012)
    # the windows' lightness: the shade chosen, the surfaces stepping away from it (lighter on dark
    # windows, darker on light ones)
    wL = SHADES[ch["shade"]][0 if dark_windows else 1]
    if dark_windows:
        bg, bg_alt, surface, surface_alt, border = (oklch(wL + d, nc, nh) for d in (0, 0.025, 0.05, 0.09, 0.20))
        fg_pref, dim_pref, muted_pref = 0.93, 0.80, 0.68
    else:
        bg, bg_alt, surface, surface_alt, border = (oklch(wL - d, nc, nh) for d in (0, 0.025, 0.05, 0.085, 0.215))
        fg_pref, dim_pref, muted_pref = 0.22, 0.38, 0.50
    texts = [(bg, main), (bg_alt, main), (surface, WORDS)]
    # standard: the main text only as far from its background as 7:1 takes it (softer); high: near the
    # extreme, at least 10:1
    soft = ch["contrast"] == "standard"
    bg_L = to_oklch(bg)[0]
    fg = solve(nh, nc, bg_L if soft else fg_pref, texts)
    fg_dim = solve(nh, nc, dim_pref, [(bg, WORDS), (surface, WORDS)])
    fg_muted = solve(nh, nc, muted_pref, [(bg, DIM), (surface, DIM)])
    # the terminal's background: as the windows', or a deeper one of its own, tinted; on the same side
    # its lightness: auto follows the windows (a tinted terminal a step deeper, as before shades were a
    # choice), or a shade of its own
    tb = TERMINALS[ch["terminal"]]
    tbh, tbc = (nh, nc) if tb is None else (th, min(tc, 0.035)) if tb == "tone" else tb
    if ch["terminal_shade"] != "auto":
        tb_L = SHADES[ch["terminal_shade"]][0 if dark_windows else 1]
    elif tb is None:
        tb_L = bg_L
    else:
        tb_L = wL - 0.04 if dark_windows else min(1.0, wL + 0.01)
    if tb is None and tb_L == bg_L:
        term_bg, term_bg_alt = bg, surface
    else:
        term_bg, term_bg_alt = oklch(tb_L, tbc, tbh), oklch(tb_L + (0.05 if dark_windows else -0.04), tbc, tbh)
    th_text = TEXTS[ch["text"]] or (nh, nc)
    term_fg = solve(th_text[0], th_text[1], tb_L if soft else fg_pref, [(term_bg, main), (term_bg_alt, WORDS)])

    # the accent: the focused window's border (against the paper and the window), fills, and ink for words
    focused = solve(ah, ac, 0.72 if dark_windows else 0.58, [(bg, BORDER), (surface, BORDER)])
    accent = focused
    accent_ink = solve(ah, ac, 0.75 if dark_windows else 0.48, [(bg, WORDS), (surface, WORDS)])
    accent_dim = oklch(0.55 if dark_windows else 0.80, ac * 0.8, ah)
    on_accent = bg if contrast(bg, accent) >= WORDS else solve(ah, 0.02, 0.15 if light_or(accent) else 0.97, [(accent, WORDS)])
    sel_bg = accent_dim
    sel_fg = solve(ah, 0.02, 0.12 if light_or(sel_bg) else 0.97, [(sel_bg, WORDS)])
    sh = 195 if ah in (125, 145) or abs(ah - 145) < 30 else 145
    secondary = oklch(0.72 if dark_windows else 0.55, 0.09, sh)
    secondary_dim = oklch(0.58 if dark_windows else 0.70, 0.07, sh)
    secondary_ink = solve(sh, 0.09, 0.75 if dark_windows else 0.45, [(bg, WORDS)])
    states = {"urgent": (25, 0.14), "warn": (55, 0.14), "ok": (145, 0.11), "info": (235, 0.08)}
    fills = {k: oklch(0.70 if dark_windows else 0.58, c, h) for k, (h, c) in states.items()}
    inks = {k + "_ink": solve(h, c, 0.72 if dark_windows else 0.45, [(bg, WORDS), (surface, WORDS)]) for k, (h, c) in states.items()}

    # the terminal's sixteen colours, each readable on the terminal's background
    hues = {"red": 27, "green": 140, "yellow": 90, "blue": 250, "magenta": 330, "cyan": 200}
    ansi = {}
    for name, h in hues.items():
        ansi[name] = solve(h, 0.12, 0.70 if dark_windows else 0.48, [(term_bg, WORDS), (term_bg_alt, WORDS)])
        ansi["br_" + name] = solve(h, 0.10, 0.82 if dark_windows else 0.40, [(term_bg, WORDS), (term_bg_alt, WORDS)])
    gc = min(tbc, 0.012)                                                # the greys, a touch of the background's hue
    if dark_windows:
        ansi["black"] = oklch(tb_L + 0.09, tbc, tbh)                    # a background, not text
        ansi["br_black"] = solve(tbh, gc, 0.62, [(term_bg, WORDS), (term_bg_alt, WORDS)])   # comments, dim text
        ansi["white"] = solve(tbh, gc, 0.86, [(term_bg, WORDS)])
        ansi["br_white"] = solve(tbh, gc, 0.97, [(term_bg, main)])
    else:
        ansi["black"] = solve(tbh, gc, 0.20, [(term_bg, main)])
        ansi["br_black"] = solve(tbh, gc, 0.48, [(term_bg, WORDS), (term_bg_alt, WORDS)])
        ansi["white"] = oklch(tb_L - 0.085, tbc, tbh)                   # backgrounds in a light terminal
        ansi["br_white"] = oklch(tb_L - 0.035, tbc, tbh)
    cursor = accent
    cursor_text = term_bg if contrast(term_bg, cursor) >= WORDS else solve(ah, 0.02, 0.12 if light_or(cursor) else 0.97, [(cursor, WORDS)])

    apps = {
        "variant": "dark" if dark_windows else "light",
        "bg": bg, "bg_alt": bg_alt, "surface": surface, "surface_alt": surface_alt, "border": border,
        "fg": fg, "fg_dim": fg_dim, "fg_muted": fg_muted, "field": bg_alt, "field_fg": fg,
        "term_bg": term_bg, "term_fg": term_fg, "term_bg_alt": term_bg_alt,
        "accent": accent, "accent_dim": accent_dim, "on_accent": on_accent,
        "secondary": secondary, "secondary_dim": secondary_dim,
        "sel_bg": sel_bg, "sel_fg": sel_fg, **fills,
        "accent_ink": accent_ink, "secondary_ink": secondary_ink, **inks,
        "titlebar_bg": surface, "titlebar_fg": accent_ink, "titlebar_inactive_bg": bg, "titlebar_inactive_fg": fg_muted,
        "desktop": paper, "desktop_grid": paper_shade, "cursor": cursor, "cursor_text": cursor_text, **ansi,
    }
    label = f"{ch['tone'].capitalize()} {ch['accent']}"
    return {
        "label": label, "paper": paper, "paper_light": paper_light, "paper_shade": paper_shade, "glow": glow,
        "ink": ink, "alert": alert, "strong": strong,
        "sheet": {"angle": 54, "midpoint": 0.45, "blend": 1.0, "shading": 1.0,
                  "glow_strength": 0.6 if light_paper else 0.5, "glow_size": 1.0, "pattern_strength": 0.5},
        "windows": {"focused": focused, "focused_inactive": surface_alt, "unfocused": border, "urgent": fills["urgent"],
                    "text": fg, "text_inactive": fg_muted, "background": bg},
        "apps": apps, "choices": ch,
    }


def light_or(hx):
    """Whether a colour is light (dark text reads on it)."""
    return luminance(hx) > 0.18


def toml(t):
    """The theme as a file, the choices written at the top."""
    c = t["choices"]
    q = lambda v: f'"{v}"' if isinstance(v, str) else repr(v)
    lines = [f"# Built by the settings tool from [scheme] in config.toml: {', '.join(f'{k} {v}' for k, v in c.items())}.",
             "# Rewritten whenever the choices change; to keep a scheme of your own, copy this file to another name.",
             f'label = "{t["label"]}"', ""]
    lines += [f"{k:<11} = {q(t[k])}" for k in ("paper", "paper_light", "paper_shade", "glow", "ink", "alert", "strong")]
    for section in ("sheet", "windows", "apps"):
        lines += ["", f"[{section}]"] + [f"{k:<20} = {q(v)}" for k, v in t[section].items()]
    return "\n".join(lines) + "\n"


def figures(t):
    """What the settings tool letters under a scheme: the main contrasts, and the least of the words'."""
    a = t.get("apps", {})
    out = {"ink": contrast(t["ink"], t["paper"])}
    if a:
        out["text"] = contrast(a["fg"], a["bg"])
        out["terminal"] = contrast(a["term_fg"], a["term_bg"])
    hard = [r for _, r, _, h in check(t) if h]
    out["below"] = len(hard)
    return {k: round(v, 1) if isinstance(v, float) else v for k, v in out.items()}


# ---- the rules, for any theme ----
def check(t):
    """The ways theme `t` falls below the rules: [(what, ratio, minimum, hard)]. The rules for words
    (4.5:1, dimmed 3:1) are the research's and hard; the border's is advice (WCAG's 3:1 for parts of
    an interface; the research does not cover it)."""
    a = t.get("apps", {})
    tests = [("sheet ink on the paper", t["ink"], t["paper"], WORDS),
             ("sheet ink on the lit paper", t["ink"], t["paper_light"], WORDS),
             ("sheet ink on the shaded paper", t["ink"], t["paper_shade"], DIM),
             ("alert on the paper", t["alert"], t["paper"], WORDS),
             ("focused border on its window", t["windows"]["focused"], t["windows"]["background"], BORDER)]
    if a:
        dark = luminance(a["term_bg"]) <= 0.18         # the terminal's own background decides, not the variant
        tests += [("text", a["fg"], a["bg"], WORDS), ("dimmed text", a["fg_muted"], a["bg"], DIM),
                  ("terminal text", a["term_fg"], a["term_bg"], WORDS), ("selection", a["sel_fg"], a["sel_bg"], WORDS)]
        for k in ("accent_ink", "secondary_ink", "ok_ink", "warn_ink", "urgent_ink", "info_ink"):
            tests.append((k, a[k], a["bg"], WORDS))
        exempt = {"black"} if dark else {"white", "br_white"}
        for p in ("", "br_"):
            for k in ("black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"):
                if p + k not in exempt:
                    tests.append(("terminal " + (p + k).replace("br_", "bright "), a[p + k], a["term_bg"], WORDS))
    out = []
    for what, fg, bg, minimum in tests:
        r = contrast(fg, bg)
        if r < minimum - 0.005:
            out.append((what, r, minimum, "border" not in what))
    return out


if __name__ == "__main__":      # python3 -m draftpaper.scheme THEME.toml ...: the rules, for `make check`
    import sys, tomllib
    failed = False
    for path in sys.argv[1:]:
        with open(path, "rb") as f:
            for what, r, m, hard in check(tomllib.load(f)):
                print(f"{path}: {what} {r:.2f}:1, {'needs' if hard else 'advised'} {m:g}:1")
                failed |= hard
    sys.exit(1 if failed else 0)
