#!/usr/bin/env python3
"""Generate the Act 1 map node icons as white silhouettes (pure stdlib).

Run from anywhere:  python3 tools/gen_map_icons.py

Writes 128x128 RGBA PNGs (white silhouette, transparent background — the game
tints them per node kind via `Sprite.color`) to crates/helheim/assets/icons/,
and a black-on-white contact sheet to target/icon-verify.png for eyeballing.

Each icon is described in a 0..100 design space (center 50,50) as filled
polygons/circles, rendered at SS x supersampling and box-downsampled to FINAL
for smooth (anti-aliased) edges. No third-party deps, no external art — tweak
the shape definitions below and re-run to iterate.
"""
import zlib, struct, math, os

FINAL = 128
SS = 4
S = FINAL * SS
SCALE = S / 100.0


def blank():
    return bytearray(S * S)


def fill_poly(buf, dpts, val):
    pts = [(x * SCALE, y * SCALE) for x, y in dpts]
    ys = [p[1] for p in pts]
    y0 = max(0, int(min(ys)))
    y1 = min(S - 1, int(max(ys)) + 1)
    n = len(pts)
    for y in range(y0, y1 + 1):
        yc = y + 0.5
        xs = []
        for i in range(n):
            ax, ay = pts[i]
            bx, by = pts[(i + 1) % n]
            if (ay <= yc < by) or (by <= yc < ay):
                xs.append(ax + (yc - ay) / (by - ay) * (bx - ax))
        xs.sort()
        for k in range(0, len(xs) - 1, 2):
            xa = max(0, int(round(xs[k])))
            xb = min(S, int(round(xs[k + 1])))
            row = y * S
            for x in range(xa, xb):
                buf[row + x] = val


def fill_circle(buf, dcx, dcy, dr, val):
    cx, cy, r = dcx * SCALE, dcy * SCALE, dr * SCALE
    r2 = r * r
    x0 = max(0, int(cx - r))
    x1 = min(S - 1, int(cx + r))
    y0 = max(0, int(cy - r))
    y1 = min(S - 1, int(cy + r))
    for y in range(y0, y1 + 1):
        dy = y + 0.5 - cy
        row = y * S
        for x in range(x0, x1 + 1):
            dx = x + 0.5 - cx
            if dx * dx + dy * dy <= r2:
                buf[row + x] = val


def rect(x0, y0, x1, y1):
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


def rot(dpts, cx, cy, deg):
    a = math.radians(deg)
    ca, sa = math.cos(a), math.sin(a)
    return [((x - cx) * ca - (y - cy) * sa + cx, (x - cx) * sa + (y - cy) * ca + cy) for x, y in dpts]


# ---- icon shape definitions ----

def icon_fight():
    """Crossed swords."""
    b = blank()
    blade = [(50, 8), (54, 20), (54, 60), (46, 60), (46, 20)]
    guard = rect(36, 60, 64, 67)
    grip = rect(47, 67, 53, 80)
    for deg in (45, -45):
        for poly in (blade, guard, grip):
            fill_poly(b, rot(poly, 50, 50, deg), 1)
        # pommel (rot() returns design-space coords; fill_circle scales them)
        px, py = rot([(50, 84)], 50, 50, deg)[0]
        fill_circle(b, px, py, 5, 1)
    return b


def icon_elite():
    """Skull."""
    b = blank()
    fill_circle(b, 50, 41, 27, 1)                       # cranium
    fill_poly(b, [(34, 56), (66, 56), (62, 74), (56, 82), (44, 82), (38, 74)], 1)  # jaw
    fill_circle(b, 39, 44, 8.5, 0)                      # left eye socket
    fill_circle(b, 61, 44, 8.5, 0)                      # right eye socket
    fill_poly(b, [(50, 50), (45, 60), (55, 60)], 0)     # nasal cavity
    for tx in (43, 50, 57):                             # teeth gaps
        fill_poly(b, rect(tx - 1.3, 64, tx + 1.3, 80), 0)
    return b


def icon_rest():
    """Campfire: flame above crossed logs."""
    b = blank()
    fill_poly(b, rot(rect(20, 72, 80, 80), 50, 76, 16), 1)
    fill_poly(b, rot(rect(20, 72, 80, 80), 50, 76, -16), 1)
    flame = [(50, 10), (60, 30), (57, 46), (66, 42), (62, 60), (50, 70),
             (38, 60), (34, 42), (43, 46), (40, 30)]
    fill_poly(b, flame, 1)
    return b


def icon_treasure():
    """Treasure chest with lock + keyhole."""
    b = blank()
    fill_poly(b, [(20, 44), (24, 34), (32, 28), (50, 25), (68, 28), (76, 34), (80, 44)], 1)  # domed lid
    fill_poly(b, rect(20, 44, 80, 78), 1)               # body
    fill_poly(b, rect(20, 45, 80, 49), 0)               # lid seam
    fill_poly(b, rect(46, 49, 54, 64), 1)               # lock plate (over seam)
    fill_circle(b, 50, 56, 2.4, 0)                      # keyhole
    fill_poly(b, rect(48.8, 57, 51.2, 62), 0)           # keyhole stem
    return b


