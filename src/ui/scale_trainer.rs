//! The Scale Trainer: the lit neck and the two selector grids that choose what it lights.
//!
//! Here rather than in the parent on size alone — this is four hundred lines of view that
//! nothing else calls. The screen keeps no state of its own: the `Scale` and the `Notation`
//! live on `App`, because three screens read them and none of them can put the pair into a
//! bad state.
//!
//! The drill that walks a scale by degree is *not* here. It is a screen of its own in
//! `walk_the_scale.rs`, for the reason `play_the_interval.rs` is a screen rather than a mode
//! of the Interval Trainer: a drill and the reference it is drawn from are two things to look
//! at, and folding one into the other gave the Scale Trainer two jobs and one keyboard.

use iced::{Element, Padding, font};

use super::fretboard::{Fretboard, MarkerStyle, NoteMarker, fretboard};
use super::{
    BODY, DRILL_NECK, FEEL_FONT, FocusTarget, INK, LINK, MUSIC_FONT, MUTE, Message, NECK_STRINGS,
    Notation, ROOT_SELECTOR_CARD_WIDTH, SELECTOR_CARD_HEIGHT, SMUFL_FLAT, SUMMARY_CARD_HEIGHT,
    SUMMARY_NAME_SIZE, SUMMARY_ROOT_SIZE, card_container, control_button, control_glyph,
    control_label, control_shuffle, intervalic_text, kind_row_spans, marker_label, note_label,
    root_note_row, root_row_spans, scale_kind_row,
};
use crate::music::notes::PitchClass;
use crate::music::scales::{Scale, ScaleKind};
use iced::Color;

pub(super) fn ui_scale_trainer(
    scale: Scale,
    notation: Notation,
    focused: FocusTarget,
) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{Space, column, container, row, text};

    // Display only: no press handler and no cursor, so the neck stays the picture it has
    // always been here. `Message` is inferred from the `Element` this becomes.
    let fb = Fretboard {
        num_frets: DRILL_NECK.frets(),
        highlighted: scale_markers(scale, notation),
        ..Fretboard::default()
    };

    let current_scale_card = container(
        column![
            row![
                note_label(scale.root_note(), SUMMARY_ROOT_SIZE, INK),
                Space::new().width(Length::Fill),
                // One degree, not a formula fragment: `1♭3` was tried first, on the
                // grounds that it reads less like an instruction to flatten the third,
                // and it crowded the header row — three buttons and the root label share
                // this line. Built like the formula row it points at — the glyph in
                // MUSIC_FONT, the digit in the body font — because one `text` cannot
                // carry both, and both at one size because that is how the pair is
                // written.
                control_button(
                    control_glyph(
                        row![
                            control_label(text(SMUFL_FLAT.to_string()).font(MUSIC_FONT), 20.0),
                            control_label(text("3"), 20.0),
                        ]
                        .spacing(0)
                    ),
                    Message::ToggleNotation,
                    focused == FocusTarget::NotationToggle,
                ),
                control_button(
                    control_shuffle(),
                    Message::RerollScale,
                    focused == FocusTarget::RerollScale,
                ),
            ]
            .spacing(8),
            text(scale.kind().name()).size(SUMMARY_NAME_SIZE).color(INK),
            intervalic_text(scale.kind().intervals()),
        ]
        .spacing(10),
    )
    .width(Length::Fixed(ROOT_SELECTOR_CARD_WIDTH))
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    let root_selector_content = container(root_row_spans().fold(
        column![].spacing(16),
        |rows, (start, len)| {
            rows.push(
                container(root_note_row(
                    &PitchClass::ALL[start..start + len],
                    scale,
                    start,
                    focused,
                    Message::SelectRoot,
                    FocusTarget::Root,
                ))
                .width(Length::Fill)
                .center_x(Length::Fill),
            )
        },
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill);

    let root_selector_card = container(root_selector_content)
        .width(Length::Fixed(ROOT_SELECTOR_CARD_WIDTH))
        .height(Length::Fixed(SELECTOR_CARD_HEIGHT))
        .padding(32)
        .style(card_container);

    let scale_selector_content = container(kind_row_spans().fold(
        column![].spacing(12),
        |rows, (start, len)| {
            rows.push(scale_kind_row(
                &ScaleKind::ALL[start..start + len],
                scale.kind(),
                start,
                focused,
                Message::SelectScaleKind,
                FocusTarget::ScaleKind,
            ))
        },
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .center_y(Length::Fill);

    let scale_selector_card = container(scale_selector_content)
        .width(Length::Fill)
        .height(Length::Fixed(SELECTOR_CARD_HEIGHT))
        .padding(32)
        .style(card_container);

    let explanation_font = iced::Font {
        family: font::Family::Cursive,
        style: font::Style::Italic,
        ..iced::Font::DEFAULT
    };

    let explanation_card = container(
        column![
            text(scale.kind().feel())
                .size(22)
                .font(FEEL_FONT)
                .color(BODY)
                .width(Length::Fill),
            text(scale.kind().common_usage())
                .size(18)
                .font(explanation_font)
                .color(MUTE)
                .width(Length::Fill),
        ]
        .spacing(12),
    )
    .width(Length::Fill)
    .height(Length::Fixed(SUMMARY_CARD_HEIGHT))
    .padding(32)
    .style(card_container);

    let summary_cards = row![current_scale_card, explanation_card]
        .width(Length::Fill)
        .spacing(16);

    let selector_cards = row![root_selector_card, scale_selector_card]
        .width(Length::Fill)
        .spacing(16);

    let details = column![summary_cards, selector_cards]
        .width(Length::Fill)
        .spacing(16);

    container(row![fretboard(fb), details].spacing(32))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 24.0,
            right: 64.0,
            bottom: 48.0,
            left: 64.0,
        })
        .center_y(Length::Fill)
        .into()
}

