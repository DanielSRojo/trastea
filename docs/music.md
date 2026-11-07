# Music theory

## Notes and pitch classes

`notes::PitchClass` has no `Display` because naming a pitch class means choosing a letter, and
that choice needs context the type does not have. `PitchClass::ALL` is a fixed-size array rather
than a slice so a miscount is a compile error. `PitchClass::NATURALS` holds pitch classes rather
than `Letter`s because its callers draw one at random and compare it against what is under a
fret, and a letter would need converting at every use. `Letter::ALL` stays private: `step` is
the only reason it exists.

`PitchClass::semitones_up_to` is always upward because a pitch class has no octave to sit below
another in — E up to C is 8, never −4. `PitchClass::shift` uses `rem_euclid` because `%` keeps
the dividend's sign: `(-1) % 12` is −1, `(-1).rem_euclid(12)` is 11.

`notes::Note` has public fields because there is no invariant to protect — every letter and
accidental pair is a legitimate note, B♯ and F♭♭ included. `Accidental::from_offset` returns
`None` beyond ±2 rather than rounding into range: triple accidentals never arise in this domain,
so they stay unrepresentable.

`notes::nearest_offset` lives in `notes` rather than beside a caller because `scales` and
`chords` both spell a degree by forcing its letter and asking what accidental corrects the
pitch; two copies of the fold would drift. The fold is `rem_euclid(12)` then subtract 12 above
6 — without it a difference reads +11 where −1 is meant, and no accidental covers +11.

`Accidental::ascii` is one shared table because `Note` and `Interval` both render an accidental
followed by something. Its empty string for a natural matters as much as the glyphs: an explicit
♮ is wrong in a note name and in a scale degree alike. The fretboard canvas draws with one font,
so the SMuFL glyphs the cards use cannot appear inside a marker; that is also why `Note` and
`Chord` implement `Display` in ASCII — `music/` cannot reach the glyph constants, which live
beside the widgets.

`Spelling::conventional_for` is the one table in the spelling path, and it is not a table of key
signatures: it picks between two finished answers when the arithmetic ties, and never produces
one. The naturals share the flats arm because they spell identically under both spellings.

## Intervals

`intervals::Interval` carries a degree number and an alteration, not just a distance, because
naming needs the number: `AugmentedFourth` and `DiminishedFifth` are the same fret and spell
different notes. `AugmentedFifth` exists for the augmented triad — without it C augmented spells
C E A♭, putting degree six's letter on degree five. `DiminishedSeventh` exists for the
diminished seventh chord. Both were added with arms in `number` and `alteration` only;
`semitones` and `Display` derive from those two, and `the_chord_degrees_derive_from_number_and_alteration`
fails if that ever stops being true.

`Interval::semitones` is derived from the degree's letter rather than a second hand-written
table: degree 4 is F, F natural is 5 semitones above C, so a perfect fourth is 5.

`Interval`'s `Display` is a degree (`b3`, `#4`) rather than a quality (`m3`, `P5`). If quality
names are ever wanted they belong in a method named for them, not a second reading of `Display`.

`Interval::between` has no caller in the app. The Interval Trainer was the expected one, but it
lights its tonal centre rather than naming a key, so nothing on it is spelled and it judges by
semitone distance. A drill that establishes a key would call it — there `F`→`B` and `F`→`B♭` are
different questions and the degree number is the answer. It returns `Option` because two spelled
notes can sit a distance apart that is not among the fifteen (`F♭` to `B♯`, a doubly augmented
fourth). Its dead-code marker is `expect` rather than `allow`, so the stale attribute warns the
day a caller lands, and the zero-warning build catches it.

## Scales

`scales::Scale` keeps all three fields private. `spelling` is stored rather than recomputed
because deriving it means spelling the whole scale, which starts from the root note; resolving
once in `new` breaks that cycle. A private `spelling` beside a public `root` would still let
`scale.root = other` compile and leave the spelling stale, so all three are sealed.

