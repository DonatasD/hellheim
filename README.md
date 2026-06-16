# Helheim

A Slay-the-Spire-style roguelike deck-builder set on the Norse barrow road,
written in Rust. The rules engine (`helheim_core`) is pure, deterministic,
and fully unit-tested; the presentation is Bevy.

## Run

    cargo run -p helheim                 # release-ish dev build
    cargo run -p helheim --features dev  # fast-rebuild dev loop
    cargo run -p helheim -- --seed 7     # reproducible run

## Controls

- **Map:** click a highlighted node to travel to it
- **Combat:** click a card to play it; click an enemy when a target is needed (Esc cancels); `1`–`9`/`0` play cards, Tab/arrows cycle targets, Enter confirms; `E` ends the turn
- **Reward/Rest:** click to choose

## Test

    cargo test --workspace               # core rules + shell unit tests
    cargo clippy --workspace --all-targets -- -D warnings

## Layout

- `crates/helheim_core` — cards, combat engine, enemy AI, run state (no Bevy)
- `crates/helheim` — Bevy shell: screens, animation queue, theme
- `docs/superpowers/specs/` — design specs; `docs/superpowers/plans/` — build plans

Act 1 is a Slay-the-Spire-style branching map: climb ~15 floors through
monsters, elites, rests, and treasure to the boss. Phase 2 specs 2–5 (gold &
shops, card upgrades, events, save/continue) build on this foundation.

Font: Fira Sans (SIL Open Font License).
Icons: placeholder art — see [CREDITS.md](CREDITS.md) (to be replaced with game-icons.net, CC BY 3.0).
