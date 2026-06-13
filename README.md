# Helheim

A Slay-the-Spire-style roguelike deck-builder set on the Norse barrow road,
written in Rust. The rules engine (`helheim_core`) is pure, deterministic,
and fully unit-tested; the presentation is Bevy.

## Run

    cargo run -p helheim                 # release-ish dev build
    cargo run -p helheim --features dev  # fast-rebuild dev loop
    cargo run -p helheim -- --seed 7     # reproducible run

## Controls

- Click a card to play it; click an enemy when a target is needed (Esc cancels)
- `1`–`9`/`0` play cards, Tab/arrows cycle targets, Enter confirms
- `E` or the button ends the turn

## Test

    cargo test --workspace               # core rules + shell unit tests
    cargo clippy --workspace --all-targets -- -D warnings

## Layout

- `crates/helheim_core` — cards, combat engine, enemy AI, run state (no Bevy)
- `crates/helheim` — Bevy shell: screens, animation queue, theme
- `docs/superpowers/specs/` — design specs; `docs/superpowers/plans/` — build plans

Phase 1 ships the 3-fight Barrow Road gauntlet. The roadmap (map, relics,
acts 2–3…) lives in the Phase 1 spec.

Font: Fira Sans (SIL Open Font License).
