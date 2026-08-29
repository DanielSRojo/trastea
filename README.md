# Trastea

A keyboard-driven guitar trainer for the desktop, written in Rust with [iced](https://iced.rs) 0.14.

Trastea draws a twelve-fret neck in standard tuning and drills three things against it: the
shape of a scale, the name of a note under your finger, and the distance between two
positions. Everything is reachable from the keyboard — the mouse is optional.

> **Status: early.** All three trainers work. `music::chords` is a documented stub, and the
> chord layer it sketches is the next thing to build.

## Screens

**Scale Trainer** — pick a root and one of sixteen scale kinds (the seven diatonic modes,
harmonic and melodic minor, the two pentatonics, blues, voodoo, Spanish gypsy, whole tone,
diminished) and see every degree lit on the neck. The scale names itself the way it would
be taught — pitch class 10 under Ionian is B♭ major, not the A♯ major that would put three
double sharps on the fretboard — so there is nothing to choose and no ♯/♭ setting to get
wrong. `i` toggles the markers between note names and scale degrees; `r` rerolls a random
scale.

**Note Trainer** — a two-directional drill with a streak counter. *Name it* lights a
position and asks which note it is; *Find it* names a note and asks you to click or walk
the cursor to a position carrying it. A right answer lights green and a wrong one red;
the next prompt follows a second later. `d` swaps the direction, `a` widens the pool from
the seven naturals to all twelve pitch classes, `r` skips the current prompt.

**Interval Trainer** — the same two directions, with nothing on screen named. A lit position
stands in for the tonal center instead of a key, and no note name appears anywhere, so a
prompt is answerable by measuring the distance between two marks and by nothing else. *Name
it* lights a root and a target and asks what interval separates them; *Find it* lights a root,
names an interval, and asks you to click or walk to a position carrying it. Twelve buttons
cover the octave rather than thirteen: with no key established, the augmented fourth and the
diminished fifth are the same six frets, so the drill judges by semitone distance instead of
pretending to tell them apart. `d` swaps the direction, `r` skips the current prompt.

## Running

```sh
cargo run          # launch the app
cargo test         # the unit tests, no UI harness needed
```

Rust edition 2024. The only dependency is `iced` (with the `canvas` and `smol` features,
the latter for the timer behind the trainers' answer flash); the fonts under `assets/` are
embedded into the binary at compile time.

Tagged releases carry a prebuilt binary for x86_64 Linux and an `.app` bundle for Apple
silicon. Every other platform builds from source with the above.

Contributors can install [`just`](https://just.systems) and run `just` to see the repo's
recipes; `just ci` runs the same gate CI runs, and nothing else here needs it.

## Keyboard model

The focus ring is a 2-D grid rather than a flat tab order, so `↑ ↓ ← →` and the vim motions
`k j h l` move the way the layout looks. Rows of unequal width clamp instead of wrapping.

| Key | Action |
| --- | --- |
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
  main.rs                window, theme, embedded fonts
  rng.rs                 hand-rolled splitmix64 — seedable, so drills are reproducible
  music/                 pure theory; no iced imports
    notes.rs             PitchClass, Letter, Accidental, Note, Spelling
    intervals.rs         Interval as a degree number + quality, not just a semitone count
    scales.rs            ScaleKind and Scale::spell — degrees derived, not looked up
    chords.rs            stub; see the module comment for the sketch
  ui/
    mod.rs               App, Screen, Message, the focus grid, the neck's geometry, the
                         Home and Scale Trainer views
    note_trainer.rs      the Note Trainer's state machine and the screen that draws it
    interval_trainer.rs  the Interval Trainer's, on the same terms
    fretboard.rs         the neck as an iced canvas widget: one Layout drives both drawing
                         and hit-testing, held together by a round-trip test
```

The split is load-bearing: `music/` never imports iced, and the UI holds no music theory of
its own. Scale spelling is arithmetic over degree numbers rather than a table of key
signatures, which is what lets a G♯ Locrian come out spelled correctly without a special
case. Which of two enharmonic root names a scale gets is arithmetic too — fewest double
accidentals, then fewest accidentals — and only the eleven scales where that comes out
exactly level fall back to what the note is conventionally called.

`note_trainer.rs` and `interval_trainer.rs` each keep their state machine and their views
together so the state can keep its fields private — they are read all over those views and
nowhere else. `App` drives both drills through the methods they mark `pub(super)` and never
reaches past them; what the neck *is* stays in `mod.rs`, since that is the instrument rather
than the drill, and both trainers want it on the same terms.

## Testing

Tests live in-module next to what they cover. The music modules test spelling and interval
arithmetic directly; the UI tests drive `App::update` with synthesized messages and assert
on state, and the seeded RNG makes prompt sequences reproducible, so the drills are testable
without rendering anything.

## License

Trastea is free software under the [GNU General Public License, version 3 or later](LICENSE).
You may run, study, share and modify it; a version you pass on has to reach its users under
those same terms, source included.

The fonts are a separate matter. They are compiled into the binary but licensed under the
[SIL Open Font License 1.1](assets/fonts/OFL.txt) rather than the GPL — Leland and Leland
Text © MuseScore BVBA, Dancing Script © the Dancing Script Project Authors, both embedded
unmodified. The OFL says plainly that a font may be bundled with software under any license,
so the two sit beside each other without either reaching into the other. Both texts ship
inside the release archives, since the binary redistributes the fonts by carrying them.
