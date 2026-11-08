# The UI shell

## App, Message and Screen

`ui::App` owns its RNG rather than reaching for a global, so every draw is a state change the
borrow checker can see. That is also why `App` has no `Default`: a default seed would have to
be a constant, and an app that replays the same scales every launch is worse than none. The
generator is passed into the drills' methods as a parameter, which lets an arm borrow
`self.note_trainer` and `self.rng` at once as disjoint fields.

Rerolling once reseeded from the clock on every draw, which made root and kind functions of
the same instant: only lcm(12, 16) = 48 of the 192 pairs could come up, and C Dorian was
unreachable. Root and kind now come from one advancing stream. A reroll that lands on the
scale already on show is rejected and redrawn so the button never looks broken, which is also
why the first scale of a session is never C Ionian. `App::reroll_caged_root` follows the same
rule with one field to draw.

`App` is the one place a `Scale` is built, because its spelling is derived from root and kind
together; five call sites each calling `Scale::new` would be five chances to pass a stale
field. The old spelling toggle and its two tests are gone for the same reason — spelling is
not a setting to preserve across a reroll. What they protected, that naming a scale never moves
it, is `naming_never_moves_the_scale` in `music/scales.rs`, asserted over all 192 combinations,
and the rest is carried by `Scale`'s private fields.

`ui::Notation` is a UI type, unlike `Spelling`, which sits on `Scale`. Spelling changes what
the notes are called, and a name is music. Notation picks which of two true things about an
already-determined scale gets drawn, so `music/` has no business knowing about it. Keeping it
off `Scale` also lets it survive a reroll. The Scale Trainer's field never holds `Fingers`; the
library and the CAGED window each keep their own setting, so picking fingers on either cannot
change what the Scale Trainer's markers say. The CAGED window shared `App`'s field until it
gained fingers. Both chord screens draw `Notation::ALL` with `notation_pills`, each with its own
message and focus target, so a pill on one cannot move the other's setting. The library's three notation buttons replaced one that cycled: a control that
changes what it says each time it is pressed does not look like a control with options, so
nobody presses it twice to find out. `Message::SetNotation` means "this one" where
`ToggleNotation` means "the next".

Each screen's state is its own struct rather than more fields on `App`, so its rules stay
testable without an `App`. Walk the Scale is its own screen rather than a mode of the Scale
Trainer, as Play the Interval is not a mode of the Interval Trainer: a reference and a drill
are two things to look at, and one screen holding both had two jobs and one keyboard.

### Messages per screen

Every screen that could share a message with another has its own variant instead —
`RerollCagedRoot`, `RerollDrillScale`, `SelectDrillKind`, `SkipPlayPrompt`, and one flash
timer per drill. A shared `RerollScale` or `SelectScaleKind` would move the Scale Trainer's
scale from another screen, and a shared `AdvancePrompt` routed by the screen on show could
retire the other trainer's prompt: both trainers can be mid-flash at once, and a flash left
behind keeps its timer alive until it is retired. That is harmless because `advance` is a
no-op once the prompt has moved on, and a tick only reaches the trainer its variant names.

The audio messages are the exception and are routed by the screen on show. That is sound only
because one screen can hold the device at a time (see the listener below).

`Message::OpenSearch` is not an `Accelerate`: `Ctrl+K` is carved out of `COMMAND_MODIFIERS`
by name, and both of its keys have to reach one target rather than whichever focusable a
screen lists. The guard scoping it to the library is what keeps `Ctrl+K` on another screen
from focusing a box nobody can see.

### `update` ordering

The timers are handled ahead of the help overlay, because the overlay is modal and anything
that reaches it dismisses it. The flash timers would otherwise spend a tick dismissing a panel
the learner is still reading; the run clock has to spend its duration rather than close the
panel; and the audio poll ticks every 10 ms, so before the fix `?` on the Tuner showed the
overlay for one frame. The poll also keeps the reading current behind the panel instead of
letting it go stale and re-confirm on close. `a_timer_tick_does_not_dismiss_the_help_overlay` keeps all three
timers in one test, so a fourth added above the guard is caught.

Handling the overlay once at the top rather than per message spares `GoBack` from meaning two
things and stops an accelerator from firing blind behind the panel. The overlay is a flag
rather than a `Screen`, so the screen underneath stays active and the overlay reads its keys
from `self.screen` instead of recovering them from the navigation history. It contributes no
focusable, which is why focus needs no notion of a modal layer.

