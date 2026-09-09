//! The Chord Library: looking a chord up, and drawing the ways to play it.
//!
//! The shapes those ways come from live in [`super::shapes`], not here. They are
//! instrument knowledge rather than one screen's, and two screens place them.

use iced::keyboard;
use iced::widget::canvas;

use crate::music::chords::{Chord, ChordQuality, Query};
use crate::music::notes::{PitchClass, Spelling};

use super::chord_diagram::{ChordDiagram, FEATURE, STRIP, StringMark, chord_diagram};
use super::shapes::{Voicing, position_label, voicings};
use super::{
    BODY, DRILL_NECK, FocusTarget, HAIRLINE_INK, INK, MUSIC_FONT, MUTE, Message, Notation,
    SMUFL_CSYM_AUGMENTED, SMUFL_CSYM_DIMINISHED, SMUFL_CSYM_HALF_DIMINISHED,
    SMUFL_CSYM_MAJOR_SEVENTH, SMUFL_SHARP, SUCCESS, card_container, focus_ring, ghost_button,
    hairline_rule, intervalic_text, note_label,
};

/// The library's state: what has been typed, which root it settled on, and what is picked.
///
/// Its own struct rather than five more fields on `App`, so they can stay private — they
/// are read all over the views below and nowhere else. The arrangement both trainers use.
/// The three notations the library can draw, in the order `i` walks them.
///
/// Its own list rather than one shared with the Scale Trainer: a scale has no fingering, so
/// the two screens have different vocabularies and no reason to move together.
pub(super) const NOTATIONS: [Notation; 3] =
    [Notation::Notes, Notation::Intervals, Notation::Fingers];

pub(super) struct ChordLibrary {
    query: String,
    /// Where the caret sits, as a character index into `query`.
    ///
    /// The box is drawn and edited here rather than by iced's `text_input`, because that
    /// widget would receive keys *as well as* the global subscription — both would fire,
    /// and `l` would insert a character and move the focus ring. One owner, one caret.
    caret: usize,
    selected_row: usize,
    selected_voicing: usize,
    search_focused: bool,
    /// Whether a `g` is waiting for its second half.
    ///
    /// `gg` is the one two-key gesture in the app. Anything other than a second `g` clears
    /// it, so a half-typed motion never lies in wait to change what the next key does.
    pending_g: bool,
    /// What the diagrams' marks say, kept here rather than on `App`.
    ///
    /// Held by the screen because only this screen can draw all three, and because sharing
    /// one field with the Scale Trainer meant picking `fingers` here quietly changed what
    /// that screen labelled its markers with.
    notation: Notation,
}

impl ChordLibrary {
    pub(super) fn new() -> Self {
        Self {
            query: String::new(),
            caret: 0,
            selected_row: 0,
            selected_voicing: 0,
            search_focused: false,
            pending_g: false,
            // Fingers by default: the question this screen is opened to answer is "how do
            // I play this", and a fingering answers it in a way a note name does not.
            notation: Notation::Fingers,
        }
    }

    pub(super) fn notation(&self) -> Notation {
        self.notation
    }

    pub(super) fn set_notation(&mut self, index: usize) {
        if let Some(&notation) = NOTATIONS.get(index) {
            self.notation = notation;
        }
    }

    /// Opening the screen: an empty box with the caret in it, and the notation left as the
    /// learner last set it. The query goes because the first keystroke should start a new
    /// search rather than extend the last one.
    pub(super) fn enter(&mut self) {
        self.set_query(String::new());
        self.focus_search();
    }

    pub(super) fn query(&self) -> &str {
        &self.query
    }

    pub(super) fn search_focused(&self) -> bool {
        self.search_focused
    }

    /// The qualities surviving the query, in the order they are listed.
    ///
    /// An empty query is every quality on the current root — browsing is the zero-input
    /// case of searching rather than a mode of its own. A query that parses picks the
    /// chord it names and the ones extending it; one that does not falls back to
    /// approximate matching, which is the only place a score is involved and never
    /// competes with an exact hit.
    pub(super) fn rows(&self) -> Vec<Chord> {
        match Query::parse(&self.query) {
            None if self.query.trim().is_empty() => every_chord().collect(),
            None => self.approximate_rows(),
            Some(Query::Root(root)) => every_chord().filter(|chord| chord.root() == root).collect(),
            // `covers` rather than `==`: a named quality is where the learner has typed to,
            // not where they are going, so `Cmaj` must not hide the `Cmaj7` it starts.
            Some(Query::Quality(kind)) => every_chord()
                .filter(|chord| kind.covers(chord.kind()))
                .collect(),
            Some(Query::Chord { root, kind }) => every_chord()
                .filter(|chord| chord.root() == root && kind.covers(chord.kind()))
                .collect(),
        }
    }

