//! The Home menu: the screens grouped into categories, the open one's entries in a second column.
//!
//! Which category is open is read off the focus, so the menu keeps no state of its own.

use iced::Element;

use super::focus::{Direction, FocusRow, FocusTarget};
use super::keys::{Accelerator, Fires};
use super::style::{BODY, INK, focus_ring, open_card_button};
use super::{Message, Screen, trainer_button, with_top_bar};

/// A block of the Home menu. Declared in draw order, which `index` relies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Scales,
    NotesAndIntervals,
    Chords,
    Tools,
}

/// One screen the menu opens.
pub(super) struct MenuItem {
    pub(super) label: &'static str,
    pub(super) caption: &'static str,
    pub(super) screen: Screen,
}

const SCALES: &[MenuItem] = &[
    MenuItem {
        label: "Scale Trainer",
        caption: "Explore and learn guitar scales",
        screen: Screen::ScaleTrainer,
    },
    MenuItem {
        label: "Walk the Scale",
        caption: "Find a scale's degrees in order, by ear or by eye",
        screen: Screen::WalkTheScale,
    },
];

const NOTES_AND_INTERVALS: &[MenuItem] = &[
    MenuItem {
        label: "Note Trainer",
        caption: "Build fretboard recall one pitch at a time",
        screen: Screen::NoteTrainer,
    },
    MenuItem {
        label: "Interval Trainer",
        caption: "Recognize distances from a tonal center",
        screen: Screen::IntervalTrainer,
    },
    MenuItem {
        label: "Play the Interval",
        caption: "Play the distance a lit position asks for",
        screen: Screen::PlayTheInterval,
    },
];

const CHORDS: &[MenuItem] = &[
    MenuItem {
        label: "Chord Library",
        caption: "Look up a chord and see where it sits",
        screen: Screen::ChordLibrary,
    },
    MenuItem {
        label: "CAGED",
        caption: "See one chord in all five places on the neck",
        screen: Screen::Caged,
    },
];

const TOOLS: &[MenuItem] = &[
    MenuItem {
        label: "Tuner",
        caption: "Tune by ear against what the app hears",
        screen: Screen::Tuner,
    },
    MenuItem {
        label: "Position Finder",
        caption: "Play a note and see where on the neck it was",
        screen: Screen::PositionFinder,
    },
];

impl Category {
    pub(super) const ALL: [Self; 4] = [
        Self::Scales,
        Self::NotesAndIntervals,
        Self::Chords,
        Self::Tools,
    ];

    fn index(self) -> usize {
        self as usize
    }

    fn label(self) -> &'static str {
        match self {
            Self::Scales => "Scales",
            Self::NotesAndIntervals => "Notes & Intervals",
            Self::Chords => "Chords",
            Self::Tools => "Tools",
        }
    }

    fn caption(self) -> &'static str {
        match self {
            Self::Scales => "See a scale on the neck, then walk it",
            Self::NotesAndIntervals => "Name, find and play pitches and distances",
            Self::Chords => "Voicings, and the CAGED cycle",
            Self::Tools => "Tune up, or find where a note was played",
        }
    }

    /// Never empty, so stepping into a category always has a first entry to land on.
    pub(super) fn entries(self) -> &'static [MenuItem] {
        match self {
            Self::Scales => SCALES,
            Self::NotesAndIntervals => NOTES_AND_INTERVALS,
            Self::Chords => CHORDS,
            Self::Tools => TOOLS,
        }
    }
}

/// The category whose entries are showing: the focused one, or the one holding the focused entry.
pub(super) fn open_category(focused: FocusTarget) -> Category {
    match focused {
        FocusTarget::HomeCategory(category) | FocusTarget::HomeEntry(category, _) => category,
        // a stale target, before the ring has landed on the menu
        _ => Category::Scales,
    }
}

/// The entry that opens `screen`, so coming back to Home lands on what was left.
pub(super) fn entry_for(screen: &Screen) -> Option<FocusTarget> {
    Category::ALL.iter().find_map(|&category| {
        category
            .entries()
            .iter()
            .position(|item| item.screen == *screen)
            .map(|index| FocusTarget::HomeEntry(category, index))
    })
}

/// Categories in the first column and the open category's entries in the second, as drawn.
pub(super) fn focus_grid(focused: FocusTarget) -> Vec<FocusRow> {
    let open = open_category(focused);
    let entries = open.entries().len();

    (0..Category::ALL.len().max(entries))
        .map(|row| {
            vec![
                Category::ALL
                    .get(row)
                    .map(|&c| FocusTarget::HomeCategory(c)),
                (row < entries).then_some(FocusTarget::HomeEntry(open, row)),
            ]
        })
        .collect()
}

