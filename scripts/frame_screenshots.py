#!/usr/bin/env python3
"""Frame raw CelestineMD screenshots into square, window-chrome cards for the README.

Style notes (common OSS / product README look):
  - square canvas
  - soft gradient backdrop
  - floating macOS-ish window chrome + traffic lights
  - rounded corners + layered drop shadow
"""

from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parents[1]
RAW = ROOT / "assets" / "screenshots" / "raw"
OUT = ROOT / "assets" / "screenshots"

CANVAS = 1280
PAD = 88  # outer margin around the window
TITLEBAR = 40
RADIUS = 18
CONTENT_RADIUS = 10
SHADOW_BLUR = 42
SHADOW_OFFSET = (0, 22)


def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def mix_rgb(c0: tuple[int, int, int], c1: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    return (
        int(lerp(c0[0], c1[0], t)),
        int(lerp(c0[1], c1[1], t)),
        int(lerp(c0[2], c1[2], t)),
    )


def gradient_backdrop(size: int) -> Image.Image:
    """Deep navy → muted teal diagonal — celestial, not generic purple."""
    # Build a small gradient then upscale (fast + soft).
    n = 64
    top_left = (10, 14, 28)
    bottom_right = (18, 42, 58)
    mid = (22, 28, 48)
    small = Image.new("RGB", (n, n))
    px = small.load()
    for y in range(n):
        for x in range(n):
            t = (x + y) / (2 * (n - 1))
            vy = y / (n - 1)
            c = mix_rgb(top_left, bottom_right, t)
            c = mix_rgb(c, mid, 0.25 * (1.0 - abs(vy - 0.45) * 2))
            cx, cy = (n - 1) / 2, (n - 1) / 2
            d = math.hypot(x - cx, y - cy) / (n * 0.72)
            shade = max(0.0, min(1.0, d - 0.35)) * 0.35
            px[x, y] = (
                max(0, int(c[0] * (1 - shade))),
                max(0, int(c[1] * (1 - shade))),
                max(0, int(c[2] * (1 - shade))),
            )
    img = small.resize((size, size), Image.Resampling.LANCZOS).convert("RGBA")
    # Very light grain so the flat gradient doesn’t look plastic
    noise = Image.effect_noise((size, size), 12).convert("L")
    grain = Image.merge(
        "RGBA",
        (
            noise,
            noise,
            noise,
            Image.new("L", (size, size), 18),
        ),
    )
    return Image.alpha_composite(img, grain)


def rounded_mask(size: tuple[int, int], radius: int) -> Image.Image:
    mask = Image.new("L", size, 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle((0, 0, size[0] - 1, size[1] - 1), radius=radius, fill=255)
    return mask


def drop_shadow(size: tuple[int, int], radius: int) -> Image.Image:
    """Soft layered shadow plate matching the window footprint."""
    w, h = size
    pad = SHADOW_BLUR * 2
    sheet = Image.new("RGBA", (w + pad * 2, h + pad * 2), (0, 0, 0, 0))
    shape = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(shape)
    d.rounded_rectangle((0, 0, w - 1, h - 1), radius=radius, fill=(0, 0, 0, 160))
    sheet.paste(shape, (pad, pad), shape)
    sheet = sheet.filter(ImageFilter.GaussianBlur(SHADOW_BLUR))
    # Second tighter pass for a contact shadow
    tight = Image.new("RGBA", sheet.size, (0, 0, 0, 0))
    tshape = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    td = ImageDraw.Draw(tshape)
    td.rounded_rectangle((0, 0, w - 1, h - 1), radius=radius, fill=(0, 0, 0, 90))
    tight.paste(tshape, (pad, pad + 6), tshape)
    tight = tight.filter(ImageFilter.GaussianBlur(14))
    return Image.alpha_composite(sheet, tight)


def load_font(size: int) -> ImageFont.ImageFont:
    candidates = [
        "/usr/share/fonts/TTF/Inter-Medium.ttf",
        "/usr/share/fonts/TTF/Inter.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
    ]
    for path in candidates:
        if Path(path).is_file():
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def frame_one(
    src: Path,
    dest: Path,
    *,
    title: str = "CelestineMD",
    subtitle: str | None = None,
) -> None:
    shot = Image.open(src).convert("RGBA")

    # Window content area inside the square canvas
    max_win_w = CANVAS - PAD * 2
    max_win_h = CANVAS - PAD * 2
    content_max_h = max_win_h - TITLEBAR - 2  # inner border
    content_max_w = max_win_w - 2

    # Scale screenshot to fit content box, keep aspect
    sw, sh = shot.size
    scale = min(content_max_w / sw, content_max_h / sh)
    nw, nh = max(1, int(sw * scale)), max(1, int(sh * scale))
    shot = shot.resize((nw, nh), Image.Resampling.LANCZOS)

    win_w = nw + 2
    win_h = nh + TITLEBAR + 2

    # Compose window
    window = Image.new("RGBA", (win_w, win_h), (0, 0, 0, 0))
    chrome = ImageDraw.Draw(window)

    # Title bar + body fill (neovoid-adjacent dark chrome)
    body_bg = (26, 27, 38, 255)  # #1a1b26
    bar_bg = (22, 22, 34, 255)
    chrome.rounded_rectangle(
        (0, 0, win_w - 1, win_h - 1),
        radius=RADIUS,
        fill=body_bg,
    )
    # Title bar (clip top rounded region by drawing a rect under the round top)
    chrome.rounded_rectangle((0, 0, win_w - 1, TITLEBAR + RADIUS), radius=RADIUS, fill=bar_bg)
    chrome.rectangle((0, TITLEBAR - 4, win_w, TITLEBAR + 2), fill=bar_bg)

    # Hairline under title bar
    chrome.line((0, TITLEBAR, win_w, TITLEBAR), fill=(55, 60, 80, 220), width=1)

    # Traffic lights
    lights = [
        ((14, TITLEBAR // 2), (255, 95, 86)),
        ((34, TITLEBAR // 2), (255, 189, 46)),
        ((54, TITLEBAR // 2), (39, 201, 63)),
    ]
    for (cx, cy), color in lights:
        chrome.ellipse((cx - 6, cy - 6, cx + 6, cy + 6), fill=color + (255,))
        # tiny highlight
        chrome.ellipse((cx - 3, cy - 4, cx - 0, cy - 1), fill=(255, 255, 255, 70))

    # Window title
    font = load_font(15)
    label = title if not subtitle else f"{title}  ·  {subtitle}"
    bbox = chrome.textbbox((0, 0), label, font=font)
    tw = bbox[2] - bbox[0]
    th = bbox[3] - bbox[1]
    chrome.text(
        ((win_w - tw) / 2, (TITLEBAR - th) / 2 - 1),
        label,
        font=font,
        fill=(154, 164, 192, 255),  # soft muted
    )

    # Screenshot into content area with slight inner radius
    content = Image.new("RGBA", (nw, nh), (0, 0, 0, 0))
    content.paste(shot, (0, 0))
    cmask = rounded_mask((nw, nh), CONTENT_RADIUS)
    # Soft top-only: leave bottom corners matching window — full round is fine
    window.paste(content, (1, TITLEBAR + 1), cmask)

    # Outer window mask for clean edges
    wmask = rounded_mask((win_w, win_h), RADIUS)
    window.putalpha(wmask)

    # Backdrop + shadow + window
    canvas = gradient_backdrop(CANVAS)
    shadow = drop_shadow((win_w, win_h), RADIUS)
    sx = (CANVAS - shadow.size[0]) // 2 + SHADOW_OFFSET[0]
    sy = (CANVAS - shadow.size[1]) // 2 + SHADOW_OFFSET[1]
    canvas.alpha_composite(shadow, (sx, sy))

    wx = (CANVAS - win_w) // 2
    wy = (CANVAS - win_h) // 2
    canvas.alpha_composite(window, (wx, wy))

    # Thin outer rim highlight
    rim = ImageDraw.Draw(canvas)
    rim.rounded_rectangle(
        (wx, wy, wx + win_w - 1, wy + win_h - 1),
        radius=RADIUS,
        outline=(255, 255, 255, 28),
        width=1,
    )

    dest.parent.mkdir(parents=True, exist_ok=True)
    canvas.convert("RGB").save(dest, "PNG", optimize=True)
    print(f"wrote {dest.relative_to(ROOT)} ({CANVAS}×{CANVAS})")


def main() -> None:
    jobs = [
        ("readme-editor.png", "neovoid · editor"),
        ("readme-graph.png", "neovoid · graph"),
        ("readme-quick-open.png", "quick open"),
        ("readme-shortcuts.png", "shortcuts"),
    ]
    for name, subtitle in jobs:
        src = RAW / name
        if not src.is_file():
            # Fall back to current output if raw missing
            src = OUT / name
        if not src.is_file():
            print(f"skip missing {name}")
            continue
        frame_one(src, OUT / name, subtitle=subtitle)


if __name__ == "__main__":
    main()