def icon_boss():
    """Three-peak crown with jewels."""
    b = blank()
    crown = [(24, 72), (24, 48), (32, 58), (38, 33), (44, 54),
             (50, 26), (56, 54), (62, 33), (68, 58), (76, 48), (76, 72)]
    fill_poly(b, crown, 1)
    fill_poly(b, rect(22, 64, 78, 74), 1)               # base band
    for jx in (33, 50, 67):                             # jewels
        fill_circle(b, jx, 70, 2.6, 0)
    return b


# ---- per-card icons (tinted by kind at runtime) ----

def card_hew():
    """Hew — a single upright blade."""
    b = blank()
    fill_poly(b, [(50, 10), (56, 22), (56, 60), (44, 60), (44, 22)], 1)  # blade
    fill_poly(b, rect(34, 60, 66, 67), 1)                                # guard
    fill_poly(b, rect(46, 67, 54, 84), 1)                                # grip
    fill_circle(b, 50, 86, 5, 1)                                         # pommel
    return b


def card_raise_shield():
    """Raise Shield — a kite shield."""
    b = blank()
    fill_poly(b, [(50, 11), (80, 21), (80, 45), (69, 71), (50, 87),
                  (31, 71), (20, 45), (20, 21)], 1)
    return b


def card_skull_splitter():
    """Skull-Splitter — a cracked skull."""
    b = blank()
    fill_circle(b, 50, 42, 26, 1)                                        # cranium
    fill_poly(b, [(35, 56), (65, 56), (61, 73), (55, 80), (45, 80), (39, 73)], 1)  # jaw
    fill_circle(b, 40, 45, 8, 0)                                         # left eye
    fill_circle(b, 60, 45, 8, 0)                                         # right eye
    fill_poly(b, [(50, 50), (46, 59), (54, 59)], 0)                      # nose
    fill_poly(b, [(46, 14), (54, 14), (59, 30), (50, 39), (58, 51),      # bold jagged crack
                  (49, 59), (42, 48), (51, 38), (41, 27)], 0)
    return b


def card_whirling_axe():
    """Whirling Axe — a bearded axe with a motion arc."""
    b = blank()
    fill_poly(b, rect(46, 24, 52, 86), 1)                                # handle
    fill_poly(b, [(52, 20), (70, 17), (82, 28), (80, 42), (66, 50),      # bearded blade (right)
                  (52, 50), (52, 40), (62, 38), (62, 32), (52, 30)], 1)
    fill_poly(b, rect(24, 30, 35, 35), 1)                                # motion ticks (left)
    fill_poly(b, rect(20, 46, 32, 51), 1)
    fill_poly(b, rect(24, 62, 35, 67), 1)
    return b


def card_haft_strike():
    """Haft Strike — a war-hammer."""
    b = blank()
    fill_poly(b, rect(30, 12, 70, 34), 1)                                # head
    fill_poly(b, rect(46, 34, 54, 88), 1)                                # haft
    return b


def card_shield_charge():
    """Shield Charge — a shield driving forward."""
    b = blank()
    fill_poly(b, [(58, 14), (86, 23), (86, 45), (76, 68), (58, 82),      # shield (shifted right)
                  (40, 68), (30, 45), (30, 23)], 1)
    fill_poly(b, rect(6, 30, 26, 35), 1)                                 # motion lines
    fill_poly(b, rect(2, 46, 24, 51), 1)
    fill_poly(b, rect(6, 62, 26, 67), 1)
    return b


def card_twin_axes():
    """Twin Axes — a pair of bearded axes facing outward."""
    b = blank()
    fill_poly(b, rect(38, 22, 43, 86), 1)                                # left handle
    fill_poly(b, [(43, 26), (25, 23), (13, 34), (15, 48), (29, 56),      # left blade
                  (43, 56), (43, 46), (33, 44), (33, 38), (43, 36)], 1)
    fill_poly(b, rect(57, 22, 62, 86), 1)                                # right handle
    fill_poly(b, [(62, 26), (80, 23), (92, 34), (90, 48), (76, 56),      # right blade
                  (62, 56), (62, 46), (72, 44), (72, 38), (62, 36)], 1)
    return b


def card_rising_fury():
    """Rising Fury — three rising flames."""
    b = blank()

    def flame(cx, base, top, w):
        mid = (top + base) / 2.0
        return [(cx, top), (cx + w, mid), (cx + w * 0.55, base - 5),
                (cx, base), (cx - w * 0.55, base - 5), (cx - w, mid)]

    fill_poly(b, flame(50, 88, 10, 17), 1)                               # tall central
    fill_poly(b, flame(29, 88, 42, 12), 1)                               # left
    fill_poly(b, flame(71, 88, 42, 12), 1)                               # right
    return b


