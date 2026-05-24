use std::fmt;

#[derive(PartialEq, Debug, Clone, Copy, Eq, Hash)]
pub enum Note {
    C,
    Cs,
    D,
    Ds,
    E,
    F,
    Fs,
    G,
    Gs,
    A,
    As,
    B,
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Note::C => write!(f, "C"),
            Note::Cs => write!(f, "C#"),
            Note::D => write!(f, "D"),
            Note::Ds => write!(f, "D#"),
            Note::E => write!(f, "E"),
            Note::F => write!(f, "F"),
            Note::Fs => write!(f, "F#"),
            Note::G => write!(f, "G"),
            Note::Gs => write!(f, "G#"),
            Note::A => write!(f, "A"),
            Note::As => write!(f, "A#"),
            Note::B => write!(f, "B"),
        }
    }
}

impl Note {
    pub const ALL: &'static [Note] = &[
        Note::C,
        Note::Cs,
        Note::D,
        Note::Ds,
        Note::E,
        Note::F,
        Note::Fs,
        Note::G,
        Note::Gs,
        Note::A,
        Note::As,
        Note::B,
    ];
    pub fn to_semitone(self) -> u8 {
        match self {
            Note::C => 0,
            Note::Cs => 1,
            Note::D => 2,
            Note::Ds => 3,
            Note::E => 4,
            Note::F => 5,
            Note::Fs => 6,
            Note::G => 7,
            Note::Gs => 8,
            Note::A => 9,
            Note::As => 10,
            Note::B => 11,
        }
    }

    pub fn from_semitone(n: u8) -> Note {
        match n % 12 {
            0 => Note::C,
            1 => Note::Cs,
            2 => Note::D,
            3 => Note::Ds,
            4 => Note::E,
            5 => Note::F,
            6 => Note::Fs,
            7 => Note::G,
            8 => Note::Gs,
            9 => Note::A,
            10 => Note::As,
            11 => Note::B,
            _ => unreachable!("n % 12 cannot be anything other than 0..11"),
        }
    }

    pub fn transpose(self, semitones: u8) -> Note {
        Note::from_semitone(self.to_semitone() + semitones)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let all = [
            Note::C,
            Note::Cs,
            Note::D,
            Note::Ds,
            Note::E,
            Note::F,
            Note::Fs,
            Note::G,
            Note::Gs,
            Note::A,
            Note::As,
            Note::B,
        ];

        for note in all {
            let s = note.to_semitone();
            assert_eq!(note, Note::from_semitone(s));
        }
    }

    #[test]
    fn transpose_test() {
        assert_eq!(Note::E.transpose(7), Note::B);
    }

    #[test]
    fn wrapping() {
        assert_eq!(Note::B.transpose(1), Note::C);
    }
}