    /// Rows for a query the grammar could not read: the chords whose symbol contains the
    /// query's characters in order, nearest match first.
    ///
    /// A subsequence rather than a substring, so `cmj7` still reaches `Cmaj7`. The hazard
    /// this would otherwise carry — `cm7` scoring against `Cmaj7` — cannot arise, because
    /// `cm7` parses and never reaches here.
    fn approximate_rows(&self) -> Vec<Chord> {
        let needle = self.query.trim().to_lowercase();
        // The library's own order is the last tiebreak, so two equally good matches come
        // out in the order the list would have shown them anyway.
        let mut scored: Vec<(usize, usize, usize, Chord)> = every_chord()
            .enumerate()
            .filter_map(|(order, chord)| {
                let (start, span) = subsequence(&needle, &chord.to_string().to_lowercase())?;

                Some((start, span, order, chord))
            })
            .collect();

        scored.sort_unstable_by_key(|&(start, span, order, _)| (start, span, order));
        scored.into_iter().map(|(.., chord)| chord).collect()
    }

    pub(super) fn selected_row(&self) -> usize {
        self.selected_row
    }

    /// Scrolls the list so the selected row is on screen.
    ///
    /// Proportional: the offset is the selection's share of the rows, so the highlight
    /// starts at the top of the viewport for the first row and reaches the bottom for the
    /// last, staying inside it all the way between. Exact at both ends, and near enough in
    /// the middle that the group headers' extra height does not push it out — which is what
    /// spares this from having to know the viewport's size or a row's height in pixels.
    pub(super) fn follow_selection(&self) -> iced::Task<Message> {
        let rows = self.rows().len();
        let y = if rows > 1 {
            self.selected_row as f32 / (rows - 1) as f32
        } else {
            0.0
        };

        iced::widget::operation::snap_to(
            list_id(),
            iced::widget::operation::RelativeOffset { x: 0.0, y },
        )
    }

    pub(super) fn selected_chord(&self) -> Option<Chord> {
        self.rows().get(self.selected_row).copied()
    }

    pub(super) fn selected_voicing(&self) -> usize {
        self.selected_voicing
    }

    /// Replaces the query. Nothing is remembered from it: the list is the whole library
    /// filtered by whatever is typed, so an empty box means the whole library rather than
    /// "the last root you named", which was state the screen never showed anybody.
    fn set_query(&mut self, query: String) {
        self.query = query;
        self.selected_row = 0;
        self.selected_voicing = 0;
    }

    pub(super) fn select_row(&mut self, index: usize) {
        if index < self.rows().len() {
            self.selected_row = index;
            self.selected_voicing = 0;
        }
    }

    pub(super) fn select_voicing(&mut self, index: usize) {
        let count = self
            .selected_chord()
            .map_or(0, |chord| voicings(chord, &DRILL_NECK).len());

        if index < count {
            self.selected_voicing = index;
        }
    }

    /// Walks the list, stopping at the ends rather than wrapping — the rule the focus grid
    /// already follows for a row of unequal width.
    pub(super) fn move_row(&mut self, delta: isize) {
        let count = self.rows().len();
        if count == 0 {
            return;
        }

        let next = (self.selected_row as isize + delta).clamp(0, count as isize - 1);
        self.select_row(next as usize);
    }

    pub(super) fn move_voicing(&mut self, delta: isize) {
        let count = self
            .selected_chord()
            .map_or(0, |chord| voicings(chord, &DRILL_NECK).len());
        if count == 0 {
            return;
        }

        let next = (self.selected_voicing as isize + delta).clamp(0, count as isize - 1);
        self.selected_voicing = next as usize;
    }

    /// The vim motions this screen claims for itself: `gg` to the first chord, `G` to the
    /// last. Reports whether it took the key.
    ///
    /// Only reached with the search box unfocused, since a focused box takes every character
    /// as text — which it must, because `g` is a note name.
    pub(super) fn motion(&mut self, c: char) -> bool {
        let pending = std::mem::replace(&mut self.pending_g, false);

        match c {
            'g' if pending => self.select_row(0),
            'g' => {
                self.pending_g = true;

                return true;
            }
            'G' => {
                let last = self.rows().len().saturating_sub(1);
                self.select_row(last);
            }
            _ => return false,
        }

        true
    }

    pub(super) fn focus_search(&mut self) {
        self.search_focused = true;
        self.caret = self.query.chars().count();
        self.pending_g = false;
    }

