//! What the drills share: direction, scoring, and the twelve interval answers.

use std::collections::HashMap;
use std::hash::Hash;

use iced::Color;

use crate::music::intervals::Interval;

use super::style::MUSIC_FONT;
use super::{Message, interval_token};

/// One entry of the answer vocabulary shared by the two interval drills.
///
/// `twin` is the other spelling of a distance the neck cannot tell apart; the label is
/// rendered from both, never stored.
pub(super) struct AnswerChoice {
    pub(super) interval: Interval,
    pub(super) twin: Option<Interval>,
}

/// The twelve answers, in the order the grid draws them: `Interval::ALL` with each
/// two-way-spelled distance collapsed into one entry.
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

pub(super) const ANSWER_COUNT: usize = ANSWERS.len();

/// The interval the grid's `index`-th button stands for, or `None` past the end.
pub(super) fn answer_at(index: usize) -> Option<Interval> {
    ANSWERS.get(index).map(|choice| choice.interval)
}

/// The vocabulary entry for an interval, so `DiminishedFifth` resolves to the
/// `AugmentedFourth` entry. Falls back to a twinless entry outside the vocabulary.
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

/// One answer's label: its degree formula, and the twin spelling after a slash.
///
/// Built from separate widgets because the accidental renders in `MUSIC_FONT` and the digit
/// in the body font.
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

/// Which way a drill is running. Transient; the stored direction is `Prompt`'s variant.
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

/// A drill's run: the streak, the session best, and the marks on the current prompt.
pub(super) struct Score<A> {
    streak: u32,
    best: u32,
    /// Every wrong answer to the current prompt, so earlier ones stay marked.
    wrong: Vec<A>,
    /// The correct answer just given, held while it is lit. `Some` is the paused state.
    correct: Option<A>,
}

// written out because the derive would require `A: Default`
impl<A> Default for Score<A> {
    fn default() -> Self {
        Score {
            streak: 0,
            best: 0,
            wrong: Vec::new(),
            correct: None,
        }
    }
}

impl<A: Copy + PartialEq> Score<A> {
    /// Takes a judged answer, unless one is already lit.
    ///
    /// Answers during the flash are the tail of the press that scored, so they are ignored.
    pub(super) fn mark(&mut self, answer: A, right: bool) {
        if self.correct.is_some() {
            return;
        }

        if right {
            self.streak += 1;
            self.best = self.best.max(self.streak);
            self.correct = Some(answer);
        } else {
            // Deduplicated so hammering one wrong button cannot grow this without bound.
            if !self.wrong.contains(&answer) {
                self.wrong.push(answer);
            }
            self.streak = 0;
        }
    }

    /// Zeroes the run without touching the session best.
    pub(super) fn break_streak(&mut self) {
        self.streak = 0;
    }

    /// Clears the marks for a fresh prompt. The streak carries on.
    pub(super) fn next_prompt(&mut self) {
        self.wrong.clear();
        self.correct = None;
    }

    /// Whether a correct answer is lit, which is the window the flash timer runs in.
    pub(super) fn is_flashing(&self) -> bool {
        self.correct.is_some()
    }

    pub(super) fn streak(&self) -> u32 {
        self.streak
    }

    pub(super) fn best(&self) -> u32 {
        self.best
    }

    pub(super) fn wrong(&self) -> &[A] {
        &self.wrong
    }

    pub(super) fn correct(&self) -> Option<A> {
        self.correct
    }
}

/// What one miss adds to a prompt's weight, on top of the floor of one every prompt has.
const MISS_DEBT: u32 = 4;
/// Four misses' worth, so one stubborn prompt cannot crowd the rest of the pool out.
const MAX_DEBT: u32 = 4 * MISS_DEBT;

/// The prompts the learner has been missing, weighted so the draw comes back to them.
pub(super) struct Misses<P> {
    debt: HashMap<P, u32>,
}