`Esc` peels one layer at a time: it closes the device list before leaving the screen, ends a
Walk the Scale run before leaving that screen, and leaves the library's search box without
leaving the library. A learner pressing `Esc` mid-sequence means "stop asking", not "take me
Home". While the device list is up, `Enter` picks a device; firing the ring behind it would
reopen the list.

The library's selection-changed task is computed in a wrapper that compares before and after,
rather than in each arm. The selection moves from the arrows, a click and the query changing
under it, and threading a `Task` out of each would spread one concern across all of them.
Inner calls go to `dispatch` rather than `update` so the before reading is taken once.

### Entering and leaving screens

`App::navigate_to` holds every entry rule so the menu button, Tab-and-Enter and the digit
accelerator cannot drift apart.

- The Scale Trainer draws a fresh scale on entry and never reopens on the one it last showed.
- Walk the Scale keeps its scale, as the CAGED window does, since a learner comes back to what
  they were working on; the run is fresh and the best streak persists.
- The trainers draw a fresh prompt; settings and best streaks persist.
- The library opens with the search box focused, since a ring parked on Back would make the
  first keystroke do nothing. The query is cleared but the root survives, so an empty box
  still means that root, every quality.
- The CAGED window is a reference rather than a run, so it reopens on what it last showed;
  `enter` only re-establishes the selection's invariant.
- The listening screens open the device on entry rather than on a press. A tuner that has to
  be started looks broken until someone notices; a failure is a state the screen draws.

`App::leaving_for` is a separate method from `navigate_to` because it has to be asked of the
screen being left before `self.screen` moves. It closes the device and ends a run, whose clock
would otherwise go on spending a duration behind a screen nobody is looking at.

### Subscriptions

The keyboard subscription emits the raw key with nothing captured; see README `## Layout` for
why translation moved into `update`. It reads `modified_key`, not `key`: iced reports `key`
without modifiers, so Shift+/ would arrive as `/`, and `modified_key` is also what makes
Shift+R read as unbound `R` rather than a reroll. The test helper `press` takes the modified
character for the same reason — `?` once shipped broken behind a test that pressed `"/"` with
SHIFT, an event no keyboard sends, and an earlier Shift+h test passed only because the guard
rejected every modifier.

The listener lives on `App` rather than in a subscription because iced hashes a subscription
recipe rather than its captured values, and a stream owned there would be dropped and rebuilt
whenever that hash changed.

- The flash timer exists only while a flash is on screen. Starting it with the flash puts the
  first tick a full second after the answer; an always-on timer would land anywhere from zero
  to a second later, which a fixed pause must not do. It also keeps the app idle otherwise.
- The flash lasts long enough to read as an answer being marked rather than the screen
  flickering, and short enough that a learner on a run never waits on it.
- The run clock ticks at one second, the resolution the `4:59` readout needs, and is gated on a
  run rather than the screen. `App::wants_run_clock` is split out because a `Subscription`
  cannot be inspected.
- The audio poll runs at 10 ms, a little under the detector's hop, so a window is picked up the
  tick after it completes. With no device the stream never opens and the poll does not tick, so
  the screen says "no input" rather than appearing to listen.

### Shared selector widgets

No screen has a root grid any more: the Scale Trainer, Walk the Scale and the CAGED window
each set their root on the glyph in `root_stage.rs`, which names it the way the screen's scale
or triad spells it, so pitch class 10 still reads B♭ under Ionian and A♯ under Minor
Pentatonic. `root_stage::kind_card` takes its message and focus target as parameters because
the Scale Trainer and Walk the Scale each own a scale.

`interval_token` and the marker-text function are pure decisions split out of their widgets so
they can be tested without a widget tree. The glyph and digit render in different fonts, so the
formula card still needs two `text` widgets per token. The accidental glyph returns `Option`
because a natural sign is wrong in a note label and in a formula alike; the four glyph constants
are hand-written, so a swapped pair would otherwise compile and pass.

### Card heights

iced lays a `Shrink` child out against the space left, so a column that runs out gives its last
child a height of zero and the content silently stops drawing. The cards set level with the
root — the kind card and the CAGED shape card — are therefore budgeted against
`ROOT_BAND_HEIGHT` by fit tests that use the same named text sizes the cards draw with. The budget restates iced's `LineHeight::default()`, `Relative(1.3)`,
because iced resolves it internally and the test works in laid-out pixels. A pill is its text,
`.padding([8, 12])` and the 3px focus ring. Harmonic Minor and the two pentatonics are wider
than the old summary card at its name size, which is why the Scale Trainer's summary card
went before its root grid did: the kind card already says which kind is chosen.

