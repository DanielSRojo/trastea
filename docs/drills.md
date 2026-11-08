# Drills

## The shared vocabulary in `drill`

`drill::ANSWERS` is `Interval::ALL` with each distance that has two spellings collapsed into
one entry: the tritone, and the two chord degrees the library added to `Interval`. It lives in
`ui/` and not in `music/intervals.rs` because merging two spellings is a statement about what a
fretboard can tell apart with no key established, and `music/` holds no instrument. It sits in
`drill` rather than in either screen because both interval drills mean the same twelve
distances by it.

- One table feeds the grid, the focus rows and the prompt pool, the way a Home category's
  `entries` feed its buttons, focus cells and digit keys. Twelve happens to divide evenly into
  `INTERVAL_ROW_WIDTH`, which the Interval Trainer's grid relies on and the played drill does
  not; the table is a vocabulary, not a layout. The width is three rather than the Note
  Trainer's four because the answers sit in the 390-wide selector column of `root_stage`, and
  four buttons wide enough for the tritone twin do not fit it.
- The twin is carried as an `Interval`, not as a written label, so a button's text cannot drift
  from the interval it stands for.
- `App` reaches the table through `drill::ANSWER_COUNT` and `drill::choice_for`, which keeps
  `AnswerChoice` and its twin private. `choice_for` falls back to a twinless entry instead of
  returning an `Option`, since neither drill can produce an interval outside the table and no
  caller could act on the failure.
- The test that the table holds twelve entries at twelve distinct distances is the twin
  decision stated as an invariant. Judging compares semitones, so two buttons for one distance
  would be two spellings of one answer with no way to say which the drill meant. Covering
  `0..=11` exactly means no prompt can be left without a button.

`drill::Drill` is transient: it tells `draw_prompt` which kind of prompt to make and is never
stored. The stored direction is the `Prompt` variant.

## Scoring in `drill::Score`

`Score<A>` is generic over the answer rather than a trait the drills implement, because it
never asks an answer anything except whether it is the same one. `Copy + PartialEq` is the
whole contract, and each drill keeps its own judging. The three trainers score through it;
Walk the Scale does not (see below).

- `wrong` is a `Vec` because the learner may keep trying, and keeping only the last wrong
  answer would un-mark the earlier ones.
- `correct: Option<A>` is the paused state. One `Option` instead of a flag beside the answer
  means "paused" cannot be set with nothing to mark.
- `Default` is written by hand because `#[derive(Default)]` would demand `A: Default`, which no
  answer type has and none needs.
- `mark` ignores answers while one is lit. Those are the tail of the press that scored — a held
  key, a double click — and counting them would inflate the streak or mark the learner wrong
  for a question already answered. A right answer does not replace the prompt; it stands, lit,
  until the drill's `advance` retires it.
- A skip and a settings change zero the streak for the same reason a wrong answer does: a
  streak that counted across them would not measure recall. The session best survives.

## Prompts and answers in the two trainers

In `note_trainer` and `interval_trainer` the `Prompt` variant *is* the direction; there is no
separate `direction` field. Two encodings of one fact could disagree, and a `FindIt` prompt on
screen beside a field saying `NameIt` is a state the view would render as nonsense. The
directions also carry different payloads — *Name it* knows a position and wants a pitch
class, *Find it* the reverse — which is the other half of why this is an enum and not a
struct with a tag.

- `interval_trainer::Prompt::NameIt` stores no interval. The two positions already imply it,
  and a stored `MajorThird` beside two positions four semitones apart would be a state the view
  renders as a lie; judging recomputes the distance from what is on the neck. `FindIt` has to
  store its interval, since no target is drawn, and its target pitch class is derived from root
  and interval for the same one-encoding reason.
- The distance is measured upward and modulo the octave, so a target below the root still reads
  as the interval it is above the root's pitch class. The drill asks about pitch classes, not
  fret counts.
- `Answer` mirrors `Prompt` because the two answer surfaces mirror the two directions. A
  mismatched pair is a wiring bug the user cannot produce, and judging returns `false` rather
  than panicking, so the symptom is "every answer is wrong" instead of a crash.
