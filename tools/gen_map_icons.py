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


def card_rending_blow():
    """Rending Blow — three bold parallel diagonal slash gashes (steep /)."""
    b = blank()
    # Three thick slash stripes running steeply from lower-left to upper-right
    # Each stripe: wide parallelogram leaning heavily to the right
    offsets = [-18, 0, 18]
    for dx in offsets:
        cx = 50 + dx
        # Steep diagonal: top-right to bottom-left  (like a '/' slash)
        gash = [(cx + 16, 10), (cx + 24, 10), (cx - 16, 90), (cx - 24, 90)]
        fill_poly(b, gash, 1)
    return b


def card_bulwark_bash():
    """Bulwark Bash — a broad heater shield with radiating impact shards."""
    b = blank()
    # Heater shield (wider, more rectangular top than kite)
    fill_poly(b, [(22, 18), (78, 18), (78, 52), (50, 85), (22, 52)], 1)
    # Impact shards radiating outward (top-left, top-right, left, right, top corners)
    shards = [
        [(8, 8), (16, 14), (10, 20)],       # top-left shard
        [(92, 8), (84, 14), (90, 20)],      # top-right shard
        [(5, 38), (14, 35), (14, 43)],      # left shard
        [(95, 38), (86, 35), (86, 43)],     # right shard
        [(18, 5), (22, 14), (28, 10)],      # upper-left shard
        [(82, 5), (78, 14), (72, 10)],      # upper-right shard
    ]
    for s in shards:
        fill_poly(b, s, 1)
    return b


def card_sundering_axe():
    """Sundering Axe — a great double-bitted axe (bits both top and bottom)."""
    b = blank()
    # Central haft
    fill_poly(b, rect(47, 10, 53, 90), 1)
    # Upper axe-head (bit points left and right at the top)
    fill_poly(b, [(47, 22), (20, 14), (16, 26), (34, 34), (47, 34)], 1)  # upper-left blade
    fill_poly(b, [(53, 22), (80, 14), (84, 26), (66, 34), (53, 34)], 1)  # upper-right blade
    # Lower axe-head (mirrored at the bottom)
    fill_poly(b, [(47, 68), (20, 76), (16, 64), (34, 56), (47, 56)], 1)  # lower-left blade
    fill_poly(b, [(53, 68), (80, 76), (84, 64), (66, 56), (53, 56)], 1)  # lower-right blade
    return b


def card_reaver():
    """Reaver — a scythe: long diagonal snath with a hooked blade at top."""
    b = blank()
    # Long diagonal snath (handle) from bottom-left to upper-right
    snath = [(24, 88), (30, 88), (76, 18), (70, 18)]
    fill_poly(b, snath, 1)
    # Hooked blade: large curved crescent at top sweeping right
    fill_poly(b, [(58, 12), (88, 18), (90, 34), (72, 46), (58, 44),
                  (62, 38), (76, 34), (76, 24), (62, 20)], 1)
    # Blade inner edge cut for crescent sharpness
    fill_poly(b, [(64, 22), (76, 26), (74, 36), (66, 40), (64, 36),
                  (70, 32), (70, 28), (64, 26)], 0)
    return b


def card_wrathful_cut():
    """Wrathful Cut — an upright blade with a jagged rage spark at the tip."""
    b = blank()
    # Blade
    fill_poly(b, [(50, 14), (56, 28), (55, 64), (45, 64), (44, 28)], 1)
    fill_poly(b, rect(36, 64, 64, 71), 1)                                # guard
    fill_poly(b, rect(47, 71, 53, 84), 1)                                # grip
    fill_circle(b, 50, 87, 4, 1)                                         # pommel
    # Rage spark — starburst of triangular spikes emanating from blade tip
    for deg in (0, 60, 120, 180, 240, 300):
        tip = rot([(50, 8)], 50, 14, deg)[0]
        base_l = rot([(47, 14)], 50, 14, deg)[0]
        base_r = rot([(53, 14)], 50, 14, deg)[0]
        fill_poly(b, [tip, base_l, base_r], 1)
    return b


