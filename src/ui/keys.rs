//! The keyboard: turning a key press into a message, the per-screen table of single-key
//! accelerators, and the `?` overlay that is drawn from that table so it cannot disagree with it.

use std::borrow::Cow;
use std::time::Duration;

use iced::keyboard::key::Named;
use iced::{Background, Border, Color, Element, Shadow, keyboard};

use super::focus::FocusTarget;
use super::style::{
    BODY, CANVAS, CANVAS_SOFT_2, INK, KEYCAP_ARROWS, KEYCAP_BACKTAB, KEYCAP_FONT, KEYCAP_TAB, MUTE,
    card_container,
};
use super::{
    App, HOME_MENU, Message, Notation, Screen, WalkTheScale, chord_library, walk_the_scale,
};

/// The modifiers that suppress a character binding.
///
/// Shift is deliberately absent. `Key::Character` already reports the shifted character,
/// so Shift has done its work by the time the key arrives and testing for it again would
/// make `?` — Shift+`/` on most layouts — unreachable. Capital letters stay unbound because
/// nothing claims `H`, not because a guard stopped the lookup.
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
        // Ahead of the general character arm below, which would otherwise bind `?` first
        // and swallow it — the guard means the compiler cannot warn about the shadowing.
        // `?` is listed here rather than in a screen's accelerators because it has to work
        // everywhere, including on screens that declare none of their own.
        keyboard::Key::Character("?") if !modifiers.intersects(COMMAND_MODIFIERS) => {
            Some(Message::ToggleHelp)
        }
        // The mirror of the arm above: that one is a character admitted despite coming
        // before the general arm, this one is a character admitted despite a command
        // modifier being held. `COMMAND_MODIFIERS` is not relaxed — exactly one key is
        // carved out of it, which is why `command_modifiers_suppress_character_keys` still
        // covers every other letter under every modifier.
        keyboard::Key::Character("k") if modifiers.contains(keyboard::Modifiers::CTRL) => {
            Some(Message::OpenSearch)
        }
        keyboard::Key::Character(c) if !modifiers.intersects(COMMAND_MODIFIERS) => character_key(c),
        _ => None,
    }
}

/// Maps an unmodified character key onto a message.
///
/// Motions win over accelerators, so `h` can never be claimed as one. Everything else that
/// is a single character is forwarded for the active screen to resolve — this function
/// cannot tell whether a character is bound, because binding is per-screen and it has no
/// screen. Multi-character payloads (dead keys, IME output) are not accelerator material
/// and stop here.
fn character_key(c: &str) -> Option<Message> {
    if let Some(motion) = vim_motion(c) {
        return Some(motion);
    }

    let mut chars = c.chars();
    let first = chars.next()?;

    chars.next().is_none().then_some(Message::Accelerate(first))
}

/// Maps the vim motion keys onto the same focus moves the arrow keys make.
fn vim_motion(c: &str) -> Option<Message> {
    match c {
        "h" => Some(Message::FocusLeft),
        "j" => Some(Message::FocusDown),
        "k" => Some(Message::FocusUp),
        "l" => Some(Message::FocusRight),
        _ => None,
    }
}

/// A key, what it fires, and how the overlay names it — so a new accelerator documents itself.
///
/// The label is a `Cow` rather than a `&'static str` because three of the drill's read a
/// setting — `timer: 5:00` is no literal. It is not a `String` either: that would cost every
/// one of the fixed labels an allocation and a `.to_owned()` to serve the three that report.
/// And it cannot be a `&'a str` borrowed from `App`: the reported labels are built by
/// `format!`, so a borrow would point at a temporary owned by the function returning it —
/// `error[E0515]: cannot return value referencing temporary value`. `Cow::Borrowed` for the
/// literals, `Cow::Owned` for the three, and one type covering both.
pub(super) type Accelerator = (char, Fires, Cow<'static, str>);

/// What an accelerator fires: a widget on the screen, skipping the walk to it, or a message
/// for an action no widget stands behind.
///
/// Two cases rather than a message for everything, because a widget target is what lets the
/// table be checked against the focus grid — every key that says it fires a widget has to
/// name one the screen actually has.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Fires {
    Widget(FocusTarget),
    Message(Message),
}

