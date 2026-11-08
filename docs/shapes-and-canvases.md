# Shapes, voicings and canvases

## The shape table

`shapes::SHAPES` is the only curated data between a chord and a diagram. Which strings sound
and which degree each carries is something guitarists know and no arithmetic produces;
everything downstream of the table is arithmetic. Placing a shape adds one number to every
offset, and changing a chord's quality moves only the strings whose degrees that quality alters.

- `shapes::REACH` is three because a hand covers four frets, which is a distance of three
  between the lowest and highest stopped note. All five CAGED shapes fit inside it, and the C
  and G shapes use every bit of it.
- A string's role is an enum rather than an `Option<u8>`, because a muted string and a stopped
  one differ in kind and the degree only means anything for the second. With `Option<u8>` the
  degree would live somewhere else, free to disagree.
- A shape stores the quality it was drawn against (`base`) rather than a list of degree numbers.
  The list is derivable, and storing it would be a second thing to keep in step.
- `shapes::CagedShape` is an enum rather than a letter inside a name string, because the reduced
  entries are also CAGED shapes: `voicings` for a major triad returns seven, and a caller
  wanting the five has no other predicate. Matching on it is exhaustive on purpose — a sixth
  shape would make the compiler name every place that has to decide about it.
- Every entry is a chord a guitarist recognises at the nut — E and A major, Asus2, Esus4, E6 and
  A6, and E7, A7, D7, C7 and G7. That is the check on the arithmetic: placed at the nut, a shape has to come out
  as the open chord it was drawn from.
- Sevenths are the largest family: seven of the fifteen qualities share `1 3 5 7`. The D shape
  joined E and A there first because D7 needs no stretch the triad did not; adding it took the
  library from 592 voicings to 670. C7 and G7 followed and took
  it to 674 and then 699; G7 counts most because it is the one that barres up the neck.

The reduced shapes exist because the six-string ones only stay fingerable near the nut.
Altering a quality drags strings apart, and a placement that needs a fifth finger is refused,
which left ten of the twelve major sixths with no shape at all. Dropping a string or two costs a
doubled note nobody misses and takes the A-shape sixth from two playable placements to
twenty-two.

## Placement

`Shape::place` refuses a quality missing a degree the shape holds. Altering `3` into `♭3` moves
one finger; a quality with no third leaves the string carrying it nowhere to go, and muting that
string would be an editorial decision made by arithmetic rather than by a guitarist.

A shape may leave out its fifth, because that is a guitarist's editorial decision: open C7 is
`x 3 2 3 1 0`, the triad's G string raised to the seventh, and putting the fifth back on the top
string needs a fifth finger. The fifth is the only degree a shape may omit, since the third and
seventh are what make a quality and the root is what places it. A quality that alters an omitted
degree is refused, so the C7 shape offers C7, Cmaj7, C♯m7 and C♯mMaj7 and never a 7♯5, m7♭5 or
°7 with the note that names it missing.

A refusal is ordinary — most shapes cannot be played at most positions — so it is an absent
voicing rather than an error. The C shape holds its third on an open string, so flattening it
would need a fret below the nut: C minor is not a C shape at the nut, though it is reachable
elsewhere.

`shapes::voicings` tries each shape at every fret rather than the first that fits, because a
shape repeats an octave up and both placements are worth showing. Across every root and quality
on `DRILL_NECK` the library offers 699 voicings; that count did not change when the module
learned to carry a shape's fingering, since fingerability is a property of the placement and not
of the numbering.

`shapes::Voicing` holds absolute frets, written the way guitarists write them (`x 3 2 0 1 0`),
rather than offsets from a window. The window is derived from the voicing so the two cannot
disagree — a diagram drawn against a stale window puts its dots at the wrong frets while still
looking perfectly legible, the worst failure a reference can have.

`shapes::position_label` reads `chord_diagram::window_for` rather than the shape's index fret,
because the two come apart: C°7 on the A shape sits at the third fret but reaches down to the
second, so its window opens at the nut. Labelled by the index fret it would read `3fr` above a
picture of the nut.

## Fingering rules

Every placement starts from `Shape::movable_fingering`, the shape with every string stopped: the
first finger holds the index fret and the offsets above it take the rest in order, the lower
string first where two share a fret. It is derived from the offsets rather than recorded beside
them for the same reason `base` is a quality.

