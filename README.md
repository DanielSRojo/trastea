# Trastea

A keyboard-driven guitar trainer for the desktop, written in Rust with [iced](https://iced.rs) 0.14.

Trastea draws a twelve-fret neck in standard tuning and drills two things against it: the
shape of a scale, and the name of a note under your finger. Everything is reachable from
the keyboard — the mouse is optional.

> **Status: early.** The Scale Trainer and the Note Trainer work. The Interval Trainer is a
> menu entry pointing at a placeholder screen, and `music::chords` is a documented stub.

## Screens

**Scale Trainer** — pick a root and one of sixteen scale kinds (the seven diatonic modes,
harmonic and melodic minor, the two pentatonics, blues, voodoo, Spanish gypsy, whole tone,
diminished) and see every degree lit on the neck. `i` toggles the markers between note
names and scale degrees; `r` rerolls a random scale.

**Note Trainer** — a two-directional drill with a streak counter. *Name it* lights a
position and asks which note it is; *Find it* names a note and asks you to click or walk
the cursor to a position carrying it. A right answer lights green and a wrong one red;
the next prompt follows a second later. `d` swaps the direction, `a` widens the pool from
the seven naturals to all twelve pitch classes, `r` skips the current prompt.

## Running

```sh
cargo run          # launch the app
cargo test         # 153 unit tests, no UI harness needed
```

Rust edition 2024. The only dependency is `iced` (with the `canvas` and `smol` features,
the latter for the timer behind the Note Trainer's answer flash); the fonts
under `assets/` are embedded into the binary at compile time.

## Keyboard model

The focus ring is a 2-D grid rather than a flat tab order, so `↑ ↓ ← →` and the vim motions
`k j h l` move the way the layout looks. Rows of unequal width clamp instead of wrapping.

| Key | Action |
|---|---|
| `Tab` / `⇧Tab` | next / previous |
| `↑ ↓ ← →`, `k j h l` | move the focus ring |
| `Enter`, `Space` | activate |
| `Esc`, `⌫` | back |
| `?` | list the keys that work on this screen |
| `1`–`9` | jump straight to a Home menu entry |

Per-screen accelerators (`r`, `i`, `d`, `a`) are declared in one table alongside the label
the `?` overlay shows for them, so a new accelerator documents itself. A key not claimed by
the current screen is inert there.

## Layout

```
src/
  main.rs            window, theme, embedded fonts
  rng.rs             hand-rolled splitmix64 — seedable, so drills are reproducible in tests
  music/             pure theory; no iced imports
    notes.rs         PitchClass, Letter, Accidental, Note, Spelling
    intervals.rs     Interval as a degree number + quality, not just a semitone count
    scales.rs        ScaleKind and Scale::spell — degrees derived, not looked up
    chords.rs        stub; see the module comment for the sketch
  ui/
    mod.rs           App, Screen, Message, the focus grid, every view
    fretboard.rs     the neck as an iced canvas widget: one Layout drives both drawing
                     and hit-testing, held together by a round-trip test
```

The split is load-bearing: `music/` never imports iced, and the UI holds no music theory of
its own. Scale spelling is arithmetic over degree numbers rather than a table of key
signatures, which is what lets a G♯ Locrian come out spelled correctly without a special
case.

## Testing

Tests live in-module next to what they cover. The music modules test spelling and interval
arithmetic directly; the UI tests drive `App::update` with synthesized messages and assert
on state, and the seeded RNG makes prompt sequences reproducible, so the drills are testable
without rendering anything.