/// Each category followed by its entries when open, so Tab descends into a category as it
/// reaches it rather than walking every category before any entry.
pub(super) fn tab_order(focused: FocusTarget) -> Vec<FocusTarget> {
    let open = open_category(focused);

    Category::ALL
        .iter()
        .flat_map(|&category| {
            let shown = if category == open {
                category.entries().len()
            } else {
                0
            };
            std::iter::once(FocusTarget::HomeCategory(category))
                .chain((0..shown).map(move |index| FocusTarget::HomeEntry(category, index)))
        })
        .collect()
}

/// Left and right cross between a category and its entries; up and down stay in a column.
///
/// Not the grid's geometry: the cell beside an entry is some other category, and landing on it
/// would swap out the column the focus just left.
pub(super) fn step(focused: FocusTarget, direction: Direction) -> FocusTarget {
    let delta: isize = match direction {
        Direction::Up => -1,
        Direction::Down => 1,
        Direction::Left | Direction::Right => 0,
    };

    match (focused, direction) {
        (FocusTarget::HomeCategory(category), Direction::Right) => {
            FocusTarget::HomeEntry(category, 0)
        }
        (FocusTarget::HomeEntry(category, _), Direction::Left) => {
            FocusTarget::HomeCategory(category)
        }
        (FocusTarget::HomeCategory(category), Direction::Up | Direction::Down) => category
            .index()
            .checked_add_signed(delta)
            .and_then(|next| Category::ALL.get(next))
            .map_or(focused, |&next| FocusTarget::HomeCategory(next)),
        (FocusTarget::HomeEntry(category, index), Direction::Up | Direction::Down) => index
            .checked_add_signed(delta)
            .filter(|&next| next < category.entries().len())
            .map_or(focused, |next| FocusTarget::HomeEntry(category, next)),
        _ => focused,
    }
}

/// Digits number whichever column the ring is in, so `2` then `1` opens the Note Trainer.
pub(super) fn accelerators(focused: FocusTarget) -> Vec<Accelerator> {
    let targets: Vec<(FocusTarget, &'static str)> = match focused {
        FocusTarget::HomeEntry(category, _) => category
            .entries()
            .iter()
            .enumerate()
            .map(|(index, item)| (FocusTarget::HomeEntry(category, index), item.label))
            .collect(),
        _ => Category::ALL
            .iter()
            .map(|&category| (FocusTarget::HomeCategory(category), category.label()))
            .collect(),
    };

    // `from_digit` runs out after nine
    (1..)
        .zip(targets)
        .filter_map(|(digit, (target, label))| {
            Some((
                char::from_digit(digit, 10)?,
                Fires::Widget(target),
                label.into(),
            ))
        })
        .collect()
}

pub(super) fn ui_home(focused: FocusTarget) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, row, text};

    let open = open_category(focused);

    let categories = column(Category::ALL.iter().map(|&category| {
        let button = trainer_button(category.label(), category.caption())
            .on_press(Message::OpenCategory(category));
        // marked even with the ring away in its entries, so the column says whose it is
        let button = if category == open {
            button.style(open_card_button)
        } else {
            button
        };
        focus_ring(button, focused == FocusTarget::HomeCategory(category))
    }))
    .spacing(12);

    let entries = column(open.entries().iter().enumerate().map(|(index, item)| {
        focus_ring(
            trainer_button(item.label, item.caption)
                .on_press(Message::Navigate(item.screen.clone())),
            focused == FocusTarget::HomeEntry(open, index),
        )
    }))
    .spacing(12);

    let hero = column![
        text("Trastea").size(56).color(INK),
        text("A focused guitar trainer for scales, intervals, and fretboard fluency.")
            .size(21)
            .color(BODY),
        row![
            // text("α").size(13).color(CANVAS),
            // text("desktop practice lab").size(13).color(INK)
        ]
        .spacing(8)
        .padding([6, 12])
    ]
    .spacing(16);

    let content = container(column![hero, row![categories, entries].spacing(24)].spacing(24))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([48, 64])
        .center_y(Length::Fill);

    with_top_bar("Trastea", content.into(), false, focused)
}

#[cfg(test)]
mod tests {
    use iced::keyboard;
    use iced::keyboard::key::Named;

    use super::*;
    use crate::ui::tests::{app_with_seed, every_screen, press_into, press_named};

    fn press(app: &mut crate::ui::App, key: &str) {
        press_into(app, key, keyboard::Modifiers::empty());
    }

    #[test]
    fn every_screen_but_home_is_in_exactly_one_category() {
        for screen in every_screen() {
            let homes = Category::ALL
                .iter()
                .flat_map(|category| category.entries())
                .filter(|item| item.screen == screen)
                .count();
            let expected = usize::from(screen != Screen::Home);

            assert_eq!(homes, expected, "{screen:?} is listed {homes} times");
        }
    }

    #[test]
    fn categories_are_declared_in_draw_order() {
        for (position, category) in Category::ALL.into_iter().enumerate() {
            assert_eq!(category.index(), position, "{category:?} is out of order");
            assert!(
                !category.entries().is_empty(),
                "{category:?} has no entries"
            );
        }
    }