## Focus

The focus functions are methods on `App` rather than free functions of `&Screen`, because a
screen's shape depends on state: the Note Trainer's answer surface depends on which way the
drill runs, and only the prompt knows. An extra parameter would work today and need another
the next time. The same holds for `App::accelerators`, where a chord screen's `i` names the next
notation pill.

Side-by-side cards would share grid rows, and moving vertically searches leftwards only, so a
shorter row clamps to its own last widget. No screen has two side-by-side cards since the
Scale Trainer's root card went; its grid is now Back, the kinds, and the controls under them.

`card_bands` splits the Tab order by card, so Tab finishes one card before starting the next
instead of hopping between side-by-side cards every few widgets. Single-card screens use one
band; Walk the Scale joined them when its root card went, because two bands split its kind rows
down the middle, and the Scale Trainer followed for the same reason.

### Back on its own row

Back has a row of its own on every screen, which is where it is drawn, in the top bar above the
cards. The cost is that Right from Back walks nowhere; Tab and Down still reach what is below.

### Grid shapes

- `KIND_ROW_WIDTHS` has seven rows, ending in pairs, so the selector column the Scale Trainer
  and Walk the Scale share can be 390 wide; see `drills.md`. It replaced a six-row shape
  (3-3-2-2-3-3) sized for the Scale Trainer's old 440px kind card, where
  `Voodoo | Spanish Gypsy | Whole Tone | Diminished` measured 456px and was cut off.
- The kind rows are ragged, so their widths are spelled out. Both views and the focus grid
  read them.
- The Interval Trainer's answers are three columns, sized for the selector column; see
  `drills.md`. Its controls sit in a row under them, as on the scale screens.
- The Note Trainer's answer grid is four columns: four divides the twelve pitch classes. The
  header row above it is five while pressing and four while playing, where direction is not
  drawn; a vertical move clamps to the nearer row's last widget as on any ragged grid. While
  playing a third row holds the input device, as on Walk the Scale.
- The CAGED window's five qualities are one row, above the shapes; the shapes are one per row,
  as many as the cycle holds, which may be fewer than five; the two controls sit in a row under
  them. Up from the first shape lands on the first quality.
- Walk the Scale's last row holds only the controls the run currently shows, so Tab never stops
  on nothing. No root screen has a root row: the root is set on the glyph.
- Rows are built by zipping rather than indexing, so a narrower `ROOT_ROW_WIDTH` drops buttons
  instead of panicking and a wider one leaves cells empty.

### What is not a stop

- A neck is one cell however many positions it holds; the cursor walks them. On Play the
  Interval the neck takes no press, so it is not a stop at all, and Walk the Scale's neck is a
  stop only while a press means something on it.
- The library's voicing strip is browsed from the chord list. A third stop would make a voicing
  reachable only after tabbing past every chord. The notation buttons sit beside the search box
  rather than under the list, since anything under it would be reachable by Tab alone.
- The device list is not in the ring. While it is up it claims the motion keys, before any other
  claim.

### Widgets that claim motion keys

The necks are a deliberate special case rather than a mechanism: seventy-eight positions reached
by anything other than the arrows would cost more than Tab being the only way out. The library's
list and the CAGED pills joined later on the same terms, so walking the list or the cycle is one
gesture rather than focus-then-activate. The pills are a column, so they claim up and down, and
only while the cycle goes further: at either end the arrow moves the ring as anywhere else, to
Back above or the controls below, which a claim at the ends would have made Tab-only.
Left and right pick the quality outright, from the quality row or from a shape, so stepping
through qualities while watching one shape never means climbing back to the row. From a shape
the ring stays on the selection; the column has nothing beside it, so claiming them costs no
stop. The qualities stop at either end rather than wrapping, as the cycle does.

The Home menu claims all four, for a different reason. It is two columns — the categories, and
beside them the entries of whichever category is open — and the open one is simply the one the
ring is on, or the one holding the entry it is on, so the menu keeps no state of its own. Grid
geometry would get the second column wrong: the cell beside an entry is some other category,
and Left onto it would swap out the column just left. So `home::step` crosses between a
category and its own entries, and up and down stay in a column. The grid is still built, as
drawn, for what the grid is otherwise for: membership, and snapping a stale target.