- `Answer`, `Prompt` and `note_trainer::Pool` stay private: the `pub(super)` methods take a
  pitch class or a position and build the `Answer` themselves. `answer_position` also moves the
  cursor, because a press is a move-then-answer and two calls would leave the caller an
  assignment to forget.
- `is_flashing` returns a `bool` and not the `Option`, since the subscription only needs to know
  the pause is on.

The `Rng` is passed into `new` and everything that redraws, because it lives on `App` beside
the drill and `&mut self.note_trainer` plus `&mut self.rng` are two mutable borrows of one
`App`. A generator owned by the drill would mean a second seed. There is no `Default`: a
constant first prompt would open every launch on the same note.

## The root on the interval screens

Both interval drills stand on `root_stage`, as the scale screens do: the neck, the root's name
in 120-point type with the question under it, and a column on the right with the answers on
the root's line — in *Name it* only — and the controls and streak at its foot. The glyph is
`still_root_glyph`, with no gesture canvas under it, because the root is the drill's to choose
and a drag that rerolled the prompt would be a second skip.

Naming the root was a reversal. Both screens used to name nothing, so that a prompt could be
answered by measuring and by nothing else. The root's name was let in because both drills are
about intervals: knowing the root is F♯ tells you nothing about which ring is a minor third
above it without the interval arithmetic the drill is training. The target is still never
named, and Play the Interval still draws what it heard as marks rather than a name, since
either would let the answer be read instead of found. The name is
`PitchClass::conventional_name` — with no key to spell in, a black key takes the spelling it
usually goes by.

## Drawing a prompt

`draw_prompt` is a rejection loop that never repeats the prompt on screen. It terminates only
because every pool holds at least two distinct prompts — seven naturals at the smallest — so
its `debug_assert!` is load-bearing. A future "drill one note" setting, or one pinning the
interval root to a single position, would turn it into a hang.

The interval prompts (`interval_trainer` and `play_the_interval`) are generated forward: root
and interval first, target derived. Lighting two positions and asking what lies between them
could land on a distance the vocabulary has no button for. A unison excludes the root's own
position and lights an octave elsewhere, since two rings on one dot show no distance to
measure; every pitch class occurs at least six times within twelve frets, so something is
always left.

The Note Trainer's pool starts on the naturals because it is the smaller map and has to be
automatic before the accidentals between its notes mean anything. All twelve answer buttons
show under either pool: narrowing them would make a wrong answer unreachable, and a drill you
cannot fail teaches nothing. The trainer keeps its own `Spelling`, separate from the Scale
Trainer's, and it is the one purely cosmetic toggle — judging compares pitch classes, so it
cannot change whether an answer is right.

## The flash

A correct answer holds the prompt for `ANSWER_FLASH`, one second, then the trainer's own tick
calls `advance`.

- `advance` is a no-op outside the flash, so a tick that crossed a skip or arrived after the
  screen was left cannot retire a prompt the learner is still reading.
- Each trainer has its own advance message because both can be mid-flash at once; the Note
  Trainer's tick must not reach the Interval Trainer.
- Every skip and toggle goes through `draw_prompt`, which clears both kinds of feedback, so
  anything that retires a prompt also ends its flash.
- The right answer joins the wrong marks instead of clearing them, so for the length of the
  flash the learner sees what they tried and what it turned out to be.
- The tick is the clock, not the user, so it does not dismiss the help overlay the way a
  keypress would.

## Marks on the neck

Across the drills a fill means "you put this here" and a ring means a question. The prompt is
an unfilled ring because a solid dot read as an answer already placed; guesses are filled in
the danger or success colour. The Interval Trainer tells root from target by colour, the way
red and green discs are already told apart, not by shape.

- Prompt marks are unlabelled: in the Note Trainer a label would print the answer inside the
  question, and in the interval drills it would name a note, which is the thing they exist to
  avoid.
- The right answer is drawn last, so on a fret pressed twice the green, the newer news, is on
  top. In an answer button the same rule holds: right beats wrong should both ever apply.
- Guesses from the other direction are filtered out rather than treated as errors; both lists
  clear on every advance, so they never mix.
- In *Find it* the root ring stays on the neck as the anchor while the cursor moves.
- The header labels each control with the mode it is in, not what pressing it would do, so the
  row is also the only status line saying which way the drill runs.
