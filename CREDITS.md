# Credits

## Map node icons

The five map node icons (`crates/helheim/assets/icons/*.png`) are **original
white silhouettes** — crossed swords (fight), skull (elite), campfire (rest),
treasure chest (treasure), and a crown (boss) — generated procedurally by
[`tools/gen_map_icons.py`](tools/gen_map_icons.py) and tinted per node kind at
runtime. They are our own work; no third-party art or licence is involved.

To restyle them, edit the shape definitions in that script and re-run
`python3 tools/gen_map_icons.py`. They can also be swapped for richer art (e.g.
[game-icons.net](https://game-icons.net), CC BY 3.0) by dropping replacement PNGs
into the icons directory with the same filenames — no code change needed.

## Font

Fira Sans — SIL Open Font License.
