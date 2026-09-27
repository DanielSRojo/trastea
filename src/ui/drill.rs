//! What the drills share: which way a drill runs, and the twelve distances the two interval
//! drills answer in.

use iced::Color;

use crate::music::intervals::Interval;

use super::style::MUSIC_FONT;
use super::{Message, interval_token};

/// One entry of the answer vocabulary shared by the two interval drills.
///
/// Here rather than in either screen for the reason [`Drill`] is here: both mean the same
/// twelve distances by it. That the twelve also divide evenly into `ANSWER_ROW_WIDTH` is a
/// fact the Interval Trainer's grid uses and the played drill does not — this is a
/// vocabulary of nameable distances, not a grid layout.
///
/// Not `music/intervals.rs`: collapsing a tritone's two spellings into one entry is a
/// statement about what a fretboard can distinguish with no key established, and `music/`
/// holds no instrument.
///
/// `twin` is the other spelling of the same distance, where the neck cannot tell the two
/// apart — the tritone, and the two chord degrees the library added to `Interval`. Carrying
/// the twin as an `Interval` rather than as a written
/// label is what stops the button's text from drifting away from the interval it stands
/// for — the label is rendered from these two, never stored beside them.
pub(super) struct AnswerChoice {
    pub(super) interval: Interval,
    pub(super) twin: Option<Interval>,
}

/// The twelve answers, in the order the grid draws them.
///
/// One table, and the grid, the focus rows, and the pool prompts are drawn from all read
/// it — the way `HOME_MENU` is the single source of the menu's buttons, focus cells, and
/// digit keys. `Interval::ALL` is fifteen; this is that list with each distance written two
/// ways collapsed into one entry, which is what makes the count divide evenly into
/// `ANSWER_ROW_WIDTH` and leaves no ragged final row.
pub(super) const ANSWERS: [AnswerChoice; 12] = [
    AnswerChoice {
        interval: Interval::Unison,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MinorSecond,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MajorSecond,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MinorThird,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MajorThird,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::PerfectFourth,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::AugmentedFourth,
        twin: Some(Interval::DiminishedFifth),
    },
    AnswerChoice {
        interval: Interval::PerfectFifth,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MinorSixth,
        twin: Some(Interval::AugmentedFifth),
    },
    AnswerChoice {
        interval: Interval::MajorSixth,
        twin: Some(Interval::DiminishedSeventh),
    },
    AnswerChoice {
        interval: Interval::MinorSeventh,
        twin: None,
    },
    AnswerChoice {
        interval: Interval::MajorSeventh,
        twin: None,
    },
];

/// How many answers the grid offers. The focus grid reads this so the cells and the
/// buttons are built from one count and cannot drift apart.
pub(super) const ANSWER_COUNT: usize = ANSWERS.len();

/// The interval the grid's `index`-th button stands for, or `None` past the end.
///
/// `App` reaches the vocabulary through this rather than through the table itself, which
/// is what keeps `AnswerChoice` and its twin field private to this module — the parent
/// needs an interval to hand back, not the shape it was stored in.
pub(super) fn answer_at(index: usize) -> Option<Interval> {
    ANSWERS.get(index).map(|choice| choice.interval)
}

/// The vocabulary entry for an interval — the one the grid would draw for it.
///
/// Two spellings of one distance share an entry, so `DiminishedFifth` resolves to the
/// `AugmentedFourth` entry that names them both. Falls back to a twinless entry for an
/// interval outside the vocabulary, which neither drill can produce but which spares this
/// from being fallible for a case no caller could act on.
pub(super) fn choice_for(interval: Interval) -> AnswerChoice {
    ANSWERS
        .iter()
        .find(|choice| {
            choice.interval == interval || choice.twin.is_some_and(|twin| twin == interval)
        })
        .map_or(
            AnswerChoice {
                interval,
                twin: None,
            },
            |choice| AnswerChoice {
                interval: choice.interval,
                twin: choice.twin,
            },
        )
}