When every string the first finger holds sounds open, the finger is released and the rest come
down one to meet it. Em is E major with its third released (`0 2 2 0 0 0`), and the surviving
fingers keep their E major numbers rather than being renumbered from one. Release is decided by
whether the first finger has work, not by `index_fret == 0`: the E shape carries a fifth on its
index fret, so sharpening it stops a string the first finger was holding, and lowering on the
index fret would have numbered a dot zero.

`Voicing::fingers` asks the shape first, because a released finger is invisible to the frets —
nothing in `0 2 2 0 0 0` says the index was lifted rather than never placed. The carried
fingering is discarded when an alteration moves a string without asking the shape: a fifth
finger, a barred string held by something other than the first, one finger on two unbarred
strings, or a crossing (a higher finger below a lower one, or above it on a lower string at the
same fret). Am is the everyday case: flattening the third drops the B string under its
neighbours, so the carried fingering would cross. C♯m on the C shape is the same-fret case: at
`x 4 2 1 2 0` the carried numbers put the ring finger on the D string and the middle on the B
string above it, both at the second fret, so it is fingered in string order instead.
Without an open string, release does nothing and the carried fingering either agrees with the
sort or is thrown away, so open strings are the only place the two can differ. A real fingering
solver is still its own project.

The fallback, `Voicing::ordered_fingering`, orders by `(fret, string)`. Keying on the fret alone
was the first attempt and is wrong on the two most common open chords: G is `3 2 0 0 0 3`, two
third-fret strings with three between them, and D is `x x 0 2 3 2` for the same reason. Same
fret means same finger only when the strings are barred. Past the fourth finger a dot goes
unlabelled rather than carrying a finger a hand does not have.

`Voicing::is_fingerable` refuses a voicing needing a fifth finger alongside the placements that
run off the neck or past `REACH` — the same kind of unplayable. A diagram drawing a dot with no
number in it was the visible symptom of this not being checked.

`Voicing::barre_fret` reads the lowest stopped fret, not the index fret. C♯°7 on the A shape is
`x 4 5 3 5 3`: the alteration drops two strings below the index fret, so keying on the index
fret found no barre and left five stopped strings for four fingers. No barre is laid below five
stopped strings — open D could bar `x x 0 2 3 2`, and nobody plays it that way because three
separate fingers are free. The barred strings need not be adjacent: the bar covers the ones
between, and a string stopped higher simply sounds its own note, which is how every barre chord
with a shape on top works.

## The chord library

`chord_library::ChordLibrary` keeps its own list rather than sharing one with the Scale Trainer:
a scale has no fingering, so the two screens have different vocabularies and no reason to move
together. Its notation field is its own because only this screen can draw all three notations,
and it defaults to fingers, since the question the screen answers is "how do I play this".

Search:

- Opening the screen clears the query so the first keystroke starts a new search; the notation
  survives.
- An empty query is every chord — browsing is the zero-input case of searching rather than a
  mode. An earlier design filtered to "the last root you named", which was state the screen
  never showed anybody.
- A query that parses keeps the chords it `covers`, because a named quality is where the learner
  has typed to, not where they are going: `Cmaj` must not hide `Cmaj7`, nor `dim` the `dim7`s.
- Only a query the grammar cannot read falls back to subsequence matching, so `cmj7` still
  reaches `Cmaj7`. The hazard of subsequences — `cm7` scoring against `Cmaj7` — cannot arise,
  because `cm7` parses. Scoring never competes with an exact hit.
- The fallback also matches the quality said aloud, so `dominant`, `suspended` and `c minor`
  find their chords: a learner who knows the name before the notation is still searching. The
  spoken name is matched without its root, so every root's `dominant` scores alike and the rows
  keep library order; a leading root is split off with `chords::parse_root` and must agree.
- `gg` is the one two-key gesture in the app; any other key clears the pending `g`, so a
  half-typed motion never changes what the next key does.
- `Esc` backs out and leaves the ring where it was; `Enter` means the chord is found, so the ring
  goes to the list, whose arrows walk its shapes. That is why `SearchKey::Accepted` is its own
  case, and why the box reports what it did instead of moving `App`'s focus itself.

Building the whole cross product of 180 chords costs about 33 µs. The rows are cheap and the
widgets are not, which is why the view groups them under root headers instead of showing a flat
wall. A header is derived where the root changes rather than stored as a row, so the selection
stays a plain index and the arrows never step over anything unselectable.