    /// Handles a key while the box has focus, reporting what it did.
    ///
    /// Almost everything is text here: a character key types rather than acting, which is
    /// why `j` does not move and `?` does not open the help. The exceptions are the picker
    /// set — the arrows, `Enter` and `Esc` — and they are what make typing and choosing one
    /// gesture rather than two.
    pub(super) fn handle_key(
        &mut self,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> KeyOutcome {
        use keyboard::key::Named;

        match key.as_ref() {
            // Leaves the box without clearing it: the query is how the learner got here,
            // and the gesture is "I found it, now let me look at the shapes".
            keyboard::Key::Named(Named::Escape) => {
                self.search_focused = false;

                return KeyOutcome::Handled;
            }
            // Accepting: the row is already selected, so this is the learner saying they
            // have found it. The ring moves on to the shapes.
            keyboard::Key::Named(Named::Enter) => {
                self.search_focused = false;

                return KeyOutcome::Accepted;
            }
            // The one pair that still navigates while typing, so a learner never has to
            // leave the box to pick from what they have narrowed to.
            keyboard::Key::Named(Named::ArrowUp) => self.move_row(-1),
            keyboard::Key::Named(Named::ArrowDown) => self.move_row(1),
            keyboard::Key::Named(Named::ArrowLeft) => self.caret = self.caret.saturating_sub(1),
            keyboard::Key::Named(Named::ArrowRight) => {
                self.caret = (self.caret + 1).min(self.query.chars().count());
            }
            keyboard::Key::Named(Named::Backspace) => {
                if let Some(at) = self.caret.checked_sub(1) {
                    let mut next: Vec<char> = self.query.chars().collect();
                    next.remove(at);
                    self.set_query(next.into_iter().collect());
                    self.caret = at;
                }
            }
            keyboard::Key::Character(text) if !modifiers.intersects(super::COMMAND_MODIFIERS) => {
                let mut next: Vec<char> = self.query.chars().collect();
                let at = self.caret.min(next.len());

                for (offset, c) in text.chars().enumerate() {
                    next.insert(at + offset, c);
                }

                let typed = text.chars().count();
                self.set_query(next.into_iter().collect());
                self.caret = at + typed;
            }
            // Anything else is not this box's business — a command-modified key, say, so
            // `Ctrl+K` from inside the box still reaches the app.
            _ => return KeyOutcome::Ignored,
        }

        KeyOutcome::Handled
    }
}

/// What a keystroke in the search box did, which is more than "was it mine".
///
/// `Accepted` is its own case because `Enter` and `Esc` mean different things: leaving the
/// box with `Esc` is backing out, and leaves the ring where it was, while `Enter` is
/// finishing — the learner has found their chord and wants to look through its shapes, so
/// the ring goes to the list, whose arrows walk exactly that.
///
/// Returned rather than reaching for `App`'s focus from in here: this struct knows what its
/// own key did, and where the ring sits is the parent's business.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyOutcome {
    /// Not this box's key — the app should go on and translate it.
    Ignored,
    /// Claimed, and the ring stays where it is.
    Handled,
    /// Claimed, and the box is finished: move the ring to the shapes.
    Accepted,
}

/// The chord list's scrollable, named so `follow_selection` can reach it.
fn list_id() -> iced::widget::Id {
    iced::widget::Id::new("chord-library-list")
}

/// Every chord the library holds, by root and then by quality.
///
/// The whole cross product, because the list is a filter over it rather than a window onto
/// one root. Building all 180 costs about 33µs — the rows are cheap; it is the widgets that
/// are not, which is why the view groups them rather than showing a flat wall.
fn every_chord() -> impl Iterator<Item = Chord> {
    PitchClass::ALL.into_iter().flat_map(|root| {
        ChordQuality::ALL
            .iter()
            .map(move |&kind| Chord::new(root, kind))
    })
}

/// Where `needle`'s characters appear in `haystack`, in order, as `(start, span)`.
///
/// `None` when they do not all appear. The pair is what ranks a match: an earlier start
/// beats a later one, and a tighter span beats a scattered one, so `maj7` sits above
/// `mMaj7` for the query `maj7`.
fn subsequence(needle: &str, haystack: &str) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return Some((0, 0));
    }

    let hay: Vec<char> = haystack.chars().collect();
    let mut start = None;
    let mut at = 0;

    for wanted in needle.chars() {
        let found = hay[at..].iter().position(|&c| c == wanted)? + at;

        start.get_or_insert(found);
        at = found + 1;
    }

    let start = start?;

    Some((start, at - start))
}

/// One piece of a written chord symbol: body-font text, or a SMuFL glyph.
///
/// Two fonts, so one `text` widget cannot carry both — the split `note_label` and
/// `intervalic_text` already make for accidentals, applied one level up to the quality.
enum SymbolPart {
    Text(&'static str),
    Glyph(char),
    /// A glyph set as a superscript rather than on the baseline.
    ///
    /// Only the diminished and half-diminished circles: engraving convention raises those
    /// two and leaves every other quality mark sitting on the baseline beside the root.
    /// Raising them all — which is what this did at first — makes `Cm` and `Csus4` look
    /// like exponents.
    Raised(char),
}

/// How a quality is written after its root.
///
/// The four with a glyph of their own use it; the rest are their abbreviation. This is the
/// typographic counterpart of `ChordQuality::suffix`, which is ASCII because `music/`
/// cannot reach these constants.
fn symbol_parts(kind: ChordQuality) -> &'static [SymbolPart] {
    use SymbolPart::{Glyph, Raised, Text};

