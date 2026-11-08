//! Key press to message, the per-screen accelerator table, and the `?` overlay drawn from it.

use std::borrow::Cow;
use std::time::Duration;

use iced::keyboard::key::Named;
use iced::{Background, Border, Color, Element, Shadow, keyboard};

use super::focus::FocusTarget;
use super::style::{
    BODY, CANVAS, CANVAS_SOFT_2, INK, KEYCAP_ARROWS, KEYCAP_BACKTAB, KEYCAP_FONT, KEYCAP_TAB, MUTE,
    card_container,
};
use super::{App, Message, Notation, Screen, WalkTheScale, home, note_trainer, walk_the_scale};

/// The modifiers that suppress a character binding.
///
/// Shift is absent because `Key::Character` already reports the shifted character, and `?` is Shift+`/`.
pub(super) const COMMAND_MODIFIERS: keyboard::Modifiers = keyboard::Modifiers::LOGO
    .union(keyboard::Modifiers::CTRL)
    .union(keyboard::Modifiers::ALT);

pub(super) fn translate_key(key: keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Message> {
    match key.as_ref() {
        keyboard::Key::Named(Named::Escape | Named::Backspace) => Some(Message::GoBack),
        keyboard::Key::Named(Named::Tab) if modifiers.shift() => Some(Message::FocusPrevious),
        keyboard::Key::Named(Named::Tab) => Some(Message::FocusNext),
        keyboard::Key::Named(Named::Enter) => Some(Message::ActivateFocused),
        keyboard::Key::Named(Named::Space) => Some(Message::ActivateFocused),
        keyboard::Key::Named(Named::ArrowUp) => Some(Message::FocusUp),
        keyboard::Key::Named(Named::ArrowDown) => Some(Message::FocusDown),
        keyboard::Key::Named(Named::ArrowLeft) => Some(Message::FocusLeft),
        keyboard::Key::Named(Named::ArrowRight) => Some(Message::FocusRight),
        // before the general character arm, which would swallow it; works on every screen
        keyboard::Key::Character("?") if !modifiers.intersects(COMMAND_MODIFIERS) => {
            Some(Message::ToggleHelp)
        }
        // the one key carved out of `COMMAND_MODIFIERS`
        keyboard::Key::Character("k") if modifiers.contains(keyboard::Modifiers::CTRL) => {
            Some(Message::OpenSearch)
        }
        keyboard::Key::Character(c) if !modifiers.intersects(COMMAND_MODIFIERS) => character_key(c),
        _ => None,
    }
}

/// Maps an unmodified character key onto a message.
///
/// Motions win over accelerators. Other single characters go to the active screen to
/// resolve; multi-character payloads (dead keys, IME output) stop here.
fn character_key(c: &str) -> Option<Message> {
    if let Some(motion) = vim_motion(c) {
        return Some(motion);
    }

    let mut chars = c.chars();
    let first = chars.next()?;

    chars.next().is_none().then_some(Message::Accelerate(first))
}

fn vim_motion(c: &str) -> Option<Message> {
    match c {
        "h" => Some(Message::FocusLeft),
        "j" => Some(Message::FocusDown),
        "k" => Some(Message::FocusUp),
        "l" => Some(Message::FocusRight),
        _ => None,
    }
}

/// A key, what it fires, and how the overlay names it.
///
/// A `Cow` because a few labels report a setting, like `timer: 5:00`, and are built by `format!`.
pub(super) type Accelerator = (char, Fires, Cow<'static, str>);

/// What an accelerator fires: a widget on the screen, or a message no widget stands behind.
/// A widget target lets the table be checked against the focus grid.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Fires {
    Widget(FocusTarget),
    Message(Message),
}

impl App {
    /// The accelerators the current screen claims. A key absent here is inert on that screen.
    pub(super) fn accelerators(&self) -> Vec<Accelerator> {
        // Home's digits depend on which column the ring is in, which no other table needs
        if self.screen == Screen::Home {
            return home::accelerators(self.focused);
        }

        // `d` means the direction while pressing and the input while playing
        if self.screen == Screen::NoteTrainer {
            return note_trainer_accelerators(self.note_trainer.answering());
        }

        // the two chord screens keep their own notation, and `i` names whichever comes next
        let notation = match self.screen {
            Screen::Caged => self.caged.notation(),
            _ => self.chord_library.notation(),
        };

        accelerators_for(&self.screen, notation, &self.walk_the_scale)
    }
}

/// One key per notation pill, and `i` for whichever comes after the one in force.
fn notation_keys(
    notation: Notation,
    pill: fn(usize) -> FocusTarget,
) -> impl Iterator<Item = Accelerator> {
    let keys = Notation::ALL
        .into_iter()
        .enumerate()
        .map(move |(index, choice)| {
            let (key, label) = match choice {
                Notation::Notes => ('n', "note names"),
                Notation::Intervals => ('d', "degrees"),
                Notation::Fingers => ('f', "fingers"),
            };
            (key, Fires::Widget(pill(index)), label.into())
        });

    keys.chain([(
        'i',
        Fires::Widget(pill(notation.next_index())),
        "cycle the three".into(),
    )])
}

pub(super) fn accelerators_for(
    screen: &Screen,
    notation: Notation,
    drill: &WalkTheScale,
) -> Vec<Accelerator> {
    match screen {
        // the table at rest, with the ring on the categories
        Screen::Home => home::accelerators(FocusTarget::HomeCategory(home::Category::Scales)),
        Screen::ScaleTrainer => [
            (
                'r',
                Fires::Widget(FocusTarget::RerollScale),
                "new scale".into(),
            ),
            (
                'i',
                Fires::Widget(FocusTarget::NotationToggle),
                "interval notation".into(),
            ),
        ]
        .into_iter()
        .chain(root_steps())
        .collect(),
        Screen::WalkTheScale => {
            let mut keys: Vec<Accelerator> = vec![(
                'r',
                Fires::Widget(FocusTarget::RerollDrillScale),
                "new scale".into(),
            )];
            keys.extend(root_steps());

            if !drill.is_running() {
                keys.push((
                    'p',
                    Fires::Widget(FocusTarget::RestartDrill),
                    "another run".into(),
                ));
            }

            if drill.is_running() {
                keys.push((
                    'm',
                    Fires::Widget(FocusTarget::ScaleAnswering),
                    match drill.answering() {
                        walk_the_scale::Answering::Pressing => "answer: press".into(),
                        walk_the_scale::Answering::Playing { .. } => "answer: play".into(),
                    },
                ));
                keys.push((
                    't',
                    Fires::Widget(FocusTarget::ScaleDuration),
                    Cow::Owned(format!("timer: {}", duration_label(drill.duration()))),
                ));

                // only while playing: in pressing mode the neck is where the answer is given
                if matches!(drill.answering(), walk_the_scale::Answering::Playing { .. }) {
                    keys.push((
                        'n',
                        Fires::Widget(FocusTarget::ScaleNeck),
                        if drill.neck_hidden() {
                            "neck: hidden".into()
                        } else {
                            "neck: shown".into()
                        },
                    ));
                    keys.push((
                        'd',
                        Fires::Widget(FocusTarget::InputDevice),
                        "input device".into(),
                    ));
                }
            }

            keys
        }
        // the table at rest, which opens pressing
        Screen::NoteTrainer => note_trainer_accelerators(note_trainer::Answering::Pressing),
        Screen::IntervalTrainer => vec![
            (
                'r',
                Fires::Widget(FocusTarget::SkipIntervalPrompt),
                "skip this interval".into(),
            ),
            (
                'd',
                Fires::Widget(FocusTarget::IntervalDirectionToggle),
                "swap direction".into(),
            ),
        ],
        Screen::PlayTheInterval => vec![
            (
                'r',
                Fires::Widget(FocusTarget::SkipPlayPrompt),
                "skip this interval".into(),
            ),
            (
                'd',
                Fires::Widget(FocusTarget::InputDevice),
                "input device".into(),
            ),
        ],
        // `/` is the same physical key as `?`, distinguished by shift
        Screen::ChordLibrary => [('/', Fires::Widget(FocusTarget::SearchBox), "search".into())]
            .into_iter()
            .chain(notation_keys(notation, FocusTarget::NotationChoice))
            // overlay only: `accelerate` takes `g` and `G` before this table, since `gg` is two keys
            .chain([(
                'G',
                Fires::Widget(FocusTarget::ChordList),
                "last chord, gg for the first".into(),
            )])
            .collect(),
        Screen::Caged => [(
            'r',
            Fires::Widget(FocusTarget::RerollCagedRoot),
            "new root".into(),
        )]
        .into_iter()
        .chain(notation_keys(notation, FocusTarget::CagedNotation))
        .chain(root_steps())
        .collect(),
        Screen::Tuner => vec![
            (
                'd',
                Fires::Widget(FocusTarget::InputDevice),
                "input device".into(),
            ),
            (
                'i',
                Fires::Widget(FocusTarget::TunerSpellingToggle),
                "flat names".into(),
            ),
        ],
        Screen::PositionFinder => vec![
            (
                'd',
                Fires::Widget(FocusTarget::InputDevice),
                "input device".into(),
            ),
            (
                'c',
                Fires::Widget(FocusTarget::Calibrate),
                "calibrate".into(),
            ),
            (
                'g',
                Fires::Widget(FocusTarget::PositionGroups),
                "show the groups".into(),
            ),
        ],
    }
}

/// Brackets rather than arrows: the root glyph is neither a focus stop nor a closable list.
fn root_steps() -> [Accelerator; 2] {
    [
        (
            '[',
            Fires::Message(Message::StepRoot(-1)),
            "root down".into(),
        ),
        (']', Fires::Message(Message::StepRoot(1)), "root up".into()),
    ]
}

fn note_trainer_accelerators(answering: note_trainer::Answering) -> Vec<Accelerator> {
    let d = match answering {
        note_trainer::Answering::Pressing => (
            'd',
            Fires::Widget(FocusTarget::DrillDirectionToggle),
            "swap direction".into(),
        ),
        note_trainer::Answering::Playing => (
            'd',
            Fires::Widget(FocusTarget::InputDevice),
            "input device".into(),
        ),
    };

    vec![
        (
            'r',
            Fires::Widget(FocusTarget::SkipPrompt),
            "skip this note".into(),
        ),
        d,
        (
            'a',
            Fires::Widget(FocusTarget::PoolToggle),
            "include accidentals".into(),
        ),
        (
            'm',
            Fires::Widget(FocusTarget::NoteAnswering),
            match answering {
                note_trainer::Answering::Pressing => "answer: press".into(),
                note_trainer::Answering::Playing => "answer: play".into(),
            },
        ),
    ]
}

/// A duration as the overlay says it: `off`, or minutes and seconds.
pub(super) fn duration_label(duration: Option<Duration>) -> String {
    match duration {
        None => "off".to_owned(),
        Some(total) => {
            let seconds = total.as_secs();
            format!("{}:{:02}", seconds / 60, seconds % 60)
        }
    }
}

/// One piece of a key hint: a word in the body font, or a keycap in `KEYCAP_FONT`.
enum KeyPart {
    Word(&'static str),
    /// A run of keycaps, so `↑ ↓ ← →` is not spaced as four parts.
    Cap(&'static str),
    /// The space between the two ways of doing one thing, on one row.
    Gap,
}

/// The keys that mean the same thing on every screen.
///
/// Every glyph here must be in the embedded keycap subset; Esc and the like stay words.
const NAVIGATION_KEYS: [(&[KeyPart], &str); 5] = {
    use KeyPart::{Cap, Gap, Word};

    [
        (
            &[Cap(KEYCAP_TAB), Gap, Cap(KEYCAP_BACKTAB)],
            "next / previous",
        ),
        (
            &[Cap(KEYCAP_ARROWS), Gap, Word("k j h l")],
            "move the focus ring",
        ),
        (&[Word("Enter"), Gap, Word("Space")], "activate"),
        (&[Word("Esc"), Gap, Word("Backspace")], "back"),
        (&[Word("?")], "this list"),
    ]
};

/// Larger than the words beside it, since the arrows come from a maths font's optical size.
const HELP_KEY_SIZE: f32 = 15.0;
const HELP_KEYCAP_SIZE: f32 = 17.0;
const HELP_KEY_GAP: f32 = 12.0;

fn key_hint(parts: &'static [KeyPart]) -> Element<'static, Message> {
    use iced::widget::{Space, row, text};

    row(parts.iter().map(|part| match part {
        KeyPart::Word(word) => text(*word).size(HELP_KEY_SIZE).color(INK).into(),
        KeyPart::Cap(run) => text(*run)
            .font(KEYCAP_FONT)
            .size(HELP_KEYCAP_SIZE)
            .color(INK)
            .into(),
        KeyPart::Gap => Space::new().width(HELP_KEY_GAP).into(),
    }))
    .spacing(3)
    .align_y(iced::Alignment::Center)
    .into()
}

/// The `?` overlay: a scrim over the live screen and a card of the keys that work on it.
///
/// Nothing in it is focusable, so the focus system needs no notion of a modal layer.
pub(super) fn ui_help_overlay(claimed: Vec<Accelerator>) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, text};

    let navigation = NAVIGATION_KEYS
        .into_iter()
        .map(|(parts, description)| (key_hint(parts), Cow::Borrowed(description)));

    let has_accelerators = !claimed.is_empty();
    let screen_keys = claimed.into_iter().map(|(key, _, label)| {
        (
            text(key.to_string()).size(HELP_KEY_SIZE).color(INK).into(),
            label,
        )
    });

    let mut card = column![
        text("Keys").size(26).color(INK),
        help_section("Anywhere", navigation),
    ]
    .spacing(22);

    card = card.push(if has_accelerators {
        help_section("On this screen", screen_keys)
    } else {
        text("No shortcuts on this screen.")
            .size(15)
            .color(MUTE)
            .into()
    });

    let panel = container(card)
        .padding([28, 32])
        .width(440)
        .style(help_panel_container);

    container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(scrim_container)
        .into()
}

fn help_section(
    title: &'static str,
    rows: impl Iterator<Item = (Element<'static, Message>, Cow<'static, str>)>,
) -> Element<'static, Message> {
    use iced::widget::{column, container, row, text};

    let entries = column(rows.map(|(keys, description)| {
        row![
            container(keys).width(150),
            text(description.into_owned())
                .size(HELP_KEY_SIZE)
                .color(BODY),
        ]
        .spacing(14)
        .into()
    }))
    .spacing(9);

    column![text(title).size(13).color(MUTE), entries]
        .spacing(11)
        .into()
}

/// The help card, on `CANVAS_SOFT_2` so it stands off the `CANVAS_SOFT` cards behind it.
fn help_panel_container(theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(CANVAS_SOFT_2)),
        ..card_container(theme)
    }
}