- `interval_trainer::ANSWER_BUTTON_WIDTH` is 76 against the 50-pixel square `ROOT_BUTTON_SIZE`
  because the twinned labels are twice as long, and columns that changed width with their
  contents would stop reading as a grid.
- The cursor stops at the neck's edges rather than wrapping, as the focus ring does at a grid's
  edge. Up is towards the nut, since the nut is drawn at the top.

## Playing the Note Trainer

The Note Trainer's playing mode is a third prompt shape, `Prompt::PlayIt { pitch_class,
string }`, beside an `Answering` mode as on Walk the Scale, with a `listener: Option<Listener>`
that exists only while playing — so "holds a device only while playing" is the type rather
than a check — and a `direction` kept outside the prompt so swapping back asks the way it was
set. Direction lives in its own field rather than in the prompt's variant, as it did, because
a playing prompt has none and the pressing one has to survive it.

- Judging is in two stages. On confirmation a pitch class other than the one asked for is
  wrong at once; no placement is needed to know it. The asked one waits for its placement,
  about three fits, and then any position on the asked string is right — named, or one of a
  narrowed group — positions only elsewhere are wrong, and a decline or an unnamed estimate is
  no judgement at all.
- That last is the benefit of the doubt. A note the screen cannot place says nothing about
  where the learner's hand was, and costing the streak for the screen's blindness would teach
  the learner to distrust the drill rather than their hand. The screen says why — slid,
  unreadable, or no string told apart — and keeps the prompt to be played again.
- Prompts the calibration cannot judge are never drawn. A position in one of `position::groups`
  is excluded, along with the rest of its pitch class on that string: the group's other string
  would place there too and be judged right, so the prompt would not ask what it says it asks.
  A flagged string is excluded outright. The pool is derived from the calibration on each draw
  rather than stored, so it cannot disagree with it, and follows the naturals-or-all-twelve
  setting as pressing does.
- With no calibration the pool is empty and the prompt is `Prompt::Unaskable`, which points at
  the Position Finder; there is no calibration walk here, by design.
- A pool of one is not drawn from, since `draw_other_than` could never return.
- The gate is a third copy of the played drills' `Gate`. The Changes Trainer is still the
  screen expected to settle whether it is one abstraction, and the copy names its twins.
- Playing stands on `root_stage`, as Play the Interval does: the asked note in
  `still_root_glyph`, its string and any notice under it, and the controls, streak and device at
  the right column's foot. A named note is the question here rather than its answer, so it can
  be set large the way *Name it* could never set it. Pressing keeps its cards: there the right
  column holds twelve answers, and *Find it*'s note is small beside a neck that is the answer.
- The header grows a fifth control, the answering toggle, and loses direction while playing.
  Answers played are kept as `Answer::Played(PitchClass)` in `Score` and their places in two
  `Placed`s — right and wrong apart, since the asked note can be wrong on one string and then
  right on another for the same prompt.

## Play the Interval

`play_the_interval::Prompt` is a struct, not an enum: there is one direction. A learner cannot
name an interval by playing one, because what is played is the answer rather than a
description of it. For the same reason the header has one control, skip.

`Gate` is two states, not a timer. A debounce was the alternative, and its duration is a
number nobody can pick: shorter than the ring of an open low E and the previous answer answers
the next prompt, longer and a learner on a run waits.

- A fresh prompt arms the gate off what the device is hearing now rather than setting it to
  `Deaf` flat. Entering the screen in silence arms at once; a prompt drawn over a ringing answer
  waits. A flat `Deaf` would behave the same in the running app, with the next poll 10 ms
  away, and be a lie for those 10 ms.
- `apply` is asymmetric: a reading that is not a pitch only re-arms. That is what makes
  silence, a strum and a fumble on the way to the answer free.
- During the flash, what is sounding is the tail of the note that scored and is ignored.
- A held string confirms the same reading every 10 ms poll; fifty polls is half a second of
  ringing and still one answer. Re-plucking the same note is a second answer, because the
  silence between re-arms the gate. A slide needs no silence: `Tracker` releases a non-pitch
  reading once the pitch moves past its tolerance.

The octave is ignored on purpose. A root at the third fret of the low E asked for a minor
seventh has no answer above it, inside one octave, that the hand can reach from where it sits,
and an octave-exact rule would drill where the octave is — the Note Trainer's knowledge — on top
of the interval. The cost is that an octave error is never caught, the trade *Find it* already
makes.

- An answer is marked where it was played: at the one position its placement names, or at the
  positions it narrows to, and at every position sounding its pitch when it cannot be placed —
  for want of a calibration, or because it was slid or unreadable. The drill hears the octave
  even when it cannot hear the string, so an unplaced E4 is two or three marks rather than the
  eight Es on the neck; the pitch class alone was the first rule, and on an uncalibrated guitar
  it lit the neck with places the note could not have come from. Judging still happens on
  confirmation, and the answer is drawn where its pitch sounds from then; the placement arrives
  about three fits later and only narrows those marks, never moving one to a position that was
  not marked. The placement is kept in
  `Placed` beside `Score` rather than in it: `Score<A>` stores answers by value and is shared
  with two screens that never place anything.
- A placement is attached to the answer judged for the note that produced it: `Placed::expect`
  takes the judged pitch and `Placed::place` takes the first placement of that same pitch. The
  gate and the placer break on the same non-pitch release, so the note just judged is the note
  being placed without further bookkeeping; a placement for a note the gate ignored — during
  the flash, or ringing into a fresh prompt — is dropped. An answer whose note ended before any
  fit is never placed and stays where its pitch sounds.
- The root's position wins any collision, so playing the root back, or answering a unison,
  cannot repaint the anchor as an answer. The other positions of that pitch class still take
  the answer's colour.
- The live reading is drawn only when it was heard while the gate was deaf, since judging is
  immediate; that is the previous answer ringing into a new prompt, drawn as heard and not as
  scored.
- The neck has no press handler, as on the CAGED window.
- `heard_line` returns the words apart from the widgets so the tests can assert that the four
  listener states read differently and that no string names a note — the claim the screen rests
  on, and one a widget tree cannot be asked about.
- The cents value has `+ 0.0` added because rounding keeps the sign it rounded away, and a hair
  flat would print "-0 cents", which reads as a fault.

Tests reach the screen with `navigate_to` rather than `open`, so no capture device is claimed
and `Message::Heard` delivers readings directly. A test's reading is a `Pitch`, not a
frequency, and its clarity is already above the gate: this is the released seam, and whether a
strum gets that far is tested in `audio::detect` against real waveforms.

## Walk the Scale

`walk_the_scale::Gate` is a copy of `play_the_interval::Gate`, deliberately. The two agree
today, and the Changes Trainer — walking a progression, where the chord under an element moves
as well as the degree — is the screen that will show whether this is one abstraction or two.
`shapes.rs` and `listener.rs` moved out when a second screen wanted them, not when a first
author guessed, and this copy waits for the same signal. It names its twin so the extraction
has both sites to read.

The scoring does not use `drill::Score`: the streak counts clean sequences rather than
answers, and the best streak outlives each run.

### State

- `Mode` has two states. Entering the screen starts a run, so there is no display state to fall
  back to, and a finished run's summary is a state of its own rather than the absence of one,
  which is why it is not an `Option<Run>`.
- Everything belonging to one run sits in `Run`, behind one `Mode` variant, so nothing from the
  last run survives into the summary.
- `Run::at` is all that is stored about the sequence; the degrees are read from
  `scale.kind().intervals()`. A stored sequence would be free to disagree with the scale, the
  argument the CAGED window's derived cycle makes. `Run::degree` uses `get` because a kind
  change could leave `at` past a shorter kind's degrees — Diminished has eight, Minor
  Pentatonic five — and changing the root or kind restarts the sequence so that never shows.
- A sequence is clean until it is answered wrongly, and only a clean one raises the streak.
  Elements advance as they are judged; only the completion flashes, since a flash per degree
  would cost seven seconds to walk a seven-note scale.
- `neck_hidden` lives inside `Answering::Playing`. In pressing mode the neck is the answer
  surface, so "pressing with the neck hidden" has no answer; putting the flag in the variant
  makes it unwriteable, and swapping to `Pressing` drops the flag, which is the whole
  implementation of "swapping back shows the neck".
- `Answering` is a preference that survives leaving the drill. The `Listener` is not inside it:
  a device living in `Playing` would stay open after the run ended. It is opened and released
  explicitly, and leaving the screen ends the run and releases it, so a duration is never spent
  behind a screen nobody is looking at.
- On entry the scale is what it was left on — the CAGED window's rule, not the Scale Trainer's,
  since a learner comes back to the scale they were working on — the run is fresh and the best
  streak persists. The screen opens playing; the device is claimed by `enter`, not at
  construction.
- After a completion the next sequence's gate is armed off the device, because the completing
  note is still ringing and would otherwise answer the first element.
- A press on the neck is inert in playing mode, where it would be a second way to answer.

### Timing

`Run` stores when it started, not a countdown: a `remaining` decremented on a tick drifts, and
lies outright if ticks are missed while the window is not drawn. The test jumps the start back
three minutes in one step, where a countdown would still read 3:00. `DURATIONS` is none, 3, 5
and 10 minutes, with `None` first so the first press of `t` steps into timing. An untimed run
is `None` rather than a flag, so "no duration" and "runs until you leave" are one fact.
Cycling the duration restarts the run, since a duration applied part-way would count from the
old run's start.

### The root gesture

The gesture lives in `root_stage.rs`, shared with the Scale Trainer and the CAGED window; it was
written here and moved out when they wanted it. The root glyph is set by drag or wheel at `STEP_DISTANCE`, 40 pixels a semitone: wide enough
that a slip on the glyph does not walk the scale, narrow enough that all twelve are one gesture
(480 pixels back to the same root). Nothing in the geometry reads it.

- The drag anchor is an `Option<f32>`, so "dragging from nowhere" cannot be written. Whole steps
  are consumed off the anchor rather than measured from the press point, so the drill never
  remembers the starting root and the pointer has to return to undo a step.
- Wheel travel short of a step is kept between gestures, so a nudge and a nudge back cancel.
  Without the remainder a trackpad, whose deltas arrive in pixels far short of a step, would
  never move the root. The wheel is ignored mid-drag: one gesture owns the root at a time.
- `ScrollDelta::Lines` comes from a notched wheel and is one step per notch; `Pixels` from a
  trackpad or high-resolution wheel is worth what a drag pixel is worth, so the wheel needs no
  constant of its own. Only the vertical axis counts, or a diagonal flick would step twice.
- The surface is a canvas rather than a `mouse_area` because a canvas's `update` sees every
  event wherever the pointer is, which lets a drag continue past the glyph. The press is the
  one moment the pointer must be on the glyph; it carries the window's x, because
  `position_in` is `None` as soon as the pointer leaves. A wheel event over the glyph is
  captured whether or not it stepped, which keeps anything else from scrolling.
- Whether a drag is under way is read off `App`'s one `RootDial`, not kept in a widget `State`,
  so `App::update` owns the gesture and the view cannot go stale. The dial yields steps rather
  than holding a root, and `App` hands them to the screen on show; leaving a screen drops it,
  so a drag cannot outlive the glyph it began on and no screen inherits another's remainder.
- The canvas draws nothing. The glyph is a widget stacked over it because a `Frame` cannot
  measure text and the letter and accidental are in two fonts — the reason the Tuner's name
  sits over its dial. `ROOT_ROW_HEIGHT` is fixed so the canvas does not move with the window.
- The grab box is one struct read by the press, the wheel and the pointer shape. Its half-width
  is fixed at 80 — half a letter and accidental at the 120-point `ROOT_DISPLAY_SIZE` — and not
  measured per note, so the box does not change size under the hand mid-drag. It is clamped to
  the canvas so it cannot claim presses on what is drawn beside it.

### Layout figures

- `NECK_COLUMN_WIDTH` is fixed at 340 because the fretboard canvas caps its own width off its
  height to keep a twelve-fret neck's aspect ratio; a share of the window would leave the slack
  beside the neck. The number is what the neck draws at this window height plus its padding.
- These figures are `root_stage`'s, and hold for the Scale Trainer and the CAGED window too.
  The CAGED neck has fifteen frets in the same column; the canvas is a fixed 280 wide either
  way, so only its fret spacing changes.
- `SELECTOR_COLUMN_WIDTH` is 390, the widest `kind_row_spans` row — `Melodic Minor`
  and `Minor Pentatonic` — plus the card's padding. From `Major Pentatonic` on the kinds sit in
  pairs, which took the column down from 470; the cost is a seventh row, which the root band
  has the height for. A
  narrower column draws the widest row past the card's edge. `the_widest_kind_row_fits_the_selector_column` guards it with a deliberately
  generous estimate of 0.58 em per glyph, since no font is available to measure in a test; the
  Scale Trainer never hits this because its kind card is half the window wide.
- The middle column has no card, since a card is a boundary and the root is the one thing the
  screen is saying.
- The root and the kind card share a horizontal line: each is centred in a `ROOT_BAND_HEIGHT`
  band that `on_root_line` places identically in both columns, with free space split one above
  to two below so the line sits a little above the middle, nearer where the eye reads the neck.
  It is the root that is centred, not root and sequence together as before; the sequence hangs
  below it, in a container given the same fill as the space above the glyph, which is what
  makes the centring exact rather than the pair's. The band is fixed rather than measured because iced cannot align across columns,
  so it must be at least the card's height — `the_kind_card_fits_the_root_band` guards that.
  The drill panel is stacked over the column's foot instead of following the card, so its
  height does not move the line.
- The sequence is the progress readout: found degrees lit, the current one bright, the rest
  muted. A `2 of 5` beside it would be a second description of the same fact, and the drill
  panel does not repeat the degree being asked for for the same reason.
- `press`/`play` are words, so they are not set with `control_glyph`, which pins its content
  to one glyph's width and centres anything longer off-centre. The remaining time is only
  pushed to the edge when there is one, so an untimed run stays centred.
- Marks take priority flash, then wrong, then answered, because the completing note is also the
  last one answered. Answered marks are labelled with the degree, never the note.
- In playing mode a found degree and a wrong answer are marked where they were played, by the
  rule Play the Interval follows: the placed position or group, and every position sounding its
  pitch when the note could not be placed. A walked scale therefore builds up as the shape
  the hand actually made rather than every place that shape could have been. Found degrees and
  the current element's wrong answers are kept in separate `Placed`s on the `Run` — a wrong
  answer can be a later degree's pitch class, played somewhere else — and both clear with what
  they belong to: the wrong ones when the element advances, all of it when the sequence does.
  Pressing never places anything, so it keeps marking every position of the pitch class.
- The focus ring walks Back, the sixteen kinds in row order, then the controls. A two-band split
  left from the old root-card layout walked the first three columns of every kind row before
  coming back for the rest, which nothing but that test would notice.
- Once a run has ended, `p` is the only key the screen claims; the settings keys have nothing
  to report about a summary.

## Scale Trainer

`scale_trainer` is split from `mod.rs` on size alone — four hundred lines of view nothing else
calls. It keeps no state: the `Scale` and `Notation` live on `App` because three screens read
them and none of them can put the pair into a bad state.

- The screen stands on `root_stage`, as Walk the Scale does: the neck, the root glyph with the
  kind's formula under it, and the kind card level with the root. The root grid and the summary
  card went with the move; the glyph names the root and the kind card shows which kind is on, so
  both restated what the screen already said. The feel and usage text sits at the foot of the
  centre column, as the drill panel does on the right, so its two or three lines cannot move the
  root line.
- The notation control shows the degree as `♭3`, not `1♭3`. The longer form was tried for
  reading less like an instruction to flatten the third, and crowded the old header row.
- `spell` stays the membership test in both notation modes, so the mode can change text and
  never make a dot disappear; the degree lookup is an `expect` because
  `degree_and_spell_agree_on_membership` in `scales.rs` proves a spelled pitch class has one.
- Markers ask `DRILL_NECK` for its extent rather than pairing the tuning with a literal twelve
  that agreed with it by coincidence.
- The root is highlighted on `PitchClass` equality with `scale.root()`. `Note` equality would
  colour the same today, but only while two invariants in `scales.rs` hold — distinct pitch
  classes within one scale, and `notes()[0] == root_note()`.
- With the ♯/♭ toggle gone, the check that a naming choice never moves a marker lives in
  `naming_never_moves_the_scale` in `music/scales.rs`, over all 192 root-and-kind combinations.