## The Home menu

Nine screens in one list was too much to take in at once, so they are grouped by subject —
scales, notes and intervals, chords, and the tools that only listen — with each group's
entries opened beside it. Subject rather than reference-versus-drill, since a learner arrives
wanting to work on scales, not wanting a drill of some kind.

The column opens on focus rather than on `Enter`, so browsing the categories with the arrows
shows what is in each without a press, and Right never lands in a column belonging to another
category. `Enter`, a click and a digit all step into the category instead. Digits number
whichever column the ring is in, so two of them reach any screen; the cost is that a screen no
longer has one digit of its own. `Esc` on an entry steps back out to its category, as Left
does, and coming back from a screen lands on the entry that opened it. Tab visits each category
followed by its entries, so it walks the whole tree rather than every category before any
entry.

## Keyboard

`translate_key` cannot tell whether a character is bound, because binding is per screen and it
has no screen; it forwards every single character and `update` resolves it, ignoring misses. A
binding's absence is therefore only observable as no change in state, which is why the tests'
snapshot struct includes `notation` — without it, `i` doing nothing would pass. `press` still
probes `translate_key` directly, since "no message" is a stronger assertion than "nothing
changed".

The `?` arm sits before the general character arm, whose guard would otherwise swallow it
without the compiler warning about shadowing. It lives there rather than in a screen's table
because it has to work on screens that declare no accelerators. `Ctrl+K` is the mirror case: the
one character admitted under a command modifier, with `k` named so the exception cannot widen to
the whole key.

`keys::Fires` has two cases, a widget and a message, because a widget target is what lets
the table be checked against the focus grid; the check walks every notation, since the
library's `i` target moves with the setting. The label is a `Cow<'static, str>`: a `String`
would cost every fixed label an allocation, and a `&'a str` borrowed from `App` cannot point at
a `format!` result owned by the returning function (`E0515`).

`gg` is taken by `accelerate` before the table, because a table entry is one key and has nowhere
to keep a pending `g`. Its table row exists for the overlay only. Anything between the two
halves cancels it, so a stale `g` cannot turn the next keystroke into a jump.

The library and the CAGED window have one key per notation mode as well as cycling `i`: with
all three visible, a learner who can see `fingers` should reach it directly, and `i` stays
because that is what it means on the Scale Trainer. `n`, `d` and `f` are note names, so the focused search box takes
them as text and the mode keys are reached after `Esc`.

An accelerator fires a widget without walking the ring onto it. An action that stays on screen
leaves focus where it was; one that navigates moves focus because navigation does.

`NAVIGATION_KEYS` is written out rather than derived from `translate_key`: those keys are
structural and will not move, unlike the per-screen accelerators, which grow with every feature
and so are read from the table that dispatches them. A keycap outside the ranges `just fonts`
passes to `--unicodes` falls back to a system font and shows up as a glyph at the wrong weight
rather than as a failure; nothing at runtime can read the font, so a test checks the table
against the ranges written out.

The help card sits on `CANVAS_SOFT_2`, the tone for surfaces that float above cards, since
`CANVAS_SOFT` would match the cards behind and leave the scrim doing all the separating. It has
one drop shadow because `container::Style` holds one `Shadow`; a three-layer modal shadow is not
expressible in iced. The scrim is opaque enough to push the screen back without hiding it, so the
overlay reads as covering the page rather than replacing it.

## The listener

`ui::listener` sits in `ui/` rather than `audio/` because it holds a highlighted row and draws a
list; a picker index is on the screen side of the line that keeps the detector testable.

- `Listening` has no `Debug` because `cpal::Stream` has none, and nothing needs one enough to
  hand-write it.
- The confirming tracker is per session: a run of agreeing windows means nothing after the
  screen has been left and re-entered.
- `Heard::Silence` at rest makes silence a state the screen draws rather than the absence of one.
  The four `State`s exist so a learner can tell a broken setup from an idle one without leaving
  the screen.
- The device list is read when the picker opens, not at startup, so it matches the machine.
- `open` is idempotent and leaves a working device alone. `fell_back` is set in one place so a
  device opened from memory and one opened from the list cannot disagree about it.
- The chosen device is remembered for the app, not the screen: which interface a learner plays
  through is a fact about the instrument. A device unplugged between the list being read and the
  row being chosen opens the default, and that is not remembered.