// written out because the derive would require `P: Default`
impl<P> Default for Misses<P> {
    fn default() -> Self {
        Misses {
            debt: HashMap::new(),
        }
    }
}

impl<P: Eq + Hash> Misses<P> {
    pub(super) fn missed(&mut self, prompt: P) {
        let debt = self.debt.entry(prompt).or_default();
        *debt = (*debt + MISS_DEBT).min(MAX_DEBT);
    }

    /// Halves the debt, so a prompt is trusted again only after a few clean answers.
    pub(super) fn recalled(&mut self, prompt: &P) {
        if let Some(debt) = self.debt.get_mut(prompt) {
            *debt /= 2;
        }
    }

    /// Never zero, so a prompt answered well still comes up and the draw always has a pool.
    pub(super) fn weight(&self, prompt: &P) -> u32 {
        1 + self.debt.get(prompt).copied().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unmissed_prompt_weighs_one() {
        assert_eq!(Misses::default().weight(&'a'), 1);
    }

    #[test]
    fn a_miss_raises_only_its_own_prompt() {
        let mut misses = Misses::default();
        misses.missed('a');

        assert_eq!(misses.weight(&'a'), 1 + MISS_DEBT);
        assert_eq!(misses.weight(&'b'), 1);
    }

    #[test]
    fn the_debt_stops_climbing_at_its_cap() {
        let mut misses = Misses::default();
        for _ in 0..10 {
            misses.missed('a');
        }

        assert_eq!(misses.weight(&'a'), 1 + MAX_DEBT);
    }

    #[test]
    fn clean_answers_pay_the_debt_down_to_the_floor() {
        let mut misses = Misses::default();
        misses.missed('a');

        let weights: Vec<u32> = (0..4)
            .map(|_| {
                misses.recalled(&'a');
                misses.weight(&'a')
            })
            .collect();

        assert_eq!(weights, [3, 2, 1, 1]);
    }

    #[test]
    fn the_vocabulary_holds_one_button_per_distance() {
        assert_eq!(ANSWERS.len(), 12);
        assert_eq!(ANSWER_COUNT, ANSWERS.len());

        // judging compares semitones, so two buttons for one distance would be ambiguous
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

        assert_eq!(distances, (0..12).collect::<Vec<u8>>());
    }

    #[test]
    fn the_vocabulary_is_interval_all_with_the_tritones_merged() {
        let named: Vec<Interval> = ANSWERS
            .iter()
            .flat_map(|choice| std::iter::once(choice.interval).chain(choice.twin))
            .collect();

        // tripwire for a variant added to `Interval` but not here
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

    #[test]
    fn a_right_answer_scores_and_holds_until_the_next_prompt() {
        let mut score = Score::default();
        score.mark('a', true);

        assert_eq!((score.streak(), score.best()), (1, 1));
        assert_eq!(score.correct(), Some('a'));
        assert!(score.is_flashing());

        // The tail of the press that scored: neither a second point nor a wrong mark.
        score.mark('a', true);
        score.mark('b', false);
        assert_eq!(score.streak(), 1, "an answer during the flash counted");
        assert!(
            score.wrong().is_empty(),
            "an answer during the flash was marked wrong"
        );

        score.next_prompt();
        assert!(!score.is_flashing());
        assert_eq!(score.streak(), 1, "a new prompt broke the run");
    }

    #[test]
    fn wrong_answers_accumulate_once_each_and_break_only_the_run() {
        let mut score = Score::default();
        score.mark('a', true);
        score.next_prompt();
        score.mark('a', true);
        score.next_prompt();

        for wrong in ['x', 'y', 'x'] {
            score.mark(wrong, false);
        }

        assert_eq!(
            score.wrong(),
            ['x', 'y'],
            "a repeated wrong answer was marked twice"
        );
        assert_eq!((score.streak(), score.best()), (0, 2));

        score.break_streak();
        assert_eq!(score.best(), 2, "breaking the run touched the best");
    }
}
