use core::cmp::Ordering;

/// A beat intensity between 0,1 and 1,0.
/// Rounded to three decimal places.
#[derive(Copy, Clone, Debug)]
pub struct BeatIntensity(f32);

impl BeatIntensity {
    /// Inclusive lower bound of a amplitude in ranges `[-1, 1]` required to be detected valid as beat.
    pub const MIN: f32 = 0.2;
    pub const MAX: f32 = 1.0;

    #[track_caller]
    pub(crate) fn new(val: f32) -> Self {
        // TODO don't know how much sense this makes here because I should also enforce
        //  this rule at the level where beats are detected
        //assert!(val <= Self::MAX, "val <= MAX! is: {}", val);
        //assert!(val >= Self::MIN, "val >= MIN! is: {}", val);

        // val should already be rounded, always (by the lower level components)

        Self(val)
    }

    pub const fn val(self) -> f32 {
        self.0
    }
}

impl PartialEq for BeatIntensity {
    fn eq(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other).unwrap(), Ordering::Equal)
    }
}

impl PartialOrd for BeatIntensity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.val().partial_cmp(&other.val())
    }
}

impl Eq for BeatIntensity {}

impl Ord for BeatIntensity {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}