impl App {
    /// The accelerators the current screen claims. A key absent here is inert on that
    /// screen, which is what keeps `r` from rerolling an invisible scale from the Home menu.
    ///
    /// A method rather than a free function of `&Screen`, for the reason the focus
    /// functions in `focus.rs` are: a screen's shape is no longer decided by which screen it is.
    /// `i` on the library has to name *the next* notation button, and which one that is
    /// depends on the notation now in force.
    pub(super) fn accelerators(&self) -> Vec<Accelerator> {
        accelerators_for(
            &self.screen,
            self.chord_library.notation(),
            &self.walk_the_scale,
        )
    }
}

pub(super) fn accelerators_for(
    screen: &Screen,
    notation: Notation,
    drill: &WalkTheScale,
) -> Vec<Accelerator> {
    match screen {
        // Numbered by position, so the menu order is the key order. `from_digit` runs out
        // after nine, which is the point at which a menu needs more than digits anyway.
        Screen::Home => HOME_MENU
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                let key = char::from_digit(index as u32 + 1, 10)?;
                Some((
                    key,
                    Fires::Widget(FocusTarget::HomeMenuItem(index)),
                    item.label.into(),
                ))
            })
            .collect(),
        Screen::ScaleTrainer => vec![
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
        ],
        // `r` means here what it means on the Scale Trainer. The rest belong to the
        // drill, and three of them report the setting they hold rather than only naming what
        // the key does, which is why the label is a `Cow` and this arm is built rather than
        // listed.
        Screen::WalkTheScale => {
            // `[` and `]` rather than the arrows: the ring still has sixteen kinds to walk on
            // this screen, and every other widget that claims the motion keys is either a focus
            // stop a Tab can leave or a list `Esc` closes. The root display is neither, so the
            // brackets fire a message rather than a widget.
            let mut keys: Vec<Accelerator> = vec![
                (
                    'r',
                    Fires::Widget(FocusTarget::RerollDrillScale),
                    "new scale".into(),
                ),
                (
                    '[',
                    Fires::Message(Message::StepDrillRoot(-1)),
                    "root down".into(),
                ),
                (
                    ']',
                    Fires::Message(Message::StepDrillRoot(1)),
                    "root up".into(),
                ),
            ];

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

                // Offered only while playing: in pressing mode the neck is the surface the
                // answer is given on, so a key that took it away would leave the drill with
                // nothing to answer with.
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
        // `r` means the same here as on the scale trainer — replace what is on screen.
        // `d` and `a` are free: `h j k l` are claimed as motions before accelerators are
        // consulted, and neither `d` nor `a` is one.
        Screen::NoteTrainer => vec![
            (
                'r',
                Fires::Widget(FocusTarget::SkipPrompt),
                "skip this note".into(),
            ),
            (
                'd',
                Fires::Widget(FocusTarget::DrillDirectionToggle),
                "swap direction".into(),
            ),
            (
                'a',
                Fires::Widget(FocusTarget::PoolToggle),
                "include accidentals".into(),
            ),
        ],
        // `r` and `d` mean here what they mean on the Note Trainer — replace what is on
        // screen, and swap which way the drill runs. There is no `a`: this screen has no
        // pool to widen.
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
        // `r` means what it means everywhere — replace what is on screen. `d` means what
        // it means on the Tuner, which is the other screen that listens; there is no
        // direction to swap here, since an interval cannot be named by playing it.
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
        // `/` is the same physical key as `?`, distinguished by shift: search, and help.
        //
        // One key per mode as well as the cycling one. With three modes visible, a learner
        // who can see `fingers` should be able to reach it directly rather than pressing
        // `i` until it comes round; `i` stays because that is what it means on the scale
        // trainer, and it is the key a hand already knows.
        Screen::ChordLibrary => {
            let next = chord_library::NOTATIONS
                .iter()
                .position(|&at| at == notation)
                .map_or(0, |at| (at + 1) % chord_library::NOTATIONS.len());

            vec![
                ('/', Fires::Widget(FocusTarget::SearchBox), "search".into()),
                (
                    'n',
                    Fires::Widget(FocusTarget::NotationChoice(0)),
                    "note names".into(),
                ),
                (
                    'd',
                    Fires::Widget(FocusTarget::NotationChoice(1)),
                    "degrees".into(),
                ),
                (
                    'f',
                    Fires::Widget(FocusTarget::NotationChoice(2)),
                    "fingers".into(),
                ),
                (
                    'i',
                    Fires::Widget(FocusTarget::NotationChoice(next)),
                    "cycle the three".into(),
                ),
                // Here for the overlay's sake. `accelerate` takes `g` and `G` before it
                // consults this table, because `gg` is two keys and a table entry is one —
                // so the target is the widget they move rather than one they activate, and
                // `gg` is named in the label because it has no row of its own.
                (
                    'G',
                    Fires::Widget(FocusTarget::ChordList),
                    "last chord, gg for the first".into(),
                ),
            ]
        }
        // `r` and `i` mean here what they mean on the Scale Trainer — replace what is on
        // screen, and switch what the dots say. There is no `d`: this screen has no direction
        // to swap, and the library's `d` is a notation it does not offer.
        Screen::Caged => vec![
            (
                'r',
                Fires::Widget(FocusTarget::RerollCagedRoot),
                "new root".into(),
            ),
            (
                'i',
                Fires::Widget(FocusTarget::NotationToggle),
                "interval notation".into(),
            ),
        ],
        // `d` is the device list. It means something different again here — the library's
        // `d` picks degrees and a trainer's swaps direction — which is the arrangement a
        // per-screen table exists to allow. `i` keeps its meaning: switch how a note is
        // written, which here is the sharp or flat name of the pitch being heard.
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
    }
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
///
/// Two fonts, so one `text` widget cannot carry both — the same split `chord_symbol` and
/// `interval_label` already make for accidentals, applied here to the modifier keys.
enum KeyPart {
    Word(&'static str),
    /// A run of keycaps rather than one, because `↑ ↓ ← →` is written as a single mark in
    /// the row and splitting it into four parts would put the row's spacing between them.
    Cap(&'static str),
    /// The space between the two ways of doing one thing, on one row.
    Gap,
}

/// The keys that mean the same thing on every screen.
///
/// Every arrow is a keycap; the rest stay words because a keycap glyph for them is either
/// unavailable — there is no Esc key in Unicode outside the symbol fonts — or no clearer
/// than the word, and every glyph here has to be paid for in the embedded subset.
///
/// Written out rather than derived from `translate_key`. These are structural and will not
/// move, so the cost of them drifting is near zero — unlike the per-screen accelerators
/// beneath them in the overlay, which grow with every feature and so are read from the same
/// declaration that dispatches them.
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

/// The help overlay's key column. A keycap is set larger than the words beside it, for the
/// reason `control_label` documents: the arrows are drawn against a maths font's optical
/// size rather than a text face's cap height.
const HELP_KEY_SIZE: f32 = 15.0;
const HELP_KEYCAP_SIZE: f32 = 17.0;
const HELP_KEY_GAP: f32 = 12.0;

/// A key hint as a row: keycaps in `KEYCAP_FONT`, everything else in the body font.
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
/// Takes the screen rather than reading the history, so what it lists is always what is
/// actually behind it. Nothing in here is focusable — it contributes no entry to
/// `focus_grid`, which is why the focus system needs no notion of a modal layer.
pub(super) fn ui_help_overlay(claimed: Vec<Accelerator>) -> Element<'static, Message> {
    use iced::Length;
    use iced::widget::{column, container, text};

    let navigation = NAVIGATION_KEYS
        .into_iter()
        .map(|(parts, description)| (key_hint(parts), Cow::Borrowed(description)));

    let has_accelerators = !claimed.is_empty();
    // Accelerators are single characters typed as themselves, so no row of them ever needs
    // a keycap — `key_hint` would be a `Word` of one letter.
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

/// One labelled group of key rows in the help overlay.
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

/// The help card itself: `card_container`'s shape and chrome on the next surface up.
///
/// Cards on the screen behind already sit on `CANVAS_SOFT`, so reusing that here would put
/// the panel at the same tone as what it floats over and leave the scrim doing all the
/// separating. `CANVAS_SOFT_2` is the tone `DESIGN.md` assigns to menus and other surfaces
/// that float above cards, which is what this is.
///
/// The shadow stays a single drop because `container::Style` holds one `Shadow`; the
/// document's Level 5 modal treatment stacks three, which iced cannot express — a limit
/// every card in the app already lives with.
fn help_panel_container(theme: &iced::Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(CANVAS_SOFT_2)),
        ..card_container(theme)
    }
}

