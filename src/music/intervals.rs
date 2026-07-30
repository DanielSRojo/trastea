use super::notes::PitchClass;
use std::fmt;

#[derive(PartialEq, Debug, Clone, Copy, Eq, Hash)]
pub enum Interval {
    Unison,
    MinorSecond,
    MajorSecond,
    MinorThird,
    MajorThird,
    PerfectFourth,
    AugmentedFourth,
    PerfectFifth,
    MinorSixth,
    MajorSixth,
    MinorSeventh,
    MajorSeventh,
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Interval::Unison => write!(f, "r"),
            Interval::MinorSecond => write!(f, "b2"),
            Interval::MajorSecond => write!(f, "2"),
            Interval::MinorThird => write!(f, "b3"),
            Interval::MajorThird => write!(f, "3"),
            Interval::PerfectFourth => write!(f, "4"),
            Interval::AugmentedFourth => write!(f, "#4"),
            Interval::PerfectFifth => write!(f, "5"),
            Interval::MinorSixth => write!(f, "b6"),
            Interval::MajorSixth => write!(f, "6"),
            Interval::MinorSeventh => write!(f, "b7"),
            Interval::MajorSeventh => write!(f, "7"),
        }
    }
}

impl Interval {
    pub fn between(from: PitchClass, to: PitchClass) -> Interval {
        let d = (to.semitone() + 12 - from.semitone()) % 12;
        Interval::from_semitone(d)
    }

    pub fn invert(self) -> Interval {
        Interval::from_semitone(12 - self.to_semitone())
    }

    pub fn add(self, other: Interval) -> Interval {
        Interval::from_semitone((self.to_semitone() + other.to_semitone()) % 12)
    }

    pub fn to_semitone(self) -> u8 {
        match self {
            Interval::Unison => 0,
            Interval::MinorSecond => 1,
            Interval::MajorSecond => 2,
            Interval::MinorThird => 3,
            Interval::MajorThird => 4,
            Interval::PerfectFourth => 5,
            Interval::AugmentedFourth => 6,
            Interval::PerfectFifth => 7,
            Interval::MinorSixth => 8,
            Interval::MajorSixth => 9,
            Interval::MinorSeventh => 10,
            Interval::MajorSeventh => 11,
        }
    }

    pub fn from_semitone(d: u8) -> Interval {
        match d % 12 {
            0 => Interval::Unison,
            1 => Interval::MinorSecond,
            2 => Interval::MajorSecond,
            3 => Interval::MinorThird,
            4 => Interval::MajorThird,
            5 => Interval::PerfectFourth,
            6 => Interval::AugmentedFourth,
            7 => Interval::PerfectFifth,
            8 => Interval::MinorSixth,
            9 => Interval::MajorSixth,
            10 => Interval::MinorSeventh,
            11 => Interval::MajorSeventh,
            _ => unreachable!("d % 12 cannot be anything other than 0..11"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn between_test() {
        let (c, cs, d) = (PitchClass::new(0), PitchClass::new(1), PitchClass::new(2));

        assert_eq!(Interval::Unison, Interval::between(c, c));
        assert_eq!(Interval::MajorSecond, Interval::between(c, d));
        assert_eq!(Interval::MinorSeventh, Interval::between(d, c));
        assert_eq!(Interval::MajorSeventh, Interval::between(cs, c));
    }

    #[test]
    fn invert_test() {
        assert_eq!(Interval::Unison.invert(), Interval::Unison);
        assert_eq!(Interval::MajorThird.invert(), Interval::MinorSixth);
        assert_eq!(Interval::MajorSeventh.invert(), Interval::MinorSecond);
        assert_eq!(Interval::MinorSixth.invert(), Interval::MajorThird);
    }

    #[test]
    fn add_test() {
        assert_eq!(
            Interval::MajorSecond.add(Interval::MajorSecond),
            Interval::MajorThird
        );
        assert_eq!(
            Interval::MajorSeventh.add(Interval::MajorSeventh),
            Interval::MinorSeventh
        );
    }
}