`Scale::new` exists because nobody teaches A♯ Ionian: it spells `A♯ B♯ C𝄪 D♯ E♯ F𝄪 G𝄪`, and a
learner reading `G𝄪` off a fret has to translate it back to A before playing it. The cost is a
tuple so the comparison is lexicographic for free. Doubles are weighed before the total because
harmonic minor on pitch class 8 spends six accidentals either way (`G♯ A♯ B C♯ D♯ E F𝄪` against
`A♭ B♭ C♭ D♭ E♭ F♭ G`); summed into one number they tie and fall to convention, which would reach
A♭ by luck. The cost is measured by spelling the scale rather than from a list of troublesome
roots, so it stays correct when a kind's intervals change.

`a_scale_is_named_the_way_it_is_taught` pins the rule from four directions: pitch class 1 Ionian
is D♭ (five flats) over C♯ (seven sharps); pitch class 10 Ionian is B♭, avoiding A♯ Ionian's three
double sharps; pitch class 1 harmonic minor is C♯, avoiding D♭'s B𝄫, which shows the rule is not
"prefer flats"; and pitch class 8 is G♯ Aeolian but A♭ Mixolydian — one pitch class, two names,
because the kind changed. No fixed table of root names can do the last.

### Why double accidentals are enough

`Scale::notes` is total. A degree's accidental is the sum of three ±1 terms: the root's
accidental, the degree's own alteration, and how far that letter's natural distance sits from the
major scale's distance for that degree. Naively that reaches ±3, but the extremes cannot align.
The third term is +1 exactly where a natural root's major scale sharpens the degree and −1
exactly where it flattens one, and the only flat any natural root's major scale needs is F
major's B♭ on degree 4 — which is also the only degree these formulas ever sharpen
(`AugmentedFourth`) and one they never flatten. So a +1 alteration only meets a third term of 0 or
−1, a −1 alteration only meets 0 or +1, and the sum stays within ±2.
`every_scale_spells_without_failing` checks this over all 384 representable scales (12 roots × 2
spellings × 16 kinds). It builds the scales as literals rather than through `Scale::new`, because
`new` steers away from the costliest spellings — exactly the ones most likely to strain the bound
— and those stay representable, since `spelling_cost` builds them on every call.

Whole Tone is written `1 2 3 ♯4 ♭6 ♭7` rather than the textbook `1 2 3 ♯4 ♯5 ♯6`. An augmented
sixth on an A♯ root would break the ±2 bound above, so the flats are what keep spelling total. The
card and the fretboard used to disagree here (the card read ♭6 while the neck drew G♯); the
formula now matches both. Whole Tone and Diminished are the only kinds allowed to keep a double
accidental after `new` chooses, because six or eight notes cannot be spread over seven letters
without one.

### Degree versus spelling

`Scale::degree` asks the formula directly rather than computing
`Interval::between(root_note, note)`. The round trip would spell the pitch and read the letters
back, adding a failure mode of its own (a distance outside the fifteen) beside "not in the scale",
leaving a caller two `None`s that mean different things. `degree`'s `find` is only well-defined
because no kind holds two intervals on one pitch class — a kind with both ♯4 and ♭5 would let the
first shadow the second. `the_tritone_kinds_use_the_degree_they_are_written_with` exists because
♯4 and ♭5 are invisible to every semitone-based test; only spelling from a root shows the
difference.

## Chords

`chords::ChordQuality` has fifteen qualities across five degree-number sets — `1 3 5`, `1 2 5`,
`1 4 5`, `1 3 5 6`, `1 3 5 7`. A shape on the neck carries one degree per string, so altering `3`
to `♭3` moves a finger while removing `3` leaves a string with nowhere to go. A sixteenth quality
outside these sets has no shape family; `the_qualities_fall_into_five_degree_sets` is where that
shows up, instead of as an empty voicing strip.

`ChordQuality::ALL` is ordered by distance from a plain triad rather than alphabetically, which
would file `6`, `7`, `aug` and `dim` by the accident of their spelling. `ScaleKind::ALL` leads with
the modes for the same reason.

`Augmented7` is written `7#5` so the ASCII a learner could copy matches the `7♯5` on screen, and
is said "seventh sharp five" because "augmented seventh" is heard by some as a seventh that is
augmented, which is a different chord. No two qualities may share a spoken name: a name that fits
two chords teaches the learner to call them one thing.

