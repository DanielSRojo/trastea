# Trastea

A keyboard-driven guitar trainer for the desktop, written in Rust with [iced](https://iced.rs) 0.14.

Trastea draws a twelve-fret neck in standard tuning and drills three things against it: the
shape of a scale, the name of a note under your finger, and the distance between two
positions. A fourth screen answers rather than asks — a chord library you look things up in.
Everything is reachable from the keyboard — the mouse is optional.

> **Status: early.** The three trainers and the chord library work. The library's shape
> table is the part still worth growing: five CAGED shapes and a handful of reduced ones
> cover the fifteen qualities, and every chord it offers is one a hand can hold.

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

**Chord Library** — a reference rather than a drill. The list is every chord it knows, twelve
roots by fifteen qualities, grouped under the root they are built on; typing narrows it.
`f♯` keeps that root, `m7♭5` keeps that quality on every root, `f♯m7♭5` keeps the one chord.
A typed symbol is parsed rather than fuzzy-matched, so `cm7` is a minor seventh and never the
major seventh it happens to be a subsequence of; approximate matching is the fallback for
input the grammar cannot read, where `cmj7` still finds `Cmaj7`.

Picking a chord shows its notes, its degrees, and the ways to play it, drawn as chord
diagrams — a four-fret window with a position label, muted and open strings marked above the
nut, and a barre drawn as one bar rather than several dots. The picked shape is drawn half
again as large as the rest. `notes`, `degrees` and `fingers` choose what the dots say, and it
opens on fingers, which is what answers "how do I play this".

The shapes are derived, not tabulated. A small set of movable ones says which strings sound
and which degree each carries; placing one is addition, and changing a chord's quality moves
only the strings whose degrees that quality alters — so an open E and a barred B♭m7 come out
of the same entry. A placement that would fall below the nut, outrun a hand, run off the neck
or need a fifth finger is refused rather than shown.

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

Per-screen accelerators are declared in one table alongside the label the `?` overlay shows
for them, so a new accelerator documents itself. A key not claimed by the current screen is
inert there — and one claimed by two screens is free to mean different things on each: `d`
swaps a trainer's direction and picks the library's degrees.

The Chord Library adds the one modal thing in the app: a search box, opened with `/` or
`Ctrl+K` and focused already when the screen opens, since looking something up starts with
typing. While it holds the keyboard almost everything is text — `j` types a `j`, `?` types a
`?` — because `n`, `d`, `f` and `g` are all note names and a search has to be able to begin
with one. `↑ ↓` still pick a chord while typing, so type-then-choose is one gesture; `Enter`
finishes and hands the keyboard to the shapes, where `← →` walk them. `Esc` leaves the box
without clearing it, and `gg` and `G` reach the ends of the list.

`/` and `?` are the same physical key, which is the whole arrangement: search, and shift for
help.

## Layout

```
src/
  main.rs                window, theme, embedded fonts
  rng.rs                 hand-rolled splitmix64 — seedable, so drills are reproducible
  music/                 pure theory; no iced imports
    notes.rs             PitchClass, Letter, Accidental, Note, Spelling
    intervals.rs         Interval as a degree number + quality, not just a semitone count
    scales.rs            ScaleKind and Scale::spell — degrees derived, not looked up
    chords.rs            ChordQuality and Chord — degrees stacked over a root, spelled the
                         way scales are; the symbol grammar, read in both directions
  ui/
    mod.rs               App, Screen, Message, the focus grid, the neck's geometry, the
                         Home and Scale Trainer views
    note_trainer.rs      the Note Trainer's state machine and the screen that draws it
    interval_trainer.rs  the Interval Trainer's, on the same terms
    fretboard.rs         the neck as an iced canvas widget: one Layout drives both drawing
                         and hit-testing, held together by a round-trip test
    chord_diagram.rs     one voicing as a canvas: a fret window, mutes, barres — its own
                         Layout and its own round-trip test, for the same reason
    chord_library.rs     the movable shapes, the voicings they place, and the screen
```

The split is load-bearing: `music/` never imports iced, and the UI holds no music theory of
its own. Scale spelling is arithmetic over degree numbers rather than a table of key
signatures, which is what lets a G♯ Locrian come out spelled correctly without a special
case. Which of two enharmonic root names a scale gets is arithmetic too — fewest double
accidentals, then fewest accidentals — and only the eleven scales where that comes out
exactly level fall back to what the note is conventionally called. Chords are named by the
same rule, which is why a group in the library can hold both spellings of one pitch class:
`D♭ F A♭` costs two flats where `C♯ E♯ G♯` costs three sharps, so pitch class 1 is a `D♭`
major triad and a `C♯` minor one.

The line the chord library draws is one step further out. A shape on the neck is instrument
knowledge rather than theory — no arithmetic produces the fact that guitarists play an E
major on those strings, with the third where it is — so the shapes are a table and everything
downstream of them is not. There is no per-chord entry, no per-root entry and no per-position
entry anywhere below, and the fingering is not an entry either. A shape with every string
stopped is a barre chord: the first finger holds the index fret and the offsets above it take
the rest in order. Every placement is that, less the strings that sound open — which is why E
minor comes out fingered like E major with the third lifted, rather than renumbered from one.

`note_trainer.rs`, `interval_trainer.rs` and `chord_library.rs` each keep their state and
their views together so the state can keep its fields private — they are read all over those
views and nowhere else. `App` drives all three through the methods they mark `pub(super)` and
never reaches past them; what the neck *is* stays in `mod.rs`, since that is the instrument
rather than the drill, and they all want it on the same terms.

Key translation is the one thing that moved out of the subscription. It was a stateless
function of a key press until the library gained a text box, and iced hashes a subscription
recipe rather than the values its closure closed over — so a closure carrying "is the box
focused" would go on running with the value it was built with. The subscription now emits the
key untranslated and `update` resolves it, where `&mut self` is in hand.

## Testing

Tests live in-module next to what they cover. The music modules test spelling and interval
arithmetic directly; the UI tests drive `App::update` with synthesized messages and assert
on state, and the seeded RNG makes prompt sequences reproducible, so the drills are testable
without rendering anything.

The ones worth knowing about are exhaustive rather than illustrative, because the domain is
small enough to walk. Every chord on every root is spelled and checked for a repeated letter;
every voicing the library offers is checked to sound only that chord's notes, to stay within
a hand, to name a finger for every string it stops and never to cross those fingers. The two canvases each hold a
round-trip test — every position drawn resolves back to itself when pressed — which is what
keeps drawing and hit-testing from being edited apart.

## License

Trastea is free software under the [GNU General Public License, version 3 or later](LICENSE).
You may run, study, share and modify it; a version you pass on has to reach its users under
those same terms, source included.

The fonts are a separate matter. They are compiled into the binary but licensed under the
[SIL Open Font License 1.1](assets/fonts/OFL.txt) rather than the GPL — Leland Text ©
MuseScore BVBA, Dancing Script © the Dancing Script Project Authors, both embedded
unmodified. The OFL says plainly that a font may be bundled with software under any license,
so the two sit beside each other without either reaching into the other. Both texts ship
inside the release archives, since the binary redistributes the fonts by carrying them.

The crates are the same argument at a larger scale. Rust links them statically, so the
binary carries their code too, and MIT and Apache-2.0 both ask for their notice to travel
with a copy. `just notices` renders `THIRD-PARTY-NOTICES.txt` from `about.toml` — a couple
of hundred crates, mostly `iced`'s tree — and the release stages it beside the other two.
It is generated at release time rather than committed, so it cannot drift out of step with
`Cargo.lock`.