    match kind {
        // A plain major triad is written as its root alone.
        ChordQuality::Major => &[],
        ChordQuality::Minor => &[Text("m")],
        ChordQuality::Diminished => &[Raised(SMUFL_CSYM_DIMINISHED)],
        ChordQuality::Augmented => &[Glyph(SMUFL_CSYM_AUGMENTED)],
        ChordQuality::Sus2 => &[Text("sus2")],
        ChordQuality::Sus4 => &[Text("sus4")],
        ChordQuality::Major6 => &[Text("6")],
        ChordQuality::Minor6 => &[Text("m6")],
        ChordQuality::Dominant7 => &[Text("7")],
        ChordQuality::Major7 => &[Glyph(SMUFL_CSYM_MAJOR_SEVENTH)],
        ChordQuality::Minor7 => &[Text("m7")],
        ChordQuality::MinorMajor7 => &[Text("m"), Glyph(SMUFL_CSYM_MAJOR_SEVENTH)],
        ChordQuality::HalfDiminished7 => &[Raised(SMUFL_CSYM_HALF_DIMINISHED)],
        // The circle is raised; the seventh after it is not.
        ChordQuality::Diminished7 => &[Raised(SMUFL_CSYM_DIMINISHED), Text("7")],
        // `7♯5`, not `+7`. Both are used, but `+7` is read by some as "with an augmented
        // seventh" — a major seventh — which is a different chord. `7♯5` says the same
        // thing as the degree row beneath it and cannot be read two ways.
        ChordQuality::Augmented7 => &[Text("7"), Glyph(SMUFL_SHARP), Text("5")],
    }
}

/// A chord written the way a musician writes it: the root with a real accidental, the
/// quality with its own glyph where it has one.
fn chord_symbol(
    chord: Chord,
    size: impl Into<iced::Pixels>,
    color: iced::Color,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::{container, text};

    let size = size.into();
    // The quality is set smaller than the root, the way chord symbols are engraved. At the
    // root's own size the diminished circle comes out at cap height and `C°` reads as `CO`.
    let quality = iced::Pixels(size.0 * 0.66);
    // A superscript is smaller again than the baseline marks, not merely lifted off them.
    // At the quality's own size the circle comes out as big as the `m` in `Cm`, which is
    // the size of a letter rather than of a mark about one.
    let raised = iced::Pixels(size.0 * 0.42);

    // The row bottom-aligns its children, which lines up the *boxes* rather than the
    // baselines — and a smaller box carries proportionally less descender under its text,
    // so a mark would hang below the root's baseline like a subscript. Lifting each by its
    // own shortfall in descender space is what puts them all on one line. iced's default
    // line height is 1.3, of which roughly 0.3 sits under the baseline.
    let sits_on_baseline = |mark: iced::Pixels| (size.0 - mark.0) * 0.3;
    // The superscript then rises a percentage of its own height above that shared
    // baseline, which brings its top up near the root's cap — where a chart puts it. Half
    // that left it sitting around the root's middle, reading as a mark that had slipped.
    let lift = sits_on_baseline(raised) + raised.0 * 0.85;

    symbol_parts(chord.kind())
        .iter()
        .fold(note_label(chord.root_note(), size, color), |row, part| {
            let (content, music, mark_size, bottom) = match part {
                SymbolPart::Text(written) => (
                    (*written).to_string(),
                    false,
                    quality,
                    sits_on_baseline(quality),
                ),
                SymbolPart::Glyph(glyph) => {
                    (glyph.to_string(), true, quality, sits_on_baseline(quality))
                }
                SymbolPart::Raised(glyph) => (glyph.to_string(), true, raised, lift),
            };

            let mark = text(content).size(mark_size).color(color);
            let mark = if music { mark.font(MUSIC_FONT) } else { mark };

            row.push(container(mark).padding(iced::Padding {
                bottom,
                ..iced::Padding::ZERO
            }))
        })
        // Bottom-aligned so a smaller mark sits beside the root rather than over it; the
        // paddings above are what turn "same bottom edge" into "same baseline".
        .align_y(iced::Alignment::End)
}

/// What one string's mark says, under the notation in force.
fn mark_label(
    chord: Chord,
    voicing: Voicing,
    notation: Notation,
    string: usize,
    fret: u8,
) -> String {
    let Some(sounding) = DRILL_NECK.pitch_class_at(string, usize::from(fret)) else {
        return String::new();
    };

    // `spell` and `degree` rather than a search through `notes()` here: what a pitch class
    // is called in a chord, and what job it does, are both questions for `music/`. This
    // module decides which of the two answers to draw and nothing else.
    match notation {
        Notation::Notes => chord
            .spell(sounding)
            .map(|note| note.to_string())
            .unwrap_or_default(),
        Notation::Intervals => chord
            .degree(sounding)
            .map(|interval| interval.to_string())
            .unwrap_or_default(),
        // An open string is stopped by nobody, so it carries no finger.
        Notation::Fingers => voicing.fingers()[string]
            .map(|finger| finger.to_string())
            .unwrap_or_default(),
    }
}

fn diagram_for(chord: Chord, voicing: Voicing, notation: Notation) -> ChordDiagram<Message> {
    let frets = voicing.strings();

    ChordDiagram {
        strings: std::array::from_fn(|string| {
            let fret = frets[string];
            let sounding =
                fret.and_then(|fret| DRILL_NECK.pitch_class_at(string, usize::from(fret)));

            StringMark {
                fret,
                label: fret.map_or_else(String::new, |fret| {
                    mark_label(chord, voicing, notation, string, fret)
                }),
                is_root: sounding == Some(chord.root()),
            }
        }),
        barre: voicing.barre_fret(),
        // Display-only in this change. The widget hit-tests regardless, so the editing
        // pass has a surface to land on — see the module comment on `chord_diagram`.
        on_press: None,
    }
}