def card_war_frenzy():
    """War Frenzy — five cards fanned in a spread hand."""
    b = blank()
    # Five wider cards fanned in an arc, pivot at bottom-center
    angles = [-36, -18, 0, 18, 36]
    # Draw all filled cards first
    for ang in angles:
        card_pts = rect(44, 14, 56, 76)   # wider cards
        fill_poly(b, rot(card_pts, 50, 88, ang), 1)
    # Cut gaps between cards: draw thin dark separator lines as cuts
    for ang in angles:
        gap_l = rect(44, 14, 45.5, 76)
        fill_poly(b, rot(gap_l, 50, 88, ang), 0)
        gap_r = rect(54.5, 14, 56, 76)
        fill_poly(b, rot(gap_r, 50, 88, ang), 0)
    # Redraw card outlines so each card has a visible solid border
    for ang in angles:
        card_pts = rect(44, 14, 56, 76)
        fill_poly(b, rot(card_pts, 50, 88, ang), 1)
        inner = rect(46, 17, 54, 74)
        fill_poly(b, rot(inner, 50, 88, ang), 0)
    return b


def card_sunder():
    """Sunder — a broken sword: two pieces with a jagged break in the middle."""
    b = blank()
    # Upper blade fragment (angled slightly left)
    upper = [(48, 12), (53, 12), (54, 46), (47, 46)]
    fill_poly(b, rot(upper, 50, 46, -6), 1)
    # Lower grip fragment (angled slightly right)
    lower = [(47, 54), (54, 54), (54, 72), (47, 72)]
    fill_poly(b, rot(lower, 50, 54, 6), 1)
    # Guard on lower fragment
    fill_poly(b, rot(rect(36, 52, 64, 59), 50, 54, 6), 1)
    # Grip continuation
    fill_poly(b, rot(rect(47, 59, 53, 82), 50, 54, 6), 1)
    fill_circle(b, 50, 85, 4, 1)                                         # pommel
    # Jagged break teeth on upper fragment (pointing down)
    jagged_up = [(46, 46), (50, 42), (54, 46), (52, 50), (48, 50)]
    fill_poly(b, rot(jagged_up, 50, 46, -6), 1)
    return b


def card_dread_roar():
    """Dread Roar — an open roaring maw with bold sound-wave arcs on the sides."""
    b = blank()
    # Open mouth: large circle
    fill_circle(b, 50, 54, 26, 1)
    # Inner void (throat/darkness)
    fill_circle(b, 50, 57, 18, 0)
    # Teeth top row (pointing down into the void)
    for tx in (36, 43, 50, 57):
        fill_poly(b, [(tx, 40), (tx + 5, 40), (tx + 3, 50), (tx - 2, 50)], 1)
    # Teeth bottom row (pointing up into the void)
    for tx in (38, 45, 52, 59):
        fill_poly(b, [(tx, 68), (tx + 5, 68), (tx + 3, 60), (tx - 2, 60)], 1)
    # Bold sound-wave arcs flanking left and right of the maw
    # Left arcs (two concentric, opening leftward)
    fill_poly(b, [(8, 38), (16, 30), (20, 38), (16, 46), (8, 46)], 1)   # outer left arc
    fill_poly(b, [(10, 38), (16, 33), (18, 38), (16, 43), (10, 43)], 0)  # cut inner left
    fill_poly(b, [(2, 38), (10, 26), (14, 38), (10, 50), (2, 50)], 1)    # far left arc
    fill_poly(b, [(4, 38), (10, 29), (12, 38), (10, 47), (4, 47)], 0)    # cut far-left inner
    # Right arcs (mirror)
    fill_poly(b, [(92, 38), (84, 30), (80, 38), (84, 46), (92, 46)], 1)  # outer right arc
    fill_poly(b, [(90, 38), (84, 33), (82, 38), (84, 43), (90, 43)], 0)  # cut inner right
    fill_poly(b, [(98, 38), (90, 26), (86, 38), (90, 50), (98, 50)], 1)  # far right arc
    fill_poly(b, [(96, 38), (90, 29), (88, 38), (90, 47), (96, 47)], 0)  # cut far-right inner
    return b