fn scale_markers(scale: Scale, notation: Notation) -> Vec<NoteMarker> {
    let mut markers = Vec::new();

    // The neck the screen draws, rather than the tuning and a literal twelve. Those agreed
    // with `DRILL_NECK` by coincidence; this asks it.
    for string in 0..NECK_STRINGS {
        for fret in 0..=DRILL_NECK.frets() {
            // Which of twelve is all a scale marker wants: a scale has no octave.
            let Some(pitch_class) = DRILL_NECK.pitch_class_at(string, fret) else {
                continue;
            };

            // `spell` stays the membership test, so the set of markers is the same
            // in both modes and the mode cannot make a dot disappear. The degree is
            // then an `expect` rather than a second gate:
            // `degree_and_spell_agree_on_membership` in scales.rs proves a spelled
            // pitch class always has one.
            if let Some(note) = scale.spell(pitch_class) {
                let degree = scale
                    .degree(pitch_class)
                    .expect("degree_and_spell_agree_on_membership: a spelled pitch has a degree");

                markers.push(NoteMarker {
                    string,
                    fret,
                    label: marker_label(notation, note, degree),
                    color: if pitch_class == scale.root() {
                        Color::from_rgb8(0xff, 0x4d, 0x4d)
                    } else {
                        LINK
                    },
                    // Filled: these carry a note name or a degree, and the label needs
                    // something to read against.
                    style: MarkerStyle::Filled,
                });
            }
        }
    }
    markers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::STANDARD_TUNING;
    use std::collections::HashSet;

    /// Every marker's pitch class, derived independently of `scale.spell` by
    /// transposing the open string — the same computation `scale_markers` itself
    /// does, kept separate here so a test failure means the marker disagrees
    /// with the tuning, not with itself.
    fn marker_pitch_class(marker: &NoteMarker) -> PitchClass {
        STANDARD_TUNING[marker.string]
            .pitch_class()
            .transpose(marker.fret as u8)
    }

    #[test]
    fn standard_tuning_low_open_string_is_e_natural() {
        // Pins STANDARD_TUNING itself: E A D G B E low to high, pitch classes
        // 4 9 2 7 11 4. Nothing else asserts this constant directly.
        let e_ionian = Scale::new(PitchClass::new(4), ScaleKind::Ionian);

        let markers = scale_markers(e_ionian, Notation::Notes);
        let open_low_e = markers
            .iter()
            .find(|m| m.string == 0 && m.fret == 0)
            .expect("the open low E is in E Ionian");

        assert_eq!(open_low_e.label, "E");
    }

    /// The markers and the neck agree because the markers *ask* the neck, rather than
    /// because a tuning and a literal twelve happened to say the same thing. This is the
    /// assertion that survives someone lengthening `DRILL_NECK`.
    #[test]
    fn every_marker_sits_where_the_neck_says_it_does() {
        let scale = Scale::new(PitchClass::new(4), ScaleKind::Ionian);

        for marker in scale_markers(scale, Notation::Notes) {
            assert_eq!(
                DRILL_NECK.pitch_class_at(marker.string, marker.fret),
                Some(marker_pitch_class(&marker)),
                "string {} fret {}",
                marker.string,
                marker.fret
            );
            assert!(marker.fret <= DRILL_NECK.frets());
        }
    }

    /// Every marker's position, for comparing two labellings of one neck.
    fn marker_positions(markers: &[NoteMarker]) -> Vec<(usize, usize)> {
        markers.iter().map(|m| (m.string, m.fret)).collect()
    }

    #[test]
    fn interval_notation_labels_the_markers_with_the_formula() {
        let a_aeolian = Scale::new(PitchClass::new(9), ScaleKind::Aeolian);

        let names = scale_markers(a_aeolian, Notation::Notes);
        let degrees = scale_markers(a_aeolian, Notation::Intervals);

        // The same dots, both ways round: the mode may change the text and nothing
        // else. Membership comes from `spell`, which never sees the mode.
        assert_eq!(
            marker_positions(&names),
            marker_positions(&degrees),
            "switching notation moved or dropped a marker"
        );
        let colors = |markers: &[NoteMarker]| -> Vec<(f32, f32, f32, f32)> {
            markers
                .iter()
                .map(|m| (m.color.r, m.color.g, m.color.b, m.color.a))
                .collect()
        };
        assert_eq!(
            colors(&names),
            colors(&degrees),
            "switching notation recoloured a marker"
        );

        // A minor's formula, and nothing outside it. Set equality both ways: no label
        // may appear that is not a degree of this scale, and every degree must show up
        // somewhere on twelve frets of six strings.
        let labels: HashSet<&str> = degrees.iter().map(|m| m.label.as_str()).collect();
        let expected: HashSet<&str> = ["1", "2", "b3", "4", "5", "b6", "b7"].into_iter().collect();
        assert_eq!(labels, expected);

        for marker in &degrees {
            if marker_pitch_class(marker) == a_aeolian.root() {
                assert_eq!(marker.label, "1", "the root is degree 1");
            }
        }
    }

    #[test]
    fn spelling_does_not_reach_interval_labels() {
        // A degree is a position in the formula, and no choice of sharps or flats
        // moves one. This used to be asserted by toggling one root two ways; with the
        // spelling derived there is no toggle, so it is asserted across every root
        // instead — which sweeps roots that resolve to sharps (F♯ Ionian) and roots
        // that resolve to flats (D♭ Ionian) alike. If spelling leaked into a degree,
        // those two would disagree.
        let kind = ScaleKind::Ionian;
        let mut expected: Vec<String> = kind.intervals().iter().map(|i| i.to_string()).collect();
        expected.sort();

        for &pitch_class in &PitchClass::ALL {
            let scale = Scale::new(pitch_class, kind);

            let mut labels: Vec<String> = scale_markers(scale, Notation::Intervals)
                .iter()
                .map(|marker| marker.label.clone())
                .collect();
            labels.sort();
            labels.dedup();

            assert_eq!(labels, expected, "{:?} Ionian", scale.root_note());
        }
    }

    #[test]
    fn f_ionian_names_pitch_class_ten_b_flat_not_a_sharp() {
        // The branch's headline claim, asserted where it reaches the screen:
        // F Ionian's fourth degree is B♭, not the semitone-only A♯ the old code
        // produced. `Spelling::Sharps` is exactly the setting that used to expose
        // the bug, since a naive sharps-only spelling would pick A♯ here.
        let scale = Scale::new(PitchClass::new(5), ScaleKind::Ionian);

        let markers = scale_markers(scale, Notation::Notes);
        let mut checked = 0;

        for marker in &markers {
            if marker_pitch_class(marker).semitone() == 10 {
                // Read off the label, now that a marker carries text rather than a
                // `Note`. That is nearer the claim in any case — `Bb` is what the
                // neck actually shows — and the structure behind it is pinned in
                // scales.rs, where the spelling is decided.
                assert_eq!(
                    marker.label, "Bb",
                    "string {} fret {}",
                    marker.string, marker.fret
                );
                checked += 1;
            }
        }

        assert!(
            checked > 0,
            "F Ionian on standard tuning never reaches pitch class 10"
        );
    }

    // `toggling_spelling_moves_no_marker_but_relabels_at_least_one` stood here,
    // automating a manual checklist item about the ♯/♭ toggle. There is no toggle to
    // check now. Its surviving half — that a naming choice never moves a marker — is
    // `naming_never_moves_the_scale` in `music/scales.rs`, over all 192 combinations
    // rather than one root two ways.

    #[test]
    fn root_highlighting_keys_on_pitch_class_not_on_note() {
        // Pins the exact predicate `scale_markers` highlights on: pitch-class
        // equality against `scale.root()`, independently recomputed here rather
        // than trusted from `marker.note`. Swapping the source line to
        // `marker.note == scale.root_note()` happens to color identically today
        // — every note in one scale's `notes()` has a distinct pitch class, and
        // `every_scale_spells_without_failing` now pins `notes()[0] ==
        // root_note()` (Fix 5 in scales.rs) — but that equivalence depends on
        // both of those holding elsewhere. Keying on `PitchClass` directly, as
        // this test requires, stays correct even if either invariant is ever
        // broken by an unrelated change; keying on `Note` would not.
        let scale = Scale::new(PitchClass::new(5), ScaleKind::Ionian);

        let markers = scale_markers(scale, Notation::Notes);
        let root_color = Color::from_rgb8(0xff, 0x4d, 0x4d);
        let (mut root_markers, mut other_markers) = (0, 0);

        for marker in &markers {
            if marker_pitch_class(marker) == scale.root() {
                assert_eq!(marker.color, root_color, "root marker not highlighted");
                root_markers += 1;
            } else {
                assert_eq!(marker.color, LINK, "non-root marker not LINK");
                other_markers += 1;
            }
        }

        assert!(root_markers > 0, "the root never appears on this tuning");
        assert!(other_markers > 0, "every marker was treated as the root");
    }

    #[test]
    fn every_scale_marker_is_filled_so_the_ring_stays_the_question() {
        // The prompt's ring is the only outlined marker; a scale dot must not become one.
        for root in PitchClass::ALL {
            for &kind in ScaleKind::ALL {
                let scale = Scale::new(root, kind);

                for notation in [Notation::Notes, Notation::Intervals] {
                    for marker in scale_markers(scale, notation) {
                        assert_eq!(
                            marker.style,
                            MarkerStyle::Filled,
                            "{scale:?} string {} fret {}",
                            marker.string,
                            marker.fret
                        );
                    }
                }
            }
        }
    }
}