/// The rule between one root's chords and the next.
///
/// Both names on the five black keys, because the rows below genuinely use both: pitch
/// class 1 spells `D♭` under a major triad and `C♯` under a minor one, since `D♭ F A♭`
/// costs two flats where `C♯ E♯ G♯` costs three sharps. A header reading `C♯` alone would
/// be wrong about a third of its own group — and showing the pair says the thing the
/// spelling rule exists to teach, which is that the name follows the chord.
fn root_header(root: PitchClass, after_a_group: bool) -> iced::Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, container, row, text};

    let sharp = Spelling::Sharps.spell(root);
    let flat = Spelling::Flats.spell(root);

    let name = if sharp == flat {
        row![note_label(sharp, 13, MUTE)]
    } else {
        row![
            note_label(sharp, 13, MUTE),
            text("/").size(13).color(HAIRLINE_INK),
            note_label(flat, 13, MUTE),
        ]
        .spacing(5)
    };

    container(
        column![
            name,
            // A hairline rather than a bordered container: a `Border` would draw on all
            // four sides, and only the underline is wanted.
            container(Space::new().height(Length::Fixed(1.0)))
                .width(Length::Fill)
                .style(hairline_rule),
        ]
        .spacing(4),
    )
    .padding(iced::Padding {
        top: if after_a_group { 14.0 } else { 4.0 },
        bottom: 2.0,
        left: 12.0,
        right: 12.0,
    })
    .into()
}

/// The magnifier in the search box, drawn rather than set.
///
/// `⌕` is not a character a text face is expected to carry and nothing embedded here has a
/// magnifier in it, so the glyph the system found for it came out as a stray `o` sitting
/// under the baseline. Two strokes depend on nothing.
struct SearchIcon;

/// The side of the square the icon is drawn in.
const SEARCH_ICON: f32 = 14.0;

impl canvas::Program<Message> for SearchIcon {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use iced::Point;
        use iced::widget::canvas::{Frame, Path, Stroke};

        let mut frame = Frame::new(renderer, bounds.size());
        let side = bounds.width.min(bounds.height);
        let stroke = Stroke::default().with_color(MUTE).with_width(1.4);

        // The lens up in the corner and the handle out of it down the diagonal, which is the
        // whole drawing: the handle leaves the rim at 45°, so it starts a radius away on
        // both axes.
        let centre = Point::new(side * 0.4, side * 0.4);
        let radius = side * 0.3;
        let grip = radius * std::f32::consts::FRAC_1_SQRT_2;

        frame.stroke(&Path::circle(centre, radius), stroke);
        frame.stroke(
            &Path::line(
                Point::new(centre.x + grip, centre.y + grip),
                Point::new(side * 0.92, side * 0.92),
            ),
            stroke,
        );

        vec![frame.into_geometry()]
    }
}