def card_blood_offering():
    """Blood Offering — a chalice goblet with a blood drop falling into it."""
    b = blank()
    # Chalice cup (trapezoid wider at top)
    fill_poly(b, [(30, 22), (70, 22), (64, 56), (36, 56)], 1)
    # Stem
    fill_poly(b, rect(45, 56, 55, 74), 1)
    # Base plate
    fill_poly(b, rect(32, 74, 68, 82), 1)
    # Cut interior of cup
    fill_poly(b, [(33, 26), (67, 26), (62, 52), (38, 52)], 0)
    # Blood drop falling from above into chalice
    fill_circle(b, 50, 10, 5, 1)                                         # round top of drop
    fill_poly(b, [(45, 10), (55, 10), (50, 20)], 1)                      # pointed drip tip
    return b


def card_iron_will():
    """Iron Will — a solid tower shield (tall, flat-topped, rectangular with rounded base)."""
    b = blank()
    # Tower shield: tall rectangle with rounded bottom point
    fill_poly(b, [(28, 12), (72, 12), (72, 68), (50, 88), (28, 68)], 1)
    # Horizontal reinforcing bar across middle
    fill_poly(b, rect(28, 46, 72, 54), 0)                                # cut horizontal band
    fill_poly(b, rect(28, 48, 72, 52), 1)                                # re-paint thin bar
    # Vertical center strip
    fill_poly(b, rect(47, 12, 53, 88), 0)                                # cut vertical
    fill_poly(b, rect(48.5, 12, 51.5, 88), 1)                            # re-paint thin line
    return b


def card_fury_of_the_bear():
    """Fury of the Bear — a stylized bear head silhouette."""
    b = blank()
    # Main head mass
    fill_circle(b, 50, 52, 30, 1)
    # Snout protruding forward (wider lower face)
    fill_poly(b, [(36, 58), (64, 58), (68, 72), (32, 72)], 1)
    # Ears
    fill_circle(b, 28, 28, 11, 1)
    fill_circle(b, 72, 28, 11, 1)
    fill_circle(b, 28, 28, 6, 0)   # ear interior left
    fill_circle(b, 72, 28, 6, 0)   # ear interior right
    # Eyes
    fill_circle(b, 39, 46, 5, 0)
    fill_circle(b, 61, 46, 5, 0)
    # Nostrils
    fill_circle(b, 44, 66, 3, 0)
    fill_circle(b, 56, 66, 3, 0)
    # Brow ridge (heavy angry brow)
    fill_poly(b, [(22, 38), (40, 34), (44, 40), (24, 44)], 1)
    fill_poly(b, [(78, 38), (60, 34), (56, 40), (76, 44)], 1)
    return b


def card_iron_hide():
    """Iron Hide — stacked horizontal armor plates (lamellar lames)."""
    b = blank()
    # Four stacked horizontal armor lames, each with slight taper
    lame_defs = [
        (18, 14, 82, 30),   # top lame
        (16, 34, 84, 50),   # second lame
        (18, 54, 82, 70),   # third lame
        (20, 74, 80, 88),   # bottom lame
    ]
    for (x0, y0, x1, y1) in lame_defs:
        # Lame shape: slight convex top edge
        mid_y = y0 - 3
        lame = [(x0, y0), (x0, y1), (x1, y1), (x1, y0),
                ((x0 + x1) / 2, mid_y)]
        fill_poly(b, lame, 1)
        # Cut interior to give plate thickness look (keep border)
        inner = rect(x0 + 4, y0 + 3, x1 - 4, y1 - 3)
        fill_poly(b, inner, 0)
        # Rivet dots at ends of each lame
        fill_circle(b, x0 + 7, (y0 + y1) / 2, 2.5, 1)
        fill_circle(b, x1 - 7, (y0 + y1) / 2, 2.5, 1)
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
        ("card_rending_blow", card_rending_blow),
        ("card_bulwark_bash", card_bulwark_bash),
        ("card_sundering_axe", card_sundering_axe),
        ("card_reaver", card_reaver),
        ("card_wrathful_cut", card_wrathful_cut),
        ("card_war_frenzy", card_war_frenzy),
        ("card_sunder", card_sunder),
        ("card_dread_roar", card_dread_roar),
        ("card_blood_offering", card_blood_offering),
        ("card_iron_will", card_iron_will),
        ("card_fury_of_the_bear", card_fury_of_the_bear),
        ("card_iron_hide", card_iron_hide),
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