    #[test]
    fn moving_through_the_categories_opens_each_in_turn() {
        let mut app = app_with_seed(0x4013);

        for category in Category::ALL {
            assert_eq!(app.focused, FocusTarget::HomeCategory(category));
            assert_eq!(open_category(app.focused), category);
            press_named(&mut app, Named::ArrowDown);
        }

        assert_eq!(
            app.focused,
            FocusTarget::HomeCategory(Category::Tools),
            "Down ran off the last category"
        );
    }

    #[test]
    fn right_steps_into_a_category_and_left_steps_back_out() {
        let mut app = app_with_seed(0x4014);
        press_named(&mut app, Named::ArrowDown);
        press_named(&mut app, Named::ArrowRight);
        assert_eq!(
            app.focused,
            FocusTarget::HomeEntry(Category::NotesAndIntervals, 0)
        );

        // past the last entry the ring stays, rather than crossing into another category
        for _ in 0..NOTES_AND_INTERVALS.len() {
            press_named(&mut app, Named::ArrowDown);
        }
        assert_eq!(
            app.focused,
            FocusTarget::HomeEntry(Category::NotesAndIntervals, 2)
        );

        press_named(&mut app, Named::ArrowLeft);
        assert_eq!(
            app.focused,
            FocusTarget::HomeCategory(Category::NotesAndIntervals),
            "Left landed on the category beside the entry, not its own"
        );
    }

    #[test]
    fn escape_leaves_the_entries_before_it_does_anything_else() {
        let mut app = app_with_seed(0x4015);
        app.focused = FocusTarget::HomeEntry(Category::Chords, 1);

        press_named(&mut app, Named::Escape);

        assert_eq!(app.screen, Screen::Home);
        assert_eq!(app.focused, FocusTarget::HomeCategory(Category::Chords));
    }

    #[test]
    fn enter_on_a_category_steps_into_it_and_enter_on_an_entry_opens_it() {
        let mut app = app_with_seed(0x4016);
        app.focused = FocusTarget::HomeCategory(Category::Tools);

        press_named(&mut app, Named::Enter);
        assert_eq!(app.screen, Screen::Home, "a category is not a screen");
        assert_eq!(app.focused, FocusTarget::HomeEntry(Category::Tools, 0));

        press_named(&mut app, Named::Enter);
        assert_eq!(app.screen, Screen::Tuner);
    }

    #[test]
    fn two_digits_reach_every_screen() {
        for (c, category) in Category::ALL.into_iter().enumerate() {
            for (e, item) in category.entries().iter().enumerate() {
                let mut app = app_with_seed(0x4017);

                press(&mut app, &(c + 1).to_string());
                assert_eq!(app.focused, FocusTarget::HomeEntry(category, 0));

                press(&mut app, &(e + 1).to_string());
                assert_eq!(app.screen, item.screen, "{}{} missed", c + 1, e + 1);
            }
        }
    }

    #[test]
    fn digits_number_the_column_the_ring_is_in() {
        let labels = |focused| -> Vec<String> {
            accelerators(focused)
                .into_iter()
                .map(|(_, _, label)| label.into_owned())
                .collect()
        };

        assert_eq!(
            labels(FocusTarget::HomeCategory(Category::Chords)),
            ["Scales", "Notes & Intervals", "Chords", "Tools"]
        );
        assert_eq!(
            labels(FocusTarget::HomeEntry(Category::Chords, 0)),
            ["Chord Library", "CAGED"]
        );
    }

    #[test]
    fn a_pressed_category_takes_the_ring() {
        let mut app = app_with_seed(0x4018);

        let _ = app.update(Message::OpenCategory(Category::Chords));

        assert_eq!(app.screen, Screen::Home);
        assert_eq!(app.focused, FocusTarget::HomeCategory(Category::Chords));
    }

    #[test]
    fn coming_back_lands_on_the_entry_that_was_left() {
        let mut app = app_with_seed(0x4019);
        press(&mut app, "2");
        press(&mut app, "3");
        assert_eq!(app.screen, Screen::PlayTheInterval);

        press_named(&mut app, Named::Escape);

        assert_eq!(app.screen, Screen::Home);
        assert_eq!(
            app.focused,
            FocusTarget::HomeEntry(Category::NotesAndIntervals, 2)
        );
    }

    #[test]
    fn tab_descends_into_a_category_as_it_reaches_it() {
        let mut app = app_with_seed(0x4020);
        let mut visited = vec![app.focused];

        for _ in 0..Category::ALL.len() + SCALES.len() {
            press_named(&mut app, Named::Tab);
            visited.push(app.focused);
        }

        assert_eq!(
            visited[..4],
            [
                FocusTarget::HomeCategory(Category::Scales),
                FocusTarget::HomeEntry(Category::Scales, 0),
                FocusTarget::HomeEntry(Category::Scales, 1),
                FocusTarget::HomeCategory(Category::NotesAndIntervals),
            ]
        );
        assert_eq!(
            visited[4],
            FocusTarget::HomeEntry(Category::NotesAndIntervals, 0)
        );
    }
}
