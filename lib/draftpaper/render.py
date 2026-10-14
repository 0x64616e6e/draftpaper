"""The sheet itself: one wallpaper per output, drawn at its native size and physical scale.

The paper is a soft gradient from `paper_light` (top left) through `paper` to `paper_shade` (bottom
right) with a few wide blooms of `glow`, then the drafting pattern in the theme's ink: a 5 mm dot grid,
the frame `margin` millimetres in from the edge, a tick across the margin at every zone boundary (zones
of about 50 mm, as shell.qml lays out its cells) and crop marks in the corners. The lettering (zone
letters, readings, the title block) is not drawn here: the live shell letters it.
"""
import math
import numpy as np
from PIL import Image, ImageDraw

SUPERSAMPLE = 2


def rgb(hexcolour):
    h = hexcolour.lstrip("#")
    return np.array([int(h[i:i + 2], 16) for i in (0, 2, 4)], dtype=np.float32)


def zones(length_px, mm):
    """How many zones of about 50 mm fit along a margin (at least two), as the shell counts them."""
    return max(2, round(length_px / (50 * mm)))


def paper(theme, w, h):
    s = theme["sheet"]
    y, x = np.mgrid[0:h, 0:w].astype(np.float32)
    u, v = x / w, y / h
    ang = math.radians(s["angle"])
    ca, sa = math.cos(ang), math.sin(ang)
    t = (ca * u + sa * v) / (ca + sa)
    p = np.clip(0.5 + (t - s["midpoint"]) / s["blend"], 0, 1)
    p = (p * p * (3 - 2 * p))[..., None]
    mid = rgb(theme["paper"])
    light = mid + (rgb(theme["paper_light"]) - mid) * s["shading"]
    shade = mid + (rgb(theme["paper_shade"]) - mid) * s["shading"]
    img = np.where(p < 0.5, light + (mid - light) * (p / 0.5), mid + (shade - mid) * ((p - 0.5) / 0.5))
    glow = rgb(theme["glow"])
    for cx, cy, r, k in ((0.16, 0.20, 0.55, 0.50), (0.80, 0.30, 0.40, 0.26), (0.42, 0.88, 0.48, 0.30)):
        r *= s["glow_size"]
        d2 = ((u - cx) * (w / h)) ** 2 + (v - cy) ** 2
        img += (glow - img) * (min(1.0, k * s["glow_strength"]) * np.exp(-d2 / (2 * (r / 2) ** 2)))[..., None]
    return img


def pattern(w, h, mm, margin):
    """The drafting pattern as an 8-bit alpha mask, drawn at twice the size and averaged down."""
    S = SUPERSAMPLE
    m = Image.new("L", (w * S, h * S), 0)
    d = ImageDraw.Draw(m)
    u = mm * S
    a = lambda x: int(round(255 * x))
    step = int(5 * u)
    for gx in range(0, w * S, step):                                  # 5 mm dots
        for gy in range(h * S - S, -1, -step):
            d.rectangle([gx, gy, gx + S - 1, gy + S - 1], fill=a(0.6))
    mg = margin * u
    d.rectangle([mg, mg, w * S - mg, h * S - mg], outline=a(0.85), width=2 * S)   # the frame
    nx, ny = zones(w * S - 2 * mg, u), zones(h * S - 2 * mg, u)
    zw, zh = (w * S - 2 * mg) / nx, (h * S - 2 * mg) / ny
    for i in range(nx + 1):                                           # zone ticks across the margins
        x = mg + i * zw
        d.line([x, 0, x, mg], fill=a(0.6), width=S)
        d.line([x, h * S - mg, x, h * S], fill=a(0.6), width=S)
    for j in range(ny + 1):
        y = mg + j * zh
        d.line([0, y, mg, y], fill=a(0.6), width=S)
        d.line([w * S - mg, y, w * S, y], fill=a(0.6), width=S)
    arm = min(3 * u, mg * 0.45)
    for cx, cy in ((mg / 2, mg / 2), (w * S - mg / 2, mg / 2), (mg / 2, h * S - mg / 2), (w * S - mg / 2, h * S - mg / 2)):
        d.line([cx - arm, cy, cx + arm, cy], fill=a(0.7), width=S)     # crop marks
        d.line([cx, cy - arm, cx, cy + arm], fill=a(0.7), width=S)
    return np.asarray(m.resize((w, h), Image.BOX), dtype=np.float32) / 255


def sheet(theme, w, h, mm, margin, path):
    img = paper(theme, w, h)
    strength = theme["sheet"]["pattern_strength"]
    if strength > 0:
        img += (rgb(theme["ink"]) - img) * (pattern(w, h, mm, margin) * strength)[..., None]
    img += np.random.default_rng(7).uniform(-0.5, 0.5, (h, w, 1)).astype(np.float32)   # no 8-bit banding
    Image.fromarray(np.clip(img + 0.5, 0, 255).astype(np.uint8), "RGB").save(path, compress_level=1)