`Chord::new` must also reject a spelling that cannot be written at all. A diminished seventh
flattens degree 7 twice; on `C♭` and `F♭`, whose natural seventh is already a major seventh, that
lands three flats below the letter. Pitch classes 4 and 11 under a diminished seventh therefore
spell only with sharps.

`ChordQuality::extensions` needs both its halves. By name alone a major seventh extends a minor
triad, because `major7` starts with `m`; by degrees alone a dominant seventh extends a major
triad, because it contains one.

## Chord symbols

`chords::Query` is three variants rather than two `Option` fields because `(None, None)` has no
meaning; as a pair of options every caller would need an arm deciding what an empty query means.

Written forms are data on the quality rather than a fuzzy matcher's guesswork. A subsequence
matcher ranks `Cmaj7` against a typed `Cm7`, since `c`, `m`, `7` is a subsequence of both;
parsing cannot, because `m7` names one quality. Forms are case-sensitive because `M7` and `m7`
are a major and a minor seventh — the pair a learner most often tells apart. Both U+0394 GREEK
CAPITAL DELTA and U+25B3 WHITE UP-POINTING TRIANGLE parse as a major seventh: charts use the
first, and a character picker hands you the second.

`Query::parse` tries whole-input forms before a root because `dim`, `aug` and `sus` begin with a
note name; no form is a bare root, so `b` and `f#` still reach `Root`. The remainder after a root
must match a form exactly, so nothing is left over and the lookup needs no length sort —
`no_two_qualities_share_a_written_form` is what keeps it unambiguous. Accidentals after the root
are taken greedily so `Cb` always means C♭; the alternative needs lookahead into the form table
and would make `Cb` mean different things depending on what followed.

The plain major triad has an empty suffix (`C`, not `Cmaj`), which is why `forms` is a separate
list from `suffix`: the parser needs something to match. Its symbol parses back as a bare root,
which the library search wants to mean "this root, every quality".

## Pitch

`pitch::Pitch` stores a MIDI number privately; the domain word is pitch, and MIDI compatibility
is a convenience rather than a promise. It has no `Display` for the reason `PitchClass` has none:
naming it still means choosing between A♯4 and B♭4. `Pitch::from_midi` bounds at G9 even though
the guitar occupies 40..=79, because the type should not promise what only the caller knows.

`Pitch::transpose` returns `None` off either end, where `PitchClass::transpose` wraps. That
difference is why the two types exist: wrapping is the point of a pitch class, and here it would
turn a high E into a low one with nothing failing to compile. `Pitch::frequency` is
`440 · 2^((m − 69)/12)` rather than a 128-entry table, which could hold individually wrong
entries.

`Pitch::nearest` discards rather than clamps. `is_finite` is checked explicitly because a NaN
compares false to everything, so `!(hz > 0.0)` would reject it only by accident, and an infinity
would reach the logarithm. The range check comes before the float-to-int cast because `as`
saturates, which would turn an absurd reading into a legitimate-looking G9. Cents land in
−50..+50 so exactly one pitch is named; the tie at 50 resolves upward because that is what
`f32::round` does.

`pitch::Cents` is a newtype because cents travels — out of `nearest`, into a tolerance
comparison, into a label — and a hertz value is an `f32` too.

## RNG

`rng::Rng` is a hand-written splitmix64 rather than a `rand` dependency because the app only ever
picks one item from a list of a dozen or two; statistical quality matters less than the state
being carried in `&mut self`, so two draws in a row cannot come out correlated the way two clock
reads do. The old `random_note`/`random_scale_kind` pair read the clock per draw and was
correlated, which is why `Rng::from_clock` is called once at startup.

`Rng::below` uses plain modulo without rejection sampling; at these sizes the bias is around one
part in 10¹⁸. `Rng::draw_other_than` is the rule every reroll and fresh prompt follows — a press
always changes what is on screen. `draw` is handed the generator rather than capturing it, so it
can borrow whatever else it needs. Callers whose pool could shrink to one value say so with a
`debug_assert!` beside the call, since the loop would never return.