def card_thors_wrath():
    """Thor's Wrath — a lightning bolt."""
    b = blank()
    fill_poly(b, [(56, 8), (40, 48), (51, 48), (42, 92),
                  (72, 42), (58, 42), (68, 8)], 1)
    return b


def card_unbowed():
    """Unbowed — a standing banner."""
    b = blank()
    fill_poly(b, rect(47, 10, 51, 88), 1)                                # pole
    fill_poly(b, [(51, 14), (84, 14), (75, 27), (84, 40), (51, 40)], 1)  # swallowtail pennant
    return b


def card_surge_of_rage():
    """Surge of Rage — upward surge chevrons."""
    b = blank()

    def chevron(cy):
        return [(26, cy), (50, cy - 24), (74, cy), (65, cy), (50, cy - 13), (35, cy)]

    fill_poly(b, chevron(56), 1)
    fill_poly(b, chevron(84), 1)
    return b


def card_berserkergang():
    """Berserkergang — the Algiz rune (a stave with two raised arms)."""
    b = blank()
    fill_poly(b, rect(47, 18, 53, 88), 1)                                # stave
    fill_poly(b, [(48, 50), (53, 46), (30, 14), (25, 18)], 1)            # left arm up
    fill_poly(b, [(52, 50), (47, 46), (70, 14), (75, 18)], 1)            # right arm up
    return b


# ---- PNG output ----

def write_rgba(path, buf):
    raw = bytearray()
    for fy in range(FINAL):
        raw.append(0)
        for fx in range(FINAL):
            s = 0
            for j in range(SS):
                base = (fy * SS + j) * S + fx * SS
                s += sum(buf[base:base + SS])
            a = (s * 255) // (SS * SS)
            raw += bytes((255, 255, 255, a))
    _write(path, FINAL, FINAL, 6, raw)


def _downsample(buf):
    out = bytearray(FINAL * FINAL)
    for fy in range(FINAL):
        for fx in range(FINAL):
            s = 0
            for j in range(SS):
                base = (fy * SS + j) * S + fx * SS
                s += sum(buf[base:base + SS])
            out[fy * FINAL + fx] = (s * 255) // (SS * SS)
    return out


def write_contact_sheet(path, bufs):
    w, h = FINAL * len(bufs), FINAL
    sheet = bytearray([255]) * (w * h * 3)
    for i, buf in enumerate(bufs):
        a = _downsample(buf)
        for y in range(FINAL):
            for x in range(FINAL):
                v = 255 - a[y * FINAL + x]
                o = (y * w + (i * FINAL + x)) * 3
                sheet[o] = sheet[o + 1] = sheet[o + 2] = v
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        raw += sheet[y * w * 3:(y + 1) * w * 3]
    _write(path, w, h, 2, raw)


def _write(path, w, h, color_type, raw):
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, color_type, 0, 0, 0)))
        f.write(chunk(b"IDAT", zlib.compress(bytes(raw), 9)))
        f.write(chunk(b"IEND", b""))


def main():
    repo = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    icons_dir = os.path.join(repo, "crates", "helheim", "assets", "icons")
    os.makedirs(icons_dir, exist_ok=True)
    order = [("fight", icon_fight), ("elite", icon_elite), ("rest", icon_rest),
             ("treasure", icon_treasure), ("boss", icon_boss)]
    bufs = []
    for name, fn in order:
        buf = fn()
        bufs.append(buf)
        write_rgba(os.path.join(icons_dir, name + ".png"), buf)
        print("wrote", name + ".png")
    card_order = [
        ("card_hew", card_hew),
        ("card_raise_shield", card_raise_shield),
        ("card_skull_splitter", card_skull_splitter),
        ("card_whirling_axe", card_whirling_axe),
        ("card_haft_strike", card_haft_strike),
        ("card_shield_charge", card_shield_charge),
        ("card_twin_axes", card_twin_axes),
        ("card_rising_fury", card_rising_fury),
        ("card_thors_wrath", card_thors_wrath),
        ("card_unbowed", card_unbowed),
        ("card_surge_of_rage", card_surge_of_rage),
        ("card_berserkergang", card_berserkergang),
    ]
    for name, fn in card_order:
        buf = fn()
        bufs.append(buf)
        write_rgba(os.path.join(icons_dir, name + ".png"), buf)
        print("wrote", name + ".png")
    target = os.path.join(repo, "target")
    if os.path.isdir(target):
        write_contact_sheet(os.path.join(target, "icon-verify.png"), bufs)
        print("verify sheet: target/icon-verify.png")


if __name__ == "__main__":
    main()