/// One answer's label: its degree formula, and the twin spelling after a slash where there
/// is one.
///
/// Built the way `intervalic_text` builds the scale trainer's formula row, and for the same
/// reason: the accidental glyph renders in `MUSIC_FONT` and the degree digit in the body
/// font, and one `text` widget cannot carry both. The twinned entries run this twice.
pub(super) fn interval_label(
    choice: &AnswerChoice,
    size: u32,
    color: Color,
) -> iced::widget::Row<'static, Message> {
    use iced::widget::{row, text};

    let token = |tokens: iced::widget::Row<'static, Message>, interval: Interval| {
        let (glyph, digit) = interval_token(interval);
        let mut tokens = tokens;

        if let Some(glyph) = glyph {
            tokens = tokens.push(
                text(glyph.to_string())
                    .size(size)
                    .font(MUSIC_FONT)
                    .color(color),
            );
        }

        tokens.push(text(digit.to_string()).size(size).color(color))
    };

    let label = token(row![].spacing(0), choice.interval);

    match choice.twin {
        Some(twin) => token(label.push(text("/").size(size).color(color)), twin),
        None => label,
    }
}

/// Which way a drill is running. Transient only — passed to `draw_prompt` to say which
/// kind of prompt to make, never stored. The stored direction is `Prompt`'s variant.
///
/// Here rather than in either trainer: both mean the same two words by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Drill {
    NameIt,
    FindIt,
}

impl Drill {
    pub(super) fn flipped(self) -> Drill {
        match self {
            Drill::NameIt => Drill::FindIt,
            Drill::FindIt => Drill::NameIt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vocabulary_holds_one_button_per_distance() {
        assert_eq!(ANSWERS.len(), 12);
        assert_eq!(ANSWER_COUNT, ANSWERS.len());

        // Twelve entries and twelve distinct distances is the whole twin decision stated as
        // an invariant: judging compares semitones, so two buttons standing for one distance
        // would be two spellings of the same answer with no way to tell which the drill
        // meant.
        let mut distances: Vec<u8> = ANSWERS
            .iter()
            .map(|choice| choice.interval.semitones())
            .collect();
        distances.sort_unstable();
        distances.dedup();

        assert_eq!(distances.len(), ANSWERS.len(), "two buttons, one distance");
    }

    #[test]
    fn the_vocabulary_covers_every_distance_in_an_octave() {
        let mut distances: Vec<u8> = ANSWERS
            .iter()
            .map(|choice| choice.interval.semitones())
            .collect();
        distances.sort_unstable();

        // 0..=11 exactly: every distance a prompt can produce has a button, so judging by
        // semitones can never leave a prompt unanswerable.
        assert_eq!(distances, (0..12).collect::<Vec<u8>>());
    }

    #[test]
    fn the_vocabulary_is_interval_all_with_the_tritones_merged() {
        let named: Vec<Interval> = ANSWERS
            .iter()
            .flat_map(|choice| std::iter::once(choice.interval).chain(choice.twin))
            .collect();

        // Every interval still reachable, one of them through a twin — this is the tripwire
        // that a variant added to `Interval` is added here too.
        assert_eq!(named.len(), Interval::ALL.len());
        for interval in Interval::ALL {
            assert!(named.contains(interval), "{interval} has no button");
        }
    }

    #[test]
    fn a_twin_is_the_same_distance_written_a_second_way() {
        let twinned: Vec<&AnswerChoice> = ANSWERS.iter().filter(|c| c.twin.is_some()).collect();

        assert_eq!(twinned.len(), 3);

        for choice in twinned {
            let twin = choice.twin.expect("filtered to the twinned entries");

            // What earns an entry a twin, and the only thing that does: the neck cannot
            // separate two spellings of one distance, so the drill offers them as one
            // answer. A pair differing in distance would be two answers wrongly merged.
            assert_eq!(
                choice.interval.semitones(),
                twin.semitones(),
                "{} and {twin} are different distances",
                choice.interval
            );
            assert_ne!(
                choice.interval.number(),
                twin.number(),
                "{} and {twin} are the same degree",
                choice.interval
            );
        }
    }

    #[test]
    fn a_button_index_resolves_to_its_interval() {
        for (index, choice) in ANSWERS.iter().enumerate() {
            assert_eq!(answer_at(index), Some(choice.interval));
        }
        assert_eq!(answer_at(ANSWERS.len()), None);
    }
}
