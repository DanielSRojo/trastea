use iced::alignment;
use iced::widget::canvas::{self, Canvas, Fill, Frame, Path, Stroke, Text};
use iced::{Color, Element, Length, Pixels, Point, Rectangle, Renderer, Theme};

use crate::music::notes::Note;

pub struct NoteMarker {
    pub string: usize, // 0 = low E, 5 = high e
    pub fret: usize,
    pub note: Note,
    pub color: Color,
}

pub struct Fretboard {
    pub num_frets: usize,
    pub highlighted: Vec<NoteMarker>,
}

impl Default for Fretboard {
    fn default() -> Self {
        Self {
            num_frets: 12,
            highlighted: Vec::new(),
        }
    }
}

impl<Message> canvas::Program<Message> for Fretboard {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());

        let pad_x = 20.0_f32;
        let pad_bottom = 20.0_f32;
        let open_marker_gap = 4.0_f32;
        let target_fret_cell_ratio = 1.35_f32;

        let fret_count = self.num_frets;
        let string_count = 6_usize;

        // Background
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), Color::BLACK);

        let available_w = bounds.width - 2.0 * pad_x;
        let available_string_spacing = available_w / (string_count - 1) as f32;
        let max_note_radius = (available_string_spacing * 0.35).clamp(8.0, 14.0);
        let pad_top = max_note_radius * 2.0 + open_marker_gap;
        let usable_h = bounds.height - pad_top - pad_bottom;
        let fret_spacing = usable_h / fret_count as f32;
        let string_spacing = available_string_spacing.min(fret_spacing / target_fret_cell_ratio);
        let usable_w = string_spacing * (string_count - 1) as f32;
        let left_edge = (bounds.width - usable_w) / 2.0;
        let note_radius = (string_spacing * 0.35).clamp(8.0, 14.0);

        let string_x = |s: usize| left_edge + s as f32 * string_spacing;
        let fret_y = |f: usize| pad_top + f as f32 * fret_spacing;

        let top_y = pad_top;
        let bottom_y = pad_top + usable_h;
        let left_x = left_edge;
        let right_x = left_edge + usable_w;

        let ink = Color::WHITE;
        let gray = Color::from_rgb8(0x3a, 0x3a, 0x3a);
        let light_gray = Color::from_rgb8(0x77, 0x77, 0x77);

        frame.stroke(
            &Path::line(Point::new(left_x, top_y), Point::new(right_x, top_y)),
            Stroke::default().with_color(ink).with_width(3.0),
        );

        // Frets — thin gray horizontal lines for frets 1–12
        for f in 1..=fret_count {
            let y = fret_y(f);
            frame.stroke(
                &Path::line(Point::new(left_x, y), Point::new(right_x, y)),
                Stroke::default().with_color(gray).with_width(1.0),
            );
        }

        // Strings — thin light-gray vertical lines
        for s in 0..string_count {
            let x = string_x(s);
            frame.stroke(
                &Path::line(Point::new(x, top_y), Point::new(x, bottom_y)),
                Stroke::default().with_color(light_gray).with_width(1.0),
            );
        }

        let marker_x = (string_x(2) + string_x(3)) / 2.0;
        let lower_double_marker_x = (string_x(1) + string_x(2)) / 2.0;
        let upper_double_marker_x = (string_x(3) + string_x(4)) / 2.0;
        let dot_gray = Color::from_rgb8(0x88, 0x88, 0x88);
        let fret_marker_radius = 6.5;

        for &f in &[3_usize, 5, 7, 9] {
            let y = (fret_y(f - 1) + fret_y(f)) / 2.0;
            frame.fill(
                &Path::circle(Point::new(marker_x, y), fret_marker_radius),
                Fill::from(dot_gray),
            );
        }

        // Double dot at fret 12
        {
            let y = (fret_y(11) + fret_y(12)) / 2.0;
            frame.fill(
                &Path::circle(Point::new(lower_double_marker_x, y), fret_marker_radius),
                Fill::from(dot_gray),
            );
            frame.fill(
                &Path::circle(Point::new(upper_double_marker_x, y), fret_marker_radius),
                Fill::from(dot_gray),
            );
        }

        // Highlighted note markers
        let note_radius = note_radius.min((fret_spacing * 0.35).clamp(8.0, 14.0));

        for marker in &self.highlighted {
            let x = string_x(marker.string);
            let y = if marker.fret == 0 {
                top_y - note_radius - open_marker_gap
            } else {
                (fret_y(marker.fret - 1) + fret_y(marker.fret)) / 2.0
            };

            frame.fill(
                &Path::circle(Point::new(x, y), note_radius),
                Fill::from(marker.color),
            );

            frame.fill_text(Text {
                content: marker.note.to_string(),
                position: Point::new(x, y),
                color: Color::WHITE,
                size: Pixels(note_radius * 1.1),
                align_x: iced::widget::text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        vec![frame.into_geometry()]
    }
}

pub fn fretboard<Message: 'static>(fb: Fretboard) -> Element<'static, Message> {
    Canvas::new(fb)
        .width(Length::Fixed(280.0))
        .height(Length::Fill)
        .into()
}