Chord symbols:

- The quality is set smaller than the root, as engraved. At the root's size the diminished
  circle reaches cap height and `C°` reads as `CO`; at the quality's own size a raised circle is
  as big as the `m` in `Cm`, so superscripts are smaller again.
- Only ° and ø are raised. Raising every quality mark, which is what this did at first, makes
  `Cm` and `Csus4` look like exponents. A superscript lifted half as far sat around the root's
  middle and read as a mark that had slipped.
- `7♯5` rather than `+7`, because `+7` is read by some as a major seventh and the degree row
  beneath it says `♯5`.
- What a pitch class is called in a chord and what degree it is are both answered by `music/`;
  the library only chooses which answer to draw.

Smaller decisions:

- The notation is three buttons rather than one that cycles. A label that changes on every press
  reads as a status line, and nobody presses a status line to see what else it could say.
- A line under the symbol says the chord's name aloud — `C` is "C major" only to someone who
  already knows.
- A chord with no shape keeps its notes and degrees and says why there is no diagram, rather
  than being hidden. No quality of the fifteen that ship is in that state.
- The shapes wrap rather than scroll sideways, so the set is one block to read. The picked one is
  drawn half again as large, and its caption grows with it.
- The search caret is a glyph rather than a blinking cursor: nothing here animates, and the
  learner needs to see that the box has the keyboard.
- The magnifier is two drawn strokes because `⌕` came out as a stray `o` under the baseline in
  whatever face the system found for it.

## CAGED

The window offers `caged::QUALITIES` — major, minor, 7, maj7 and m7 — because those keep at
least three shapes at every root on `CAGED_NECK`, and a test holds them to it. Major yields five
everywhere; 7 and maj7 four, and five at C alone, because the C-shape seventh is fingerable only
at the nut; minor and m7 three or four. The diminished, augmented, suspended and sixth chords
place one or two, which the library already shows better as diagrams. The screen was written
against a `Chord` from the start, so admitting these was a selector rather than a rewrite.

- The quality is a `ChordQuality` checked against `QUALITIES` on the way in, rather than an enum
  of its own: a second enum would restate five variants for one `contains`, and the message
  carries the quality the way `SelectDrillKind` carries a scale kind. A quality not offered is
  ignored, so no pill can be unlit while its chord is on show.
- The quality pills sit in a row above the shape pills, inside the shape card, as the scale
  screens' kinds sit above their controls. They read as chord-symbol suffixes, with `maj` for the
  major triad because its suffix is empty; the root is already set large beside them.
- A new quality keeps the selection's index, clamped, as a new root does.

- The selection is an index into the derived cycle rather than a `CagedShape`, since the cycle's
  length depends on the quality and an index into what was found cannot name a shape that is
  not there.
- `caged::cycle` keeps one placement per letter, the lowest. A shape recurs an octave up, and
  CAGED means five windows rather than eleven.
- A position's claim has three states rather than two booleans, because a position the selected
  shape shares with a neighbour is a different thing to draw from one it owns alone, and that
  overlap is what the screen exists to show. The G shape's root on the low E at the eighth fret
  is inside no other window.
- Colour carries the claim and `MarkerStyle` carries the root — the two dimensions
  `fretboard::NoteMarker` already had, which kept `fretboard.rs` and its round-trip test out of
  the change.
- The root is filled and every other degree ringed, because finding the root inside a shape is
  the skill CAGED teaches and it should not wait on reading a label.
- The neck is display-only; shapes are walked with the pills and the arrows. The pills are a
  column in cycle order, which is the order the shapes sit down the vertical neck beside it.
- The root is set on the glyph shared with the Scale Trainer and Walk the Scale. The chord under
  it follows the notation the dots do, notes or degrees, so `i` changes everything that names
  a note at once; a formula fixed in degrees beside dots in note names read as a second setting.

`neck::CAGED_NECK` has fifteen frets because on twelve, eight of the twelve roots cannot complete
the cycle. The four that can are D, E, G and A — the roots of the open shapes themselves.

## The fretboard canvas

The edges of `fretboard::Layout` are methods rather than fields, since stored they could
disagree with the spacings they derive from.

- `pad_top` is built from the largest radius the neck could use, because it has to reserve the
  open-string band before `fret_spacing`, and so the final radius, is known. Collapsing it into
  `note_radius` would move the whole neck down as the window shrank.