fn scrim_container(_theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        text_color: Some(INK),
        background: Some(Background::Color(Color { a: 0.86, ..CANVAS })),
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::style::KEYCAP_BACK;
    use crate::ui::tests::{app_on, every_screen, press};

    #[test]
    fn vim_motions_move_the_focus_ring() {
        let none = keyboard::Modifiers::empty();

        assert!(matches!(press("h", none), Some(Message::FocusLeft)));
        assert!(matches!(press("j", none), Some(Message::FocusDown)));
        assert!(matches!(press("k", none), Some(Message::FocusUp)));
        assert!(matches!(press("l", none), Some(Message::FocusRight)));
    }

    #[test]
    fn modified_vim_letters_are_not_motions() {
        for modifiers in [
            keyboard::Modifiers::LOGO,
            keyboard::Modifiers::CTRL,
            keyboard::Modifiers::ALT,
        ] {
            assert!(press("h", modifiers).is_none(), "{modifiers:?}+h");
        }
    }

    #[test]
    fn command_modifiers_suppress_character_keys() {
        for modifiers in [
            keyboard::Modifiers::LOGO,
            keyboard::Modifiers::CTRL,
            keyboard::Modifiers::ALT,
        ] {
            assert!(press("r", modifiers).is_none(), "{modifiers:?}+r");
            // `k` named so the Ctrl exception cannot silently widen to the whole key
            if modifiers != keyboard::Modifiers::CTRL {
                assert!(press("k", modifiers).is_none(), "{modifiers:?}+k");
            }
        }
    }

    /// An accelerator can only name a widget the screen actually has.
    #[test]
    fn every_accelerator_targets_a_widget_on_its_screen() {
        for screen in every_screen() {
            let reachable = app_on(screen.clone()).focusables();

            // every notation, since the library's `i` names whichever button comes next
            for (index, notation) in Notation::ALL.into_iter().enumerate() {
                let mut app = app_on(screen.clone());
                app.chord_library.set_notation(index);
                app.caged.set_notation(index);

                for (key, fires, label) in app.accelerators() {
                    let Fires::Widget(target) = fires else {
                        continue;
                    };
                    assert!(
                        reachable.contains(&target),
                        "{screen:?} under {notation:?} binds {key:?} ({label}) to \
                         {target:?}, which is not on it"
                    );
                }
            }
        }
    }

    /// A keycap outside the ranges `just fonts` passes to `--unicodes` would fall back to a
    /// system font; nothing at runtime can read the font, so the ranges are written out here.
    #[test]
    fn every_mark_is_one_the_embedded_subset_carries() {
        let carried = |glyph: char| {
            matches!(
                glyph,
                '\u{2190}'..='\u{2193}' | '\u{21E4}'..='\u{21E5}'
            )
        };

        // the back arrow shares the font, though it is not in `NAVIGATION_KEYS`
        for glyph in KEYCAP_BACK.chars() {
            assert!(carried(glyph), "{glyph:?} is not in the subset");
        }

        let caps = NAVIGATION_KEYS
            .into_iter()
            .flat_map(|(parts, description)| parts.iter().map(move |part| (part, description)));

        for (part, description) in caps {
            let KeyPart::Cap(run) = part else { continue };

            for glyph in run.chars().filter(|glyph| !glyph.is_whitespace()) {
                assert!(
                    carried(glyph),
                    "{glyph:?} on the {description:?} row is not in the subset"
                );
            }
        }

        for glyph in KEYCAP_BACK.chars() {
            assert!(carried(glyph), "the back arrow is not in the subset");
        }
    }

    #[test]
    fn no_screen_binds_the_same_accelerator_twice() {
        for screen in every_screen() {
            let mut keys: Vec<char> =
                accelerators_for(&screen, Notation::Notes, &WalkTheScale::new())
                    .iter()
                    .map(|(k, _, _)| *k)
                    .collect();
            let count = keys.len();
            keys.sort_unstable();
            keys.dedup();

            assert_eq!(keys.len(), count, "{screen:?} binds a key twice");
        }
    }

    #[test]
    fn unbound_named_keys_produce_no_message() {
        for named in [Named::F1, Named::Home, Named::PageDown, Named::Insert] {
            assert!(
                translate_key(keyboard::Key::Named(named), keyboard::Modifiers::empty()).is_none(),
                "{named:?} is bound"
            );
        }
    }

    #[test]
    fn adding_vim_motions_did_not_shadow_the_named_keys() {
        let escape = keyboard::Key::Named(Named::Escape);
        let modifiers = keyboard::Modifiers::empty();

        assert!(matches!(
            translate_key(escape, modifiers),
            Some(Message::GoBack)
        ));
    }
}
