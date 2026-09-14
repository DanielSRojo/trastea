# Trastea

[![ci](https://img.shields.io/github/actions/workflow/status/DanielSRojo/trastea/ci.yaml?branch=main&label=ci)](https://github.com/DanielSRojo/trastea/actions/workflows/ci.yaml)
[![audit](https://img.shields.io/github/actions/workflow/status/DanielSRojo/trastea/audit.yaml?branch=main&label=audit)](https://github.com/DanielSRojo/trastea/actions/workflows/audit.yaml)
[![crates.io](https://img.shields.io/crates/v/trastea)](https://crates.io/crates/trastea)
[![msrv](https://img.shields.io/badge/dynamic/toml?url=https://raw.githubusercontent.com/DanielSRojo/trastea/main/Cargo.toml&query=$.package.rust-version&label=msrv)](Cargo.toml)
[![license](https://img.shields.io/crates/l/trastea)](LICENSE)

A keyboard-driven guitar trainer for the desktop, written in Rust with [iced](https://iced.rs) 0.14.

Trastea draws a neck in standard tuning and drills four things against it: the degrees of a
scale in order, the name of a note under your finger, the distance between two positions, and
that last one again with the answer played on the guitar rather than clicked. Three more
screens answer rather than ask — a scale trainer that lights a shape, a chord library you look
things up in, and a CAGED window showing one chord in all five places it sits. An eighth
listens the same way two of the drills do: plug a guitar into an audio interface and the tuner
names what it hears. Everything is reachable
from the keyboard — the mouse is optional.

> **Status: early.** The four drills, the scale trainer, the chord library and the CAGED
> window all work. The
> library's shape table is the part still worth growing: five CAGED shapes and a handful of
> reduced ones cover the fifteen qualities, and every chord it offers is one a hand can hold.
> The CAGED window is a major-triad screen for the same reason — E and A are the only letters
> the table carries for the sevenths.

## Screens

**Scale Trainer** — pick a root and one of sixteen scale kinds (the seven diatonic modes,
harmonic and melodic minor, the two pentatonics, blues, voodoo, Spanish gypsy, whole tone,
diminished) and see every degree lit on the neck. The scale names itself the way it would
be taught — pitch class 10 under Ionian is B♭ major, not the A♯ major that would put three
double sharps on the fretboard — so there is nothing to choose and no ♯/♭ setting to get
wrong. `i` toggles the markers between note names and scale degrees; `r` rerolls a random
scale.

**Walk the Scale** — the Scale Trainer's picture, asked as a question. Pick a kind here too,
drag the root, and the screen names one degree at a time and walks the scale in order,
refusing to move on until that degree is found. Nothing is lit while it asks: a neck with every degree
marked carries the answer to every element of the sequence. What has been found so far stays
lit, labelled with the degree it satisfied, so the shape builds up as it is played.

A screen of its own rather than a mode of the Scale Trainer, for the reason Play the Interval
is a screen rather than a mode of the Interval Trainer — a reference and a drill are two
things to look at, and one screen holding both has two jobs and one keyboard. It owns the
scale it drills for the same reason the CAGED window owns its root: rerolling here would
otherwise move a screen you are not on.

The root is the one control that is also what the screen is saying. It is set on the glyph
that names it — dragged left or right, or scrolled over, a semitone every forty pixels and
wrapping past B to C — rather than on a grid of twelve buttons like the Scale Trainer's and
the CAGED window's. A screen that states the scale in 120-point type and then restates it as
twelve buttons is spending a card and twelve stops in the focus ring to say something it has
already said, so the drill spends neither: its ring is Back, the sixteen kinds, and the run's
controls.

Completing the last degree is what scores, and only that — a flash after every note would cost
seven seconds to walk a seven-note scale. A degree is judged by the pitch class it sounds, so
it is right at any position carrying it and in any octave, which is the rule both interval
drills already apply and the reason a walked scale need not literally ascend the neck.

The screen is three columns: the neck alone on the left, so nothing takes width the strings
need; the root set large on the black ground with the sequence at its feet, the degrees already
found lit and the one being asked for bright, which is the progress readout and the question in
one mark; and the kind grid on the right with the run's controls beneath it. The root is set on
the glyph in the middle rather than in a grid — see above.

`m` swaps between answering with a press and answering on the guitar, the second being the
same listener the played drill and the tuner use and the one the screen opens on — the drill is
about finding a degree on the instrument, and the neck is what you fall back to. The device is
claimed on entering the screen rather than at startup, so an app that never opens this screen
never listens. `t` sets a duration, after which the run
stops and says how many sequences it completed; `n` hides the neck outright, offered only
while playing, since in pressing mode the neck is the surface the answer is given on. `r`
picks another scale to drill. `t` and `n` report what they are set to in the `?` overlay
rather than only naming what the key does.

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

**Play the Interval** — the same prompt as the Interval Trainer's *Find it*, answered on the
guitar instead of with a click. A lit position stands for the root, an interval is named, and
the screen waits to hear it. Plug in, play the note, and it is judged by the pitch class it
sounds — so the interval is correct in any octave and at any position carrying it, which is the
rule *Find it* already applies to a press. Nothing is named here either: what it heard is drawn
as marks on every position carrying it, never as a note name, because a screen that printed
"you played C♯" could be answered by hunting for a name rather than by measuring a distance.

A note is judged once it settles, and once only. The detector refuses to release anything until
it is both confident and stable, so a fumbled fret on the way to the answer produces nothing to
judge and a strum is heard as nothing rather than as wrong; a held string confirms the same
reading every ten milliseconds and costs the streak once rather than two hundred times. A fresh
prompt is deaf until the strings go quiet — the correct answer is still ringing when the flash
retires the prompt a second later, and a prompt that took what was already sounding would be
answered by the previous answer. Detecting that break costs nothing: a note stopping, a
re-pluck, and a slide all break the detector's run before the next note confirms, so its own
definition of "a different note" is the drill's definition of "a different answer".

The one number on the screen is how far the reading sits from the pitch it was named as, in
cents, and it names nothing. The drill judges the pitch the instrument actually sounds, so a
guitar forty cents flat is judged on the note it has become — without the offset a learner
whose tuning has slipped is told only that they are wrong, over and over, with nothing to say
why. `r` skips the current prompt and `d` picks the input device.

**Chord Library** — a reference rather than a drill. The list is every chord it knows, twelve
roots by fifteen qualities, grouped under the root they are built on; typing narrows it.
`f♯` keeps that root, `m7♭5` keeps that quality on every root, `f♯m7♭5` keeps the one chord.
A typed symbol is parsed rather than fuzzy-matched, so `cm7` is a minor seventh and never the
major seventh it happens to be a subsequence of; approximate matching is the fallback for
input the grammar cannot read, where `cmj7` still finds `Cmaj7`.

Picking a chord shows its name said aloud, its notes, its degrees, and the ways to play it,
drawn as chord diagrams — a four-fret window with a position label, muted and open strings
marked above the nut, and a barre drawn as one bar rather than several dots. The picked shape
is drawn half again as large as the rest. `notes`, `degrees` and `fingers` choose what the
dots say, and it opens on fingers, which is what answers "how do I play this".

The spoken name is there because a symbol says how a chord is written and nothing about how
it is called. `C♯m7` is "C sharp minor seventh" and `C` is "C major" — the second is the one
worth printing, since a plain triad is the chord whose symbol is silent about its quality.
The names are prose, held apart by a test: two qualities that read out the same way would
teach a learner to call two chords one thing.

The shapes are derived, not tabulated. A small set of movable ones says which strings sound
and which degree each carries; placing one is addition, and changing a chord's quality moves
only the strings whose degrees that quality alters — so an open E and a barred B♭m7 come out
of the same entry. A placement that would fall below the nut, outrun a hand, run off the neck
or need a fifth finger is refused rather than shown.

**CAGED** — one root's five shapes tiled along a single neck, rather than five separate
diagrams. Pick a root and the screen places every CAGED shape of its major triad, orders them
up the neck, and draws them together: the selected one in blue, its neighbours grey, and the
frets where two of them meet in orange, because a note that belongs to both shapes is the
thing worth seeing. Roots are filled and thirds and fifths ringed, since finding the root
inside a shape is the skill the system is for. `← →` walk the cycle, `r` picks another root and
`i` switches the dots between note names and degrees.

Nothing here is tabulated. The screen asks the same placement arithmetic the chord library
asks, keeps the lowest placement of each shape, and orders them by fret — and the letters come
out as a rotation of C-A-G-E-D at every root without anything having said so. C major begins
with the C shape, A major with the A shape, and a test walks all twelve to prove it.

The neck is fifteen frets here rather than the twelve the other screens draw. Twelve truncates
the cycle: eight of the twelve roots lose a shape off the end, and C major loses its D shape
outright, which needs the thirteenth fret. Fourteen is the shortest neck that holds every
shape at every root; fifteen is what it draws, because the inlays fall on 3-5-7-9-12-15 and a
neck that ends on a marked fret looks deliberate. The screen states how many shapes it found
rather than promising five — five is what a major triad yields, and a minor one yields the
three a guitarist is actually taught.

**Tuner** — the one screen that listens. Plug a guitar into an audio interface, pick the
input, and it names the pitch it hears and says how far off it is in cents. It is chromatic
rather than six-string: it names the nearest of all twelve pitch classes in whatever octave it
hears, so a capo and an alternate tuning need nothing said about them and drop D is just D2.
The trade is that a string a full semitone flat reads as the note it has become rather than the
one it was reaching for.

The dial is the reading and the name is a label on it, because a bent note crosses a semitone
boundary smoothly while its name changes all at once. A dot rides an arc from half a semitone
flat at the ♭ to half a semitone sharp at the ♯, with a fixed mark at the top for it to arrive
at, and it turns green where the name does. Which way to turn the peg is said in words under
the dial — a learner reading the dot's side of centre has to hold "left means flat" as well,
and the words are the same length either way. `d` picks the input device and `i` switches the
name between its sharp and flat spellings.

Nothing is named unless the app is sure. A reading has to clear a confidence threshold *and*
agree with the readings before it, which is what makes a strummed chord produce no name at all
rather than a confident wrong one — measured on a real guitar, held notes score 0.996 and above
where the best frame of a strum reaches 0.987. Silence is drawn as silence, and not as a dot
resting at the top of the arc, which would read as a note perfectly in tune.

## Installing

Two ways in: `cargo` builds it from source on any platform iced supports, and a tagged
release carries a prebuilt archive for the two platforms CI builds.

### With cargo

```sh
cargo install trastea --locked
```

That drops a `trastea` binary in `~/.cargo/bin`. `--locked` builds against the `Cargo.lock`
the release was cut from rather than resolving fresh versions, which is what makes your
build the one CI tested. `--version 0.2.0` pins an older release, and `--force` reinstalls
one you already have.

This is the route for anything the release archives do not cover — Windows, x86_64 macOS,
a Linux that is not x86_64 — and the only one that needs a Rust toolchain. On Linux it also
needs ALSA development headers, since `cpal` builds against them — `libasound2-dev`,
`alsa-lib` or `alsa-lib-devel`, depending on the distribution. Without them the build stops
in a build script with a `pkg-config` error rather than a Rust one, which reads like a broken
crate and is not one. The compiler CI
uses is pinned in `rust-toolchain.toml`, but rustup reads that file from inside a checkout
only, so `cargo install` builds on whatever toolchain you have; anything at or past the
pinned version is fine.

### From a release archive

The [releases page](https://github.com/DanielSRojo/trastea/releases) carries, per tag:

| File | What it is |
| --- | --- |
| `trastea-vX.Y.Z-x86_64-linux.tar.gz` | the binary |
| `trastea-vX.Y.Z-aarch64-macos.zip` | a `Trastea.app` bundle for Apple silicon |
| `SHA256SUMS` | checksums covering both |

Each archive stages `LICENSE`, `LICENSE-fonts.txt` and `THIRD-PARTY-NOTICES.txt` beside
the binary, since the binary carries the fonts and the crates compiled into it.

On Linux, verify and unpack:

```sh
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf trastea-v0.2.0-x86_64-linux.tar.gz
install -Dm755 trastea-v0.2.0-x86_64-linux/trastea ~/.local/bin/trastea
```

On macOS, unzip and move `Trastea.app` into `/Applications`. The bundle is neither signed
nor notarized, so Gatekeeper refuses the first launch — open it once from the context menu
with **Open**, or clear the quarantine attribute yourself:

```sh
xattr -dr com.apple.quarantine /Applications/Trastea.app
```

Either one is a one-time thing; it launches normally after that.

## Running

```sh
cargo run          # launch the app
cargo test         # the unit tests, no UI harness needed
```

Rust edition 2024. Two dependencies: `iced` (with the `canvas` and `smol` features, the
latter for the timer behind the trainers' answer flash) and
[`cpal`](https://github.com/RustAudio/cpal) for audio capture. The pitch detection is not a third — it is arithmetic in
`audio/detect.rs`, because the difference function this needs is affordable at guitar rates
and an FFT would be the wrong tool anyway. The fonts under `assets/` are embedded into the
binary at compile time.

**On Linux, `cpal` builds against ALSA**, so the headers have to be present:
`libasound2-dev` on Debian and Ubuntu, `alsa-lib` on Arch, `alsa-lib-devel` on Fedora. This
is the one thing here that is not satisfied by a Rust toolchain alone, and it applies to
`cargo install trastea` as much as to a checkout. macOS needs no equivalent.

Contributors can install [`just`](https://just.systems) and run `just` to see the repo's
recipes; `just ci` runs the same gate CI runs, and needs those ALSA headers on Linux.

## Keyboard model

The focus ring is a 2-D grid rather than a flat tab order, so `↑ ↓ ← →` and the vim motions
`k j h l` move the way the layout looks. Rows of unequal width clamp instead of wrapping.

| Key | Action |
| --- | --- |
| `⇥` / `⇤` | next / previous |
| `↑ ↓ ← →`, `k j h l` | move the focus ring |
| `Enter`, `Space` | activate |
| `Esc`, `Backspace` | back |
| `?` | list the keys that work on this screen |
| `1`–`9` | jump straight to a Home menu entry |

Every arrow the chrome draws — the overlay's two rows and the back button in each screen's
top bar — comes from one subset embedded like the other fonts; see `just fonts`. The tab
pair is in almost no text face, so what the fallback found for it differed per machine; the
four plain arrows are in nearly every face but a body font sets them as hairlines against
its own optical size, which is why the back arrow read as a rule with a nick in it. Taking
all six from the same place is what makes them one set of marks. Everything else stays a
word, because a keycap glyph for it is either unavailable — there is no Esc key in Unicode
outside the symbol fonts — or no clearer than the word. Back-tab is ⇤ rather than ⇧⇥ because
⇧ is a white arrow, drawn hollow wherever it is drawn at all, and beside a line arrow the
two read as marks from different families.

Per-screen accelerators are declared in one table alongside the label the `?` overlay shows
for them, so a new accelerator documents itself. A key not claimed by the current screen is
inert there — and one claimed by two screens is free to mean different things on each: `d`
swaps a trainer's direction, picks the library's degrees, and opens the input list on both
screens that listen. `r` is the one key that means the same thing everywhere it is bound —
replace what is on screen — including on Walk the Scale, where it picks another scale to
drill. That screen's `[` and `]` step its root down and up from wherever the ring is standing;
they are brackets rather than `← →` because the arrows belong to the ring, which still has
sixteen kinds to walk there, and because the root display they step is deliberately not a
focus stop for the arrows to be claimed on. A key that holds a setting rather than firing an action says so in the overlay: that
screen's `t` reads `timer: 5:00` and its `n` reads `neck: hidden`, which is why an
accelerator's label is a `Cow<'static, str>` and not a `&'static str`.

Three widgets claim the motion keys rather than letting them move the focus ring: a trainer's
neck, which walks its cursor — Walk the Scale's does too, while it is being answered by
pressing; the CAGED window's shape row, where `← →` change the selection
outright so walking the cycle is one gesture rather than focus-then-activate; and the input
list, which claims `↑ ↓` for the same reason while it is open, on whichever screen opened it.
Tab is the way out of the first two, and `Esc` closes the third without leaving the screen —
the same thing it does to the library's search box.

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
    pitch.rs             Pitch and Cents — one sounding note, and the arithmetic to and
                         from hertz; the only part of music/ that holds an octave
    intervals.rs         Interval as a degree number + quality, not just a semitone count
    scales.rs            ScaleKind and Scale::spell — degrees derived, not looked up
    chords.rs            ChordQuality and Chord — degrees stacked over a root, spelled the
                         way scales are; the symbol grammar, read in both directions
  audio/                 samples in, hertz out; imports neither iced nor music/
    detect.rs            McLeod pitch detection — a difference function, not an FFT
    input.rs             the capture device, and the only untestable file in the tree
  ui/
    mod.rs               App, Screen, Message, the focus grid, Neck and the two necks the
                         screens draw, the twelve nameable distances both interval drills
                         ask for, the Home view, and the root buttons and formula tokens
                         that three screens share
    scale_trainer.rs     the Scale Trainer: the lit neck and the grids that choose it
    walk_the_scale.rs    the scale drill's state machine and the screen that draws it —
                         its own scale, its sequence, and the gate that decides when a
                         reading is an answer
    note_trainer.rs      the Note Trainer's state machine and the screen that draws it
    interval_trainer.rs  the Interval Trainer's, on the same terms
    play_the_interval.rs the played drill's, with the gate that decides when a reading is
                         an answer
    listener.rs          the capture device as the screens hold it: what it heard, the
                         picker, and the four states — belonging to neither screen that listens
    fretboard.rs         the neck as an iced canvas widget: one Layout drives both drawing
                         and hit-testing, held together by a round-trip test
    chord_diagram.rs     one voicing as a canvas: a fret window, mutes, barres — its own
                         Layout and its own round-trip test, for the same reason
    shapes.rs            the movable shapes and the voicings they place — instrument
                         knowledge, belonging to neither screen that asks for it
    chord_library.rs     the Chord Library: looking a chord up, and drawing the ways to play it
    caged.rs             the CAGED window's state and the screen that draws it
    tuner.rs             the Tuner's spelling and correction, and the dial as a canvas — one
                         Dial places the arc the canvas draws and the name stacked over it
```

The split is load-bearing: `music/` never imports iced, and the UI holds no music theory of
its own. `audio/` holds the same line one step further out — it reports hertz and knows nothing
about notes, because naming a frequency is theory's job and `music/pitch.rs` already does it.
That is also what makes the detector testable: a test asserting "82.41 Hz within a cent" needs
a sine wave and no musical vocabulary at all, and every one of them runs with no device
present. `input.rs` is the only file in the tree that cannot be tested, which is the whole
reason the seam sits where it does — opening a device, de-interleaving it and handing samples
across a thread boundary is all it is allowed to do, and every decision lives on the other
side of it. Scale spelling is arithmetic over degree numbers rather than a table of key
signatures, which is what lets a G♯ Locrian come out spelled correctly without a special
case. Which of two enharmonic root names a scale gets is arithmetic too — fewest double
accidentals, then fewest accidentals — and only the eleven scales where that comes out
exactly level fall back to what the note is conventionally called. Chords are named by the
same rule, which is why a group in the library can hold both spellings of one pitch class:
`D♭ F A♭` costs two flats where `C♯ E♯ G♯` costs three sharps, so pitch class 1 is a `D♭`
major triad and a `C♯` minor one.

The shapes sit in `shapes.rs` rather than inside a screen because two screens place them. A
movable shape is instrument knowledge, like the tuning and the neck itself, and a screen
importing another screen is the arrangement that keeping each screen's state and views in one
module exists to prevent — so the table moved out rather than the new screen reaching across.
A shape names the CAGED letter it derives from and, separately, how it is reduced from the full
one; the printed name is built from those two rather than stored beside them. That split is
what lets the CAGED window ask for the five without parsing a string, since the reduced entries
are also E and A shapes and also place.

How long a neck is stopped being one number when a second screen disagreed about it. A `Neck`
carries its tuning and its fret count, and the two named ones — twelve frets for the drills and
the library, fifteen for the CAGED window — are passed to the placement arithmetic rather than
read from a constant inside it. A bare `usize` parameter would have done the same work and let
a call site pass the wrong number without anything failing to compile.

A pitch class says which of twelve and deliberately says nothing else, which is what every
screen wants — a scale has no octave, a chord quality has no octave, and a CAGED shape is the
same shape wherever it sits. The neck has always known more than it said. `pitch.rs` is the
other half: a `Pitch` is one sounding note, carried as a MIDI number so that its pitch class
and its octave both fall out by division and cannot disagree, converting to and from hertz by
arithmetic over A440 rather than by a table of frequencies. The tuning is stated in those —
`E2 A2 D3 G3 B3 E4` — and `pitch_class_at` is derived from `pitch_at` rather than computed
beside it, since two parallel computations would only be unlikely to disagree. Those are
concert pitch rather than the written pitch a guitarist reads, which sits an octave above; a
tuning an octave out would spell every scale identically and tune correctly against itself, so
the test that catches it has to pin a frequency rather than a name.

Nothing above the neck holds an octave. Scales, chords, intervals and the shapes stay in pitch
classes, because none of them has a question a pitch answers, and letting one in would buy a
second description of something that already has one.

The line the shapes draw is one step further out. A shape on the neck is instrument
knowledge rather than theory — no arithmetic produces the fact that guitarists play an E
major on those strings, with the third where it is — so the shapes are a table and everything
downstream of them is not. There is no per-chord entry, no per-root entry and no per-position
entry anywhere below, and the fingering is not an entry either. A shape with every string
stopped is a barre chord: the first finger holds the index fret and the offsets above it take
the rest in order. Every placement is that, less the strings that sound open — which is why E
minor comes out fingered like E major with the third lifted, rather than renumbered from one.

The device took the same route the shapes did, one release later. `Listening`, the confirming
tracker, the picker and the poll were fields of `Tuner` while the Tuner was the only screen
that listened; the played drill wanting all four is what moved them into `listener.rs` rather
than what made one screen import another. The two screens hold one each and only one ever has
a stream open — `leaving_for` closes it on the way out, which is also what lets the audio
messages be routed by the screen on show rather than carrying a screen's name. That routing
would be wrong for the trainers' flash timers, where both can genuinely be mid-flash at once,
and it is right here for exactly the reason it is wrong there.

`note_trainer.rs`, `interval_trainer.rs`, `play_the_interval.rs`, `chord_library.rs` and
`caged.rs` each keep their
state and their views together so the state can keep its fields private — they are read all
over those views and nowhere else. `App` drives all four through the methods they mark
`pub(super)` and never reaches past them; what the neck *is* stays in `mod.rs`, since that is
the instrument rather than the drill, and they all want it on the same terms. The CAGED window
keeps only a root and an index into the shapes that root produced: the cycle itself is derived
wherever it is wanted, since a cached list is a field free to disagree with the root beside it.

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
a hand, to name a finger for every string it stops and never to cross those fingers. Every
root's CAGED cycle is walked and checked to spell a rotation of C-A-G-E-D, which is the claim
the window rests on and the one thing that would break silently if a shape's offsets were
edited. The two canvases each hold a round-trip test — every position drawn resolves back to
itself when pressed — which is what keeps drawing and hit-testing from being edited apart.

## License

Trastea is free software under the [GNU General Public License, version 3 or later](LICENSE).
You may run, study, share and modify it; a version you pass on has to reach its users under
those same terms, source included.

The fonts are a separate matter. They are compiled into the binary but licensed under the
[SIL Open Font License 1.1](assets/fonts/OFL.txt) rather than the GPL — Leland Text ©
MuseScore BVBA and Dancing Script © the Dancing Script Project Authors, both embedded
unmodified, and Noto Sans Math © Google LLC, subset by `just fonts` to the six arrows the
chrome draws. The licence permits that subset and asks only that the result stay
under it, which it does; Noto reserves no font name, so the family keeps its own. It also
says plainly that a font may be bundled with software under any licence, so the three sit
beside the GPL without either side reaching into the other. Both texts ship inside the
release archives, since the binary redistributes the fonts by carrying them.

The crates are the same argument at a larger scale. Rust links them statically, so the
binary carries their code too, and MIT and Apache-2.0 both ask for their notice to travel
with a copy. `just notices` renders `THIRD-PARTY-NOTICES.txt` from `about.toml` — a couple
of hundred crates, mostly `iced`'s tree — and the release stages it beside the other two.
It is generated at release time rather than committed, so it cannot drift out of step with
`Cargo.lock`.