- The cursor ring's gap and stroke are constants on `Layout` because `pad_top` reserves room for
  them: the cursor starts on the open low E, drawn above the nut. When they were separate numbers
  at the `stroke` call the ring was clipped by exactly the four pixels now reserved. At the 14 px
  radius cap, `pad_top` is 14 × 2 + 4 + 4 = 36.
- The radius is 0.35 of the smaller spacing, clamped to 8–14 px. `TARGET_FRET_CELL_RATIO` of 1.35
  keeps a fret band from growing much taller than a string is wide, past which it reads as a
  ladder. The widget is a fixed 280 wide and fills vertically, so height is what varies.
- `position_at` returns `None` off the neck rather than clamping, which on a trainer would turn a
  misdirected click into a wrong answer the user did not earn. The hit area is the marker's
  footprint, so outer-string dots register across their width.
- Fret n owns the band between line n − 1 and line n, so the nut is the top of fret 1's band, not
  the bottom of fret 0's. At the radius cap the open band fills the reserved padding exactly, so
  `y = 0` is a legitimate fret 0 at some sizes.

`fretboard::NoteMarker` takes a label already decided rather than a `Note`, so a new way to label
a marker needs no change here. It has no `Default` on purpose: a marker that silently defaulted
to filled is the bug the style field exists to prevent, and the Note Trainer's prompt spent a
while being a filled dot that read as an answer already placed.

- An outlined marker is stroked on the radius rather than inset, so the ring and the dot it
  replaces are the same size; the half-stroke overhang is covered by the hit slack. It is wider
  than the cursor ring's 2 px, since a prompt must not read as thinner than a focus ring.
- The cursor is a ring, not a dot — a second filled circle would read as a second lit note — and
  it is drawn last so no marker buries it. The caller owns it, since the app runs its own focus
  system and the canvas cannot know whether it is focused.
- `on_press` is a plain `fn` pointer: every caller passes a bare enum constructor, and a
  `Box<dyn Fn>` would buy capture nobody uses at the cost of an allocation and `Copy`.
  `Default` is hand-written because the derive would demand `Message: Default`.
- A neck with no handler neither captures events nor asks for a redraw. A hit captures the press
  so it does not fall through to whatever is behind the canvas; a miss leaves the event alone.

## The chord diagram canvas

`chord_diagram` is a sibling of `fretboard` rather than a mode on it. The neck draws twelve frets
from the nut and has no way to say a string is not sounded — a mute is an annotation above the
nut, not a marker at a position. A window offset, a barre and a mute bolted onto `fretboard` would
make its round-trip invariant conditional on a mode flag. The diagram's geometry is the harder
of the two, which is reason to want the test more.

- `MIN_WINDOW` is four so every diagram in a strip is the same height and the strip reads as a
  row of chords. `NUT_REACH` is two because a shape stopping at the second fret still looks like
  an open-position chord; past that the nut is dead space and the window follows the hand.
- Two bands sit above the nut: the mark row, and a line above it for open strings' labels, which
  have no filled disc to sit on. With only the mark row reserved the label landed off the canvas.
- An open string is a ring above the nut, never a filled dot, which would read as a note stopped
  at the nut. The mark row resolves to fret 0 only when the nut is shown.
- Dots are 0.34 of the spacing, clamped to 7.5–13 px; below 7.5 a two-character name like `Bb`
  stops being legible. A three-character label (`Bbb`, `bb7`) is shrunk rather than clipped or
  given a larger disc, since discs must stay one size across a strip.
- `LABEL_GUTTER` takes 28 px for the `5fr` label out of the width before strings are spaced, so
  the neck stays centred on what is left.
- One struct per string rather than parallel arrays, so a fret and its label cannot end up on
  different strings.
- `position_at` returns `None` off the diagram. The widget is display-only for now but hit-tests
  so a later editing pass has a surface, and there a stray press clamped onto a position would
  silently rewrite a chord.

`STRIP` is 180 × 188 and `FEATURE` 264 × 292. A dot's radius follows the width, so the picked
diagram's labels come out about half again as large rather than merely further apart — legible
across the room, with the others scannable. The heights are chosen against the widths to hold the
same cell proportion `fretboard::Layout` targets, and `a_cell_is_about_as_tall_as_it_is_wide`
catches either constant being nudged by eye.