pub(super) fn ui_chord_library(
    library: &ChordLibrary,
    focused: FocusTarget,
) -> iced::Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, button, column, container, row, scrollable, text};

    let typed = library.query();
    let box_text = if typed.is_empty() {
        text("Search a chord…").size(16).color(MUTE)
    } else {
        text(typed.to_string()).size(16).color(INK)
    };

    // The caret is a glyph rather than a blinking cursor: nothing here animates, and a
    // learner needs to see that the box has the keyboard, not where a redraw is due.
    let caret = if library.search_focused() {
        text("▏").size(16).color(SUCCESS)
    } else {
        text("/").size(16).color(MUTE)
    };

    let search_box = focus_ring(
        button(
            row![
                canvas(SearchIcon)
                    .width(Length::Fixed(SEARCH_ICON))
                    .height(Length::Fixed(SEARCH_ICON)),
                box_text,
                Space::new().width(Length::Fill),
                caret
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(ghost_button)
        .on_press(Message::OpenSearch),
        focused == FocusTarget::SearchBox,
    );

    let rows = library.rows();
    let list: iced::Element<'static, Message> = if rows.is_empty() {
        container(
            text(format!("No chord matches “{typed}”"))
                .size(15)
                .color(MUTE),
        )
        .padding(12)
        .into()
    } else {
        scrollable(
            rows.iter()
                .enumerate()
                .fold(column![].spacing(2), |list, (index, &chord)| {
                    // A header wherever the root changes. Derived here rather than stored
                    // in the rows, so the selection stays a plain index into the chords and
                    // the arrows never have to step over anything unselectable.
                    let opens_a_group = index == 0 || rows[index - 1].root() != chord.root();
                    let list = if opens_a_group {
                        list.push(root_header(chord.root(), index > 0))
                    } else {
                        list
                    };
                    let picked = index == library.selected_row();

                    list.push(
                        button(chord_symbol(chord, 17, if picked { INK } else { BODY }))
                            .padding([7, 12])
                            .width(Length::Fill)
                            .style(if picked {
                                super::selected_row_button
                            } else {
                                super::row_button
                            })
                            .on_press(Message::SelectChordRow(index)),
                    )
                }),
        )
        .id(list_id())
        .height(Length::Fill)
        .into()
    };

    let list_card = focus_ring(
        container(list)
            .width(Length::Fixed(232.0))
            .height(Length::Fill)
            .padding(10)
            .style(card_container),
        focused == FocusTarget::ChordList,
    );

    let detail = match library.selected_chord() {
        None => container(Space::new()).into(),
        Some(chord) => detail_pane(
            chord,
            library.selected_voicing(),
            library.notation(),
            focused,
        ),
    };

    container(
        row![
            column![search_box, list_card]
                .spacing(12)
                .width(Length::Fixed(240.0)),
            detail,
        ]
        .spacing(20),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(iced::Padding {
        top: 8.0,
        right: 48.0,
        bottom: 40.0,
        left: 48.0,
    })
    .into()
}

fn detail_pane(
    chord: Chord,
    selected: usize,
    notation: Notation,
    focused: FocusTarget,
) -> iced::Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, button, column, container, row, scrollable, text};

    // Spelled with the same glyphs the symbol above them uses, the way the scale trainer's
    // summary card spells its root and its formula. `note.to_string()` is the ASCII the
    // canvas is limited to, and this is not a canvas.
    let notes = chord
        .notes()
        .into_iter()
        .fold(row![].spacing(14), |line, note| {
            line.push(note_label(note, 18, BODY))
        });
    let degrees = intervalic_text(chord.degrees());

    // Three buttons rather than one that cycles. A control whose label changes on every
    // press does not read as a control with options — it reads as a status line, and
    // nobody presses a status line to see what else it could say. Laid out as the scale
    // trainer lays out its roots: pills, with the chosen one filled.
    let notation_buttons =
        NOTATIONS
            .iter()
            .enumerate()
            .fold(row![].spacing(6), |buttons, (index, &choice)| {
                let chosen = choice == notation;

                buttons.push(focus_ring(
                    button(
                        text(match choice {
                            Notation::Notes => "notes",
                            Notation::Intervals => "degrees",
                            Notation::Fingers => "fingers",
                        })
                        .size(14),
                    )
                    .padding([6, 14])
                    .style(if chosen {
                        super::selected_root_button
                    } else {
                        ghost_button
                    })
                    .on_press(Message::SetNotation(index)),
                    focused == FocusTarget::NotationChoice(index),
                ))
            });

    let header = container(
        column![
            row![
                chord_symbol(chord, 44, INK),
                Space::new().width(Length::Fill),
                notation_buttons,
            ]
            .align_y(iced::Alignment::Center),
            row![notes, Space::new().width(Length::Fixed(32.0)), degrees]
                .align_y(iced::Alignment::Center),
        ]
        .spacing(10),
    )
    .width(Length::Fill)
    .padding(24)
    .style(card_container);

    let shapes = voicings(chord, &DRILL_NECK);
    let strip: iced::Element<'static, Message> = if shapes.is_empty() {
        // Specified rather than hidden: the chord keeps its notes and degrees, and the
        // screen says why there is nothing to look at.
        text("No shape available for this chord")
            .size(16)
            .color(MUTE)
            .into()
    } else {
        // Wrapped rather than scrolled sideways: a shape that does not fit drops to the
        // next line, so the whole set is one block to read instead of a strip to drag.
        scrollable(
            shapes
                .iter()
                .enumerate()
                .fold(row![].spacing(12), |strip, (index, &voicing)| {
                    strip.push(voicing_card(
                        chord,
                        voicing,
                        index,
                        index == selected,
                        notation,
                    ))
                })
                .wrap()
                .vertical_spacing(12),
        )
        .into()
    };

    container(column![header, container(strip).padding(8)].spacing(16))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn voicing_card(
    chord: Chord,
    voicing: Voicing,
    index: usize,
    picked: bool,
    notation: Notation,
) -> iced::Element<'static, Message> {
    use iced::widget::{button, column, container, text};

    let caption = format!("{}  ·  {}", position_label(voicing), voicing.shape_name());
    // The picked shape is drawn half again as large. Its caption grows with it, so the two
    // stay one object rather than a big picture with a small note under it.
    let (size, caption_size) = if picked { (FEATURE, 16) } else { (STRIP, 13) };

    container(
        button(
            column![
                chord_diagram(diagram_for(chord, voicing, notation), size),
                text(caption)
                    .size(caption_size)
                    .color(if picked { INK } else { MUTE }),
            ]
            .spacing(8)
            .align_x(iced::Alignment::Center),
        )
        .padding(8)
        .style(if picked {
            super::selected_row_button
        } else {
            super::row_button
        })
        // A press anywhere inside picks the voicing: selection is about the diagram as a
        // whole, never about which position was pressed.
        .on_press(Message::SelectVoicing(index)),
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    use keyboard::key::Named;

    fn pc(semitone: u8) -> PitchClass {
        PitchClass::new(semitone)
    }

    fn arrow(named: Named) -> keyboard::Key {
        keyboard::Key::Named(named)
    }

    fn typed(text: &str) -> ChordLibrary {
        let mut library = ChordLibrary::new();
        library.set_query(text.to_string());
        library
    }

    #[test]
    fn an_empty_query_lists_the_whole_library() {
        let library = ChordLibrary::new();

        assert_eq!(library.rows().len(), 12 * 15);
    }

    #[test]
    fn the_rows_run_by_root_then_by_the_curated_quality_order() {
        // Not alphabetical: within a root, triads first and extensions last.
        let rows = ChordLibrary::new().rows();

        assert_eq!(rows.first().map(|c| c.to_string()), Some("C".into()));
        assert_eq!(rows[1].to_string(), "Cm");
        assert_eq!(rows[15].root(), pc(1), "the second group is the next root");
        assert_eq!(
            rows.last().map(|c| c.kind()),
            Some(ChordQuality::Augmented7)
        );
    }

    #[test]
    fn a_root_alone_narrows_to_that_root() {
        let library = typed("f#");
        let rows = library.rows();

        assert_eq!(rows.len(), 15);
        assert!(rows.iter().all(|chord| chord.root() == pc(6)));
    }

    #[test]
    fn a_quality_alone_narrows_to_that_quality_on_every_root() {
        // The list is a filter over the whole library now, so a bare quality is twelve
        // chords rather than one — which is the more useful answer and the more obvious
        // one, since nothing on screen was ever naming a "current root".
        let library = typed("m7b5");
        let rows = library.rows();

        assert_eq!(rows.len(), 12);
        assert!(
            rows.iter()
                .all(|chord| chord.kind() == ChordQuality::HalfDiminished7)
        );
    }

    #[test]
    fn a_root_and_a_quality_narrow_to_that_chord() {
        // Nothing is written longer than `maj7`, so this one narrows to a single row. See
        // `a_named_quality_keeps_the_chords_that_extend_it` for one that does not.
        let library = typed("bbmaj7");

        assert_eq!(library.rows().len(), 1);
        assert_eq!(
            library.selected_chord().map(|c| c.to_string()),
            Some("Bbmaj7".into())
        );
    }

    #[test]
    fn clearing_the_query_restores_the_whole_library() {
        let mut library = typed("f#maj7");
        library.set_query(String::new());

        assert_eq!(library.rows().len(), 12 * 15);
    }

    #[test]
    fn an_exact_parse_is_not_ranked_against_an_approximate_one() {
        // `cm7` is a subsequence of `Cmaj7`, so a matcher scoring by adjacency would offer
        // the major seventh here. Parsing settles it outright and nothing else is listed.
        let library = typed("cm7");

        assert_eq!(library.rows().len(), 1);
        assert_eq!(
            library.selected_chord().map(|c| c.to_string()),
            Some("Cm7".into())
        );
    }

    #[test]
    fn a_named_quality_keeps_the_chords_that_extend_it() {
        // `Cmaj` is a stop on the way to `Cmaj7`, and used to be the end of the road: an
        // exact parse listed that one chord and hid every longer name it starts.
        let names = |query: &str| {
            typed(query)
                .rows()
                .iter()
                .map(|chord| chord.to_string())
                .collect::<Vec<_>>()
        };

        assert_eq!(names("cmaj"), ["C", "C6", "Cmaj7"]);
        assert_eq!(names("cdim"), ["Cdim", "Cdim7"]);
        assert_eq!(names("cm"), ["Cm", "Cm6", "Cm7", "CmMaj7"]);
        // The chord named outright still leads, so `Enter` picks what was typed.
        assert_eq!(
            typed("cm").selected_chord().map(|c| c.to_string()),
            Some("Cm".into())
        );
    }

    #[test]
    fn a_quality_alone_extends_on_every_root() {
        // The same widening one level up: `dim` alone must not hide the twelve `dim7`s.
        let rows = typed("dim").rows();

        assert_eq!(rows.len(), 24);
        assert_eq!(rows[0].to_string(), "Cdim");
        assert_eq!(rows[1].to_string(), "Cdim7");
    }

    #[test]
    fn a_query_that_does_not_parse_falls_back_to_approximate_matching() {
        // A typo: `cmj7` reads as no chord, and the subsequence finds the one meant.
        let library = typed("cmj7");

        assert_eq!(
            library.rows().first().map(|c| c.to_string()),
            Some("Cmaj7".into())
        );
    }

    #[test]
    fn a_query_matching_nothing_leaves_the_list_empty() {
        let library = typed("zzz");

        assert!(library.rows().is_empty());
        assert_eq!(library.selected_chord(), None);
    }

    #[test]
    fn a_row_is_named_under_the_quality_it_carries() {
        // The scale trainer's rule, carried across: a pitch class is not spelled one fixed
        // way down a group, it is spelled as each chord is written. This is also why a
        // group header has to show both names — see `root_header`.
        let names: Vec<String> = typed("a#")
            .rows()
            .iter()
            .map(|chord| chord.to_string())
            .collect();

        assert!(names.iter().any(|name| name.starts_with("Bb")));
        assert!(names.iter().any(|name| name.starts_with("A#")));
    }

    #[test]
    fn a_black_key_group_needs_both_of_its_names() {
        // What `root_header` renders the pair for. On the five black keys the qualities
        // genuinely disagree, so one name would be wrong about part of its own group.
        for &root in &PitchClass::ALL {
            let spellings: Vec<String> = ChordQuality::ALL
                .iter()
                .map(|&kind| Chord::new(root, kind).root_note().to_string())
                .collect();
            let mut distinct = spellings.clone();
            distinct.sort();
            distinct.dedup();

            let has_two_names = Spelling::Sharps.spell(root) != Spelling::Flats.spell(root);

            assert_eq!(
                distinct.len() > 1,
                has_two_names,
                "{root:?} spells {distinct:?}"
            );
        }
    }

    #[test]
    fn while_the_box_has_focus_a_motion_key_types() {
        let mut library = ChordLibrary::new();
        library.focus_search();

        for c in ["j", "k", "h", "l", "?"] {
            library.handle_key(
                &keyboard::Key::Character(c.into()),
                keyboard::Modifiers::empty(),
            );
        }

        assert_eq!(library.query(), "jkhl?");
        assert!(library.search_focused());
    }

    #[test]
    fn the_arrows_pick_a_row_without_leaving_the_box() {
        let mut library = ChordLibrary::new();
        library.focus_search();

        library.handle_key(&arrow(Named::ArrowDown), keyboard::Modifiers::empty());
        library.handle_key(&arrow(Named::ArrowDown), keyboard::Modifiers::empty());

        assert_eq!(library.selected_row(), 2);
        assert!(library.search_focused(), "the box lost focus");
        assert_eq!(library.query(), "", "an arrow typed");
    }

    #[test]
    fn the_selection_stops_at_the_ends() {
        let mut library = ChordLibrary::new();

        library.move_row(-1);
        assert_eq!(library.selected_row(), 0);

        library.move_row(10_000);
        assert_eq!(library.selected_row(), library.rows().len() - 1);
    }

    #[test]
    fn escape_unfocuses_the_box_and_keeps_the_query() {
        let mut library = typed("cmaj7");
        library.focus_search();
        library.move_row(0);

        let outcome = library.handle_key(&arrow(Named::Escape), keyboard::Modifiers::empty());

        assert_eq!(
            outcome,
            KeyOutcome::Handled,
            "escape fell through to the app"
        );
        assert!(!library.search_focused());
        assert_eq!(library.query(), "cmaj7", "the query was cleared");
        assert_eq!(library.rows().len(), 1);
    }

    #[test]
    fn enter_finishes_the_search_and_hands_on_the_keyboard() {
        // The difference from escape: escape backs out, enter is done. Only enter says the
        // ring should move, and `App` is what moves it.
        let mut library = typed("cmaj7");
        library.focus_search();

        let outcome = library.handle_key(&arrow(Named::Enter), keyboard::Modifiers::empty());

        assert_eq!(outcome, KeyOutcome::Accepted);
        assert!(!library.search_focused());
        assert_eq!(library.query(), "cmaj7", "enter cleared the query");
    }

    #[test]
    fn backspace_deletes_at_the_caret() {
        let mut library = ChordLibrary::new();
        library.focus_search();

        for c in ["c", "m", "a", "j", "7"] {
            library.handle_key(
                &keyboard::Key::Character(c.into()),
                keyboard::Modifiers::empty(),
            );
        }
        library.handle_key(&arrow(Named::Backspace), keyboard::Modifiers::empty());

        assert_eq!(library.query(), "cmaj");
    }

    #[test]
    fn a_command_modified_key_falls_through_to_the_app() {
        // `Ctrl+K` has to keep working from inside the box, so the box must decline it.
        let mut library = ChordLibrary::new();
        library.focus_search();

        let outcome = library.handle_key(
            &keyboard::Key::Character("k".into()),
            keyboard::Modifiers::CTRL,
        );

        assert_eq!(outcome, KeyOutcome::Ignored);
        assert_eq!(library.query(), "");
    }

    #[test]
    fn the_voicing_selection_walks_and_stops() {
        let mut library = typed("e");
        library.select_row(0);

        let count = voicings(
            library.selected_chord().expect("E major is listed"),
            &DRILL_NECK,
        )
        .len();
        assert!(count > 1, "E major should offer more than one shape");

        library.move_voicing(1);
        assert_eq!(library.selected_voicing(), 1);

        library.move_voicing(-100);
        assert_eq!(library.selected_voicing(), 0);

        library.move_voicing(100);
        assert_eq!(library.selected_voicing(), count - 1);
    }

    #[test]
    fn picking_a_new_chord_returns_to_its_first_shape() {
        let mut library = typed("e");
        library.move_voicing(1);
        library.select_row(1);

        assert_eq!(library.selected_voicing(), 0);
    }

    #[test]
    fn every_chord_in_the_roster_can_be_played() {
        // Not required by the spec — a chord with no shape is specified and handled — but
        // true of the fifteen that ship, and worth knowing if it stops being true.
        for &root in &PitchClass::ALL {
            for &kind in ChordQuality::ALL {
                let chord = Chord::new(root, kind);

                assert!(
                    !voicings(chord, &DRILL_NECK).is_empty(),
                    "{chord} cannot be played"
                );
            }
        }
    }
}