- `close` runs on leaving. Holding the stream for a quick re-entry buys nothing, since the poll is
  gated on a listening screen being shown, and an audio interface is a device other programs
  want. Dropping the `Input` is what stops the stream.
- Analysis runs on the UI thread in `poll`, not in the audio callback: the callback's budget is a
  deadline, and a detector that runs long there is a glitch in the system's audio rather than a
  late frame.
- `poll` returns what it released so a drill can fold and judge in one call. Reading it back off
  the device afterwards would make two readers of one reading.
- `hz` is separate from `reading` because one is the measurement and the other the
  interpretation: a string being tuned up moves the hertz continuously while the name stays put.
- The device list is modal and the screen behind it is not drawn.
- `fitting` is opt-in, off by default: the inharmonicity fit is the slow path, and only the
  screens that place notes read it. `poll_fitted` returns the fit beside the reading it belongs to so a
  screen never holds one without the other; `poll` drops it, so the other screens did not change.
- When a fit is due is `fit_due`, a pure function of the release and the hops since the note was
  confirmed: on confirmation, then every `FIT_EVERY` (8) hops, about 85 ms, half a fit window, so
  consecutive fits overlap rather than reading the same audio twice. A new note re-confirms
  through the tracker's unpitched hops, which resets the count, so it is fitted on its own
  confirmation.

## Placing a note

- `placement::Placer` follows one sounding note and places it once; the Position Finder, Play the
  Interval, Walk the Scale and the Note Trainer each hold one, so they place by one rule. It was
  the Position Finder's own until the drills placed notes too, and was moved rather than copied.
  It sits in `ui/` beside `position.rs`, the pure half it calls, because it needs `Pitch`,
  `Calibration` and `Neck`, none of which `audio/` may import.
- One placement per note. A note gathers fits until `FITS_PER_NOTE` (3) are accepted and is then
  placed once from their median; later fits of the same note change nothing. The first window
  can still hold the attack, and a screen that lights one place and then moves it is the thing the
  spec rules out — showing a pitch and then revising where it came from. A note that ends with
  fewer fits is placed from what it has, one whose every fit was refused is declined as
  unreadable, and one that ended before any fit was attempted is not placed at all.
- A slide and a missing calibration decide the placement without a fit, so those notes are
  placed on their first reading.
- The result is `Placement::Placed` or `Placement::Declined` with its reason, an enum rather than
  an estimate beside an optional reason, which could say a note was both named and slid into.
  `Placed` carries the median coefficient so the Position Finder can judge drift on it; drift
  stays out of the placer so the drills carry no state they never read.
- `Placer::hear` returns what it placed during that reading as an iterator rather than holding
  a "last placement" to be read: one reading can end a note and start one that is placed on
  arrival, and reading only the last would lose the first. It is `+ use<>` so the iterator holds
  no borrow of the placer, and the caller can ask it whether the sounding note is placed mid-loop.

## The Position Finder

- A reading with no placement yet leaves the previous answer up rather than showing a bare
  pitch; the answer reads as live while the note it places is still sounding.
- The calibration walk takes the same median of three for each of its twelve readings, and asks
  again — rather than skipping — when a note ends without them. A harmonic reading counts as
  refused there, since a calibration needs a logarithm. The ringing tail of the reading just
  recorded is ignored rather than reported as the wrong note for the next step.
- Groups are linked through neighbours within `MARGIN`, not measured from the best match alone.
  With three candidates where A–B and B–C are each inside the margin but A–C is not, measuring
  from the best gives a different group depending on which one the note landed nearest, and no
  list made before playing could match that. Linking makes the groups a partition, so
  `position::groups` lists exactly what `estimate` will answer — a test checks every listed group
  against the estimate at its midpoint.
- A flagged string's positions join every group at their pitch, because a prediction that did not
  follow the model cannot rule a candidate out; one is never named alone, even as the only
  candidate.
- `FIT_GATE` and `MARGIN` are both 0.4 in log₂ B, four times run 3's widest take-to-take
  spread. Checked on the spike's Stratocaster with the pluck spot varied between takes, they
  held: the E4 trio named alone every time, A3 at D@7 and G@2 answered as the predicted group,
  A@12 named alone, and no take left unnamed — so neither moved. `RISE_TOLERANCE` started at 0.5 and is 0.8: on the
  spike's Stratocaster the plain B string rises 2.59 from open to twelfth fret, and the spike's own
  B@5 sat 0.31 above the model's slope, so the steep rise is the string rather than a bad reading.
  The drift constants start at 0.3 and have not been measured against a guitar yet.
