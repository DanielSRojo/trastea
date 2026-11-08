//! The Chord Library: looking a chord up, and drawing the ways to play it.
//! The shapes themselves live in `super::shapes`.

use iced::keyboard;
use iced::widget::canvas;

use crate::music::chords::{Chord, ChordQuality, Query, parse_root};
use crate::music::notes::{PitchClass, Spelling};

use super::chord_diagram::{ChordDiagram, FEATURE, STRIP, StringMark, chord_diagram};
use super::focus::FocusTarget;
use super::neck::DRILL_NECK;
use super::shapes::{Voicing, position_label, voicings};
use super::style::{
    BODY, FEEL_FONT, HAIRLINE_INK, INK, MUSIC_FONT, MUTE, SMUFL_CSYM_AUGMENTED,
    SMUFL_CSYM_DIMINISHED, SMUFL_CSYM_HALF_DIMINISHED, SMUFL_CSYM_MAJOR_SEVENTH, SMUFL_SHARP,
    SUCCESS, card_container, focus_ring, ghost_button, hairline_rule,
};
use super::{Message, Notation, intervalic_text, notation_pills, note_label};

/// The library's state: what has been typed and what is picked.
pub(super) struct ChordLibrary {
    query: String,
    /// Where the caret sits, as a character index into `query`.
    ///
    /// The box is drawn here rather than by iced's `text_input`, which would receive keys as
    /// well as the global subscription, so `l` would both type and move the focus ring.
    caret: usize,
    selected_row: usize,
    selected_voicing: usize,
    search_focused: bool,
    /// Whether a `g` is waiting for its second half. Any other key clears it.
    pending_g: bool,
    /// Kept here rather than on `App` so it does not change the Scale Trainer's labels.
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
            notation: Notation::Fingers,
        }
    }

    pub(super) fn notation(&self) -> Notation {
        self.notation
    }

    pub(super) fn set_notation(&mut self, index: usize) {
        if let Some(&notation) = Notation::ALL.get(index) {
            self.notation = notation;
        }
    }

    /// Opening the screen: an empty box with the caret in it, the notation left as last set.
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
    /// An empty query is every chord. A query that parses picks the chord it names and the
    /// ones extending it; one that does not falls back to approximate matching.
    pub(super) fn rows(&self) -> Vec<Chord> {
        match Query::parse(&self.query) {
            None if self.query.trim().is_empty() => every_chord().collect(),
            None => self.approximate_rows(),
            Some(Query::Root(root)) => every_chord().filter(|chord| chord.root() == root).collect(),
            // `covers` rather than `==`, so `Cmaj` does not hide the `Cmaj7` it starts.
            Some(Query::Quality(kind)) => every_chord()
                .filter(|chord| kind.covers(chord.kind()))
                .collect(),
            Some(Query::Chord { root, kind }) => every_chord()
                .filter(|chord| chord.root() == root && kind.covers(chord.kind()))
                .collect(),
        }
    }

    /// Rows for a query the grammar could not read: the chords whose symbol or spoken name
    /// contains the query's characters in order, nearest match first. `cm7` parses, so it
    /// never reaches here.
    fn approximate_rows(&self) -> Vec<Chord> {
        let needle = self.query.trim().to_lowercase();
        // The library's own order is the last tiebreak.
        let mut scored: Vec<(usize, usize, usize, Chord)> = every_chord()
            .enumerate()
            .filter_map(|(order, chord)| {
                let (start, span) = approximate_match(&needle, chord)?;

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
    /// The offset is the selection's share of the rows: exact at both ends and close enough
    /// between, so it needs neither the viewport's size nor a row's height.
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

    /// Replaces the query. Nothing else is remembered from it.
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

    /// Walks the list, stopping at the ends rather than wrapping.
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

    /// The vim motions this screen claims: `gg` to the first chord, `G` to the last.
    /// Reports whether it took the key. Only reached with the search box unfocused.
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
    /// Character keys type; only the arrows, `Enter` and `Esc` act.
    pub(super) fn handle_key(
        &mut self,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> KeyOutcome {
        use keyboard::key::Named;

        match key.as_ref() {
            // Leaves the box without clearing the query.
            keyboard::Key::Named(Named::Escape) => {
                self.search_focused = false;

                return KeyOutcome::Handled;
            }
            // The row is already selected; the ring moves on to the shapes.
            keyboard::Key::Named(Named::Enter) => {
                self.search_focused = false;

                return KeyOutcome::Accepted;
            }
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
            keyboard::Key::Character(text)
                if !modifiers.intersects(super::keys::COMMAND_MODIFIERS) =>
            {
                let mut next: Vec<char> = self.query.chars().collect();
                let at = self.caret.min(next.len());

                for (offset, c) in text.chars().enumerate() {
                    next.insert(at + offset, c);
                }

                let typed = text.chars().count();
                self.set_query(next.into_iter().collect());
                self.caret = at + typed;
            }
            // Declined, so `Ctrl+K` from inside the box still reaches the app.
            _ => return KeyOutcome::Ignored,
        }

        KeyOutcome::Handled
    }
}

/// What a keystroke in the search box did. `App` moves the ring on `Accepted`.
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
fn every_chord() -> impl Iterator<Item = Chord> {
    PitchClass::ALL.into_iter().flat_map(|root| {
        ChordQuality::ALL
            .iter()
            .map(move |&kind| Chord::new(root, kind))
    })
}

/// The best of `needle` against the chord's symbol, its quality said aloud, or its root
/// followed by that quality, so `cmj7`, `dominant` and `c dominant` all land.
fn approximate_match(needle: &str, chord: Chord) -> Option<(usize, usize)> {
    // Matched without the root, so every root's `dominant` scores alike and keeps library order.
    let spoken = chord.kind().spoken();
    let rooted = parse_root(needle)
        .filter(|&(root, _)| root == chord.root())
        .and_then(|(_, rest)| subsequence(rest.trim_start(), spoken));

    [
        subsequence(needle, &chord.to_string().to_lowercase()),
        subsequence(needle, spoken),
        rooted,
    ]
    .into_iter()
    .flatten()
    .min()
}

/// Where `needle`'s characters appear in `haystack`, in order, as `(start, span)`.
/// An earlier start and then a tighter span rank higher, so `maj7` beats `mMaj7`.
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
enum SymbolPart {
    Text(&'static str),
    Glyph(char),
    /// A glyph set as a superscript. Only the diminished and half-diminished circles are raised.
    Raised(char),
}

/// How a quality is written after its root; the typographic counterpart of `ChordQuality::suffix`.
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
        // `7♯5`, not `+7`, which some read as a major seventh.
        ChordQuality::Augmented7 => &[Text("7"), Glyph(SMUFL_SHARP), Text("5")],
    }
}

/// A chord written with a real accidental and the quality's own glyph where it has one.
fn chord_symbol(
    chord: Chord,
    size: impl Into<iced::Pixels>,
    color: iced::Color,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::{container, text};

    let size = size.into();
    // At the root's own size the diminished circle reaches cap height and `C°` reads as `CO`.
    let quality = iced::Pixels(size.0 * 0.66);
    // At the quality's size the circle comes out as big as the `m` in `Cm`.
    let raised = iced::Pixels(size.0 * 0.42);

    // The row bottom-aligns boxes rather than baselines, so each smaller mark is lifted by its
    // shortfall in descender space. iced's line height is 1.3, roughly 0.3 below the baseline.
    let sits_on_baseline = |mark: iced::Pixels| (size.0 - mark.0) * 0.3;
    // The superscript then rises to bring its top near the root's cap.
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
        // The paddings above turn "same bottom edge" into "same baseline".
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

    match notation {
        Notation::Notes => chord
            .spell(sounding)
            .map(|note| note.to_string())
            .unwrap_or_default(),
        Notation::Intervals => chord
            .degree(sounding)
            .map(|interval| interval.to_string())
            .unwrap_or_default(),
        // An open string carries no finger.
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
        // Display-only for now; the widget hit-tests regardless.
        on_press: None,
    }
}

/// The rule between one root's chords and the next.
/// Both names on black keys: pitch class 1 spells `D♭` under a major triad, `C♯` under a minor.
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
            // A hairline rather than a `Border`, which would draw all four sides.
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

/// The magnifier in the search box, drawn because no embedded font carries `⌕`.
struct SearchIcon;

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

        // The handle leaves the rim at 45°, so it starts a radius away on both axes.
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

    // A glyph rather than a blinking cursor: nothing here animates.
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
                    // Derived here so the selection stays a plain index into the chords.
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
                                super::style::selected_row_button
                            } else {
                                super::style::row_button
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
    use iced::widget::{Space, column, container, row, scrollable, text};

    // Not `note.to_string()`, which is the ASCII the canvas is limited to.
    let notes = chord
        .notes()
        .into_iter()
        .fold(row![].spacing(14), |line, note| {
            line.push(note_label(note, 18, BODY))
        });
    let degrees = intervalic_text(chord.degrees());

    let notation_buttons = notation_pills(
        notation,
        Message::SetNotation,
        FocusTarget::NotationChoice,
        focused,
    );

    let header = container(
        column![
            row![
                chord_symbol(chord, 44, INK),
                Space::new().width(Length::Fill),
                notation_buttons,
            ]
            .align_y(iced::Alignment::Center),
            text(chord.spoken()).size(22).font(FEEL_FONT).color(MUTE),
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
        text("No shape available for this chord")
            .size(16)
            .color(MUTE)
            .into()
    } else {
        // Wrapped rather than scrolled sideways.
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
            super::style::selected_row_button
        } else {
            super::style::row_button
        })
        // A press anywhere inside picks the voicing; the position pressed does not matter.
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
        // A bare quality lists it on all twelve roots.
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
        // Nothing is written longer than `maj7`, so this narrows to a single row.
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
        // `cm7` is a subsequence of `Cmaj7`; parsing settles it and nothing else is listed.
        let library = typed("cm7");

        assert_eq!(library.rows().len(), 1);
        assert_eq!(
            library.selected_chord().map(|c| c.to_string()),
            Some("Cm7".into())
        );
    }

    #[test]
    fn a_named_quality_keeps_the_chords_that_extend_it() {
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
        // `dim` alone must not hide the twelve `dim7`s.
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
    fn a_quality_said_aloud_finds_that_quality_on_every_root() {
        let kinds = |query: &str| {
            typed(query)
                .rows()
                .iter()
                .map(|chord| chord.kind())
                .collect::<Vec<_>>()
        };

        let dominant = kinds("dominant");
        assert_eq!(dominant.len(), 12);
        assert!(dominant.iter().all(|&kind| kind == ChordQuality::Dominant7));

        let suspended = kinds("suspended");
        assert_eq!(suspended.len(), 24);
        assert!(
            suspended
                .iter()
                .all(|&kind| matches!(kind, ChordQuality::Sus2 | ChordQuality::Sus4))
        );

        // `half-diminished` matches too, but further in, so it ranks after the plain ones.
        let diminished = kinds("diminished");
        assert_eq!(diminished.len(), 36);
        assert!(
            diminished[..24]
                .iter()
                .all(|&kind| matches!(kind, ChordQuality::Diminished | ChordQuality::Diminished7))
        );
        assert!(
            diminished[24..]
                .iter()
                .all(|&kind| kind == ChordQuality::HalfDiminished7)
        );

        assert!(kinds("minor").contains(&ChordQuality::MinorMajor7));
    }

    #[test]
    fn a_root_then_a_quality_said_aloud_finds_that_chord() {
        assert_eq!(
            typed("c dominant").selected_chord().map(|c| c.to_string()),
            Some("C7".into())
        );
        assert_eq!(
            typed("f# minor").selected_chord().map(|c| c.to_string()),
            Some("F#m".into())
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
        // A pitch class is spelled as each chord is written, not one fixed way down a group.
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
        // On the five black keys the qualities disagree, so `root_header` shows both names.
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
        // Only enter says the ring should move.
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
        // Not required, but true of the fifteen that ship.
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