/// The dimming layer behind the help card. Opaque enough to push the screen back without
/// hiding it, so the overlay reads as covering the page rather than replacing it.
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
            // `k` too, everywhere except the one combination carved out below. The letter
            // is named here so the exception cannot silently widen to the whole key.
            if modifiers != keyboard::Modifiers::CTRL {
                assert!(press("k", modifiers).is_none(), "{modifiers:?}+k");
            }
        }
    }

    /// Guards the promise that scoping is structural: an accelerator can only name a widget
    /// the screen actually has, so reshaping a focus grid cannot strand one.
    #[test]
    fn every_accelerator_targets_a_widget_on_its_screen() {
        for screen in every_screen() {
            let reachable = app_on(screen.clone()).focusables();

            // Every notation, not just the default: the library's `i` names whichever
            // button comes next, so its target moves as the setting does and each of
            // those has to be a widget the screen actually has.
            for (index, &notation) in chord_library::NOTATIONS.iter().enumerate() {
                let mut app = app_on(screen.clone());
                app.chord_library.set_notation(index);

                // A key that fires a message has no widget to be missing; only the ones that
                // say they fire a widget can be wrong about it.
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

    /// A trainer added to `HOME_MENU` must arrive with its digit already attached. Without
    /// this, a fourth entry would silently be the only one you cannot reach by number.
    #[test]
    fn every_home_menu_item_has_a_digit_accelerator() {
        let bound = accelerators_for(&Screen::Home, Notation::Notes, &WalkTheScale::new());

        assert_eq!(bound.len(), HOME_MENU.len());

        for (index, item) in HOME_MENU.iter().enumerate() {
            let expected = char::from_digit(index as u32 + 1, 10).expect("menu fits in digits");

            assert_eq!(
                bound[index],
                (
                    expected,
                    Fires::Widget(FocusTarget::HomeMenuItem(index)),
                    item.label.into()
                ),
                "menu item {index} ({}) is misnumbered or mislabelled",
                item.label
            );
        }
    }

    /// A keycap outside the two ranges `just fonts` passes to `--unicodes` falls through to
    /// whatever the system happens to have — the failure the subset exists to prevent, and
    /// one that shows up as a glyph at the wrong weight rather than as anything that fails.
    /// Nothing at runtime can read the font, so the ranges are written out here and the
    /// table is checked against them.
    #[test]
    fn every_mark_is_one_the_embedded_subset_carries() {
        let carried = |glyph: char| {
            matches!(
                glyph,
                '\u{2190}'..='\u{2193}' | '\u{21E4}'..='\u{21E5}'
            )
        };

        // The back arrow is drawn in the same font and so answers to the same range, even
        // though it is not a keycap and does not appear in `NAVIGATION_KEYS`.
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

    /// Named keys that carry no character and claim no binding must translate to nothing,
    /// rather than falling through to the accelerator path.
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