- Drift is judged on one miss per answered note — measured minus the nearest prediction, signed —
  over the last five. Five misses on one side, at least 0.3 out and within 0.3 of each other, read
  as the strings having moved together; scattered misses read as playing.
- `calibration::kept_path` is `None` under test, so no test reads the learner's calibration or
  replaces it on completing a walk. Persistence is tested through `read` and `write` with a
  temporary path, as for the remembered device.
- Calibration is a mode, `c`, rather than a screen: a menu entry of its own would be one
  more thing on Home for something done once per string change, and the only screen that reads a
  calibration is the one that captures it.

## Style

- `style::ROOT_MARKER` marks the Interval Trainer's tonal center because every other colour on that
  neck is taken: `LINK` is the prompt, `DANGER` and `SUCCESS` the answers, white the cursor ring.
  Amber reads as warm against all four.
- The semantic colours are restated as constants because canvas markers and button styles are
  built where no `&Theme` is in hand.
- `ghost_button` rounds to a full pill, so the label decides the shape: a single glyph draws a
  circle beside its neighbours' lozenges, and a `MUSIC_FONT` label stands shorter because SMuFL
  metrics are nothing like a text face's. `control_label` pins the line box and `control_glyph`
  floors the width.
- A lone accidental sits high because SMuFL draws against a staff, and a flat, which hangs its
  bowl under its staff line, starts higher than a sharp and falls further. Fixing the container
  height is what makes top padding move the glyph rather than the pill.
- The back arrow is larger than the body's 18; at 18 the maths face drew it as a rule with a nick.
- The shuffle mark is drawn because U+1F500 is a colour emoji on most systems and absent on the
  rest. It is stroked in `INK` because `ghost_button` sets text to `INK` in every status. Its box
  is wider than tall so the flat tails separate it from a ✗, and shorter than
  `CONTROL_LINE_HEIGHT` so it reads as a mark rather than art. It is laid out in fractions of a
  15px box, since rounding to whole pixels made it lopsided, and the head is longer than wide
  after a first pass read as a spike. A canvas has no line box, so the pill wraps it in one.
- The live streak is drawn in the success colour: the flash says one answer was right and goes,
  and a standing streak keeps saying it.
- List rows are square and borderless because a column of pills reads as buttons. The picked row
  is lifted rather than filled, since the chord symbol is already the loudest thing in it.
- A wrong answer is filled rather than outlined, so the buttons still to try are the empty ones.
  `correct_answer_button` is `selected_root_button` to the pixel but kept separate, since a
  selection and a verdict have no reason to move together.

## The neck

- `neck::Neck` carries its fret count because the screens disagree on it, and `place(.., 12)`
  versus `place(.., 15)` would compile either way. The tuning travels with it because
  `pitch_class_at` needs both.
- `DRILL_NECK` has twelve frets. `CAGED_NECK` has fifteen: twelve truncates the cycle, with eight
  of twelve roots losing a shape and C major losing its D shape at the thirteenth fret. Fourteen
  is the shortest that holds every shape, and fifteen ends on an inlay (3-5-7-9-12-15).
- The tuning's const constructor panics at compile time, so a typo is a build failure rather
  than a runtime `None`.
- `pitch_at` returns `Option` rather than panicking: the callers are a click and a cursor, and a
  bounds bug should misjudge one answer rather than end the process.
- `Position` has named fields because the Interval Trainer's *Name it* prompt holds two, and a
  swapped root and target would invert every interval on screen with nothing failing to compile.
  The Note Trainer's single-position prompts still use a pair. `Position::pitch_class` is bound
  to `DRILL_NECK` because only the drills hold one; a neck parameter would thread through a dozen
  call sites to vary nothing.
- The cursor stops at the neck's edges, since wrapping from high E to low E looks like a jump.
- Placed markers carry their pitch class, so a position that yielded `None` never enters the list
  and the drawing code needs no `expect`.
- `Message::ChooseNotePosition` and its siblings take two loose `usize`s so the bare variant
  constructor fits `Fretboard::on_press`'s `fn(usize, usize) -> Message`.
- `Neck::positions_of` filters every position through `pitch_at` rather than computing frets
  from the pitch, so the candidates for a pitch and the pitch at a position cannot disagree.
