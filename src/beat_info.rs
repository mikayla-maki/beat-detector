use crate::envelope_detector::Envelope;
use core::cmp::Ordering;

/// Information about a single detected beat and its context.
#[derive(Debug, Copy, Clone)]
pub struct BeatInfo {
    /// Beats per minute between 0 and 255.
    bpm: u8,
    envelope: Envelope,
    /// More information about the beat. Was it a low level beat (drums)
    /// or a high level beat (claps).
    frequency_band: FrequencyBand,
}

impl BeatInfo {
    pub(crate) const fn new(bpm: u8, frequency_band: FrequencyBand, envelope: Envelope) -> Self {
        Self {
            bpm,
            frequency_band,
            envelope,
        }
    }

    /// Beats per minute between 0 and 255.
    pub const fn bpm(&self) -> u8 {
        self.bpm
    }

    pub const fn frequency_band(&self) -> FrequencyBand {
        self.frequency_band
    }

    pub fn envelope(&self) -> Envelope {
        self.envelope
    }

    /// Returns the time of the beat at its maximum peak/amplitude in seconds since the
    /// beginning of recording.
    pub fn time_of_beat(&self) -> f32 {
        self.envelope.highest().relative_time
    }
}

impl PartialEq for BeatInfo {
    fn eq(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Equal))
    }
}

impl PartialOrd for BeatInfo {
    fn partial_cmp(&self, _other: &Self) -> Option<Ordering> {
        Some(Ordering::Greater)
        // TODO self.relative_time.partial_cmp(&other.relative_time)
    }
}

impl Eq for BeatInfo {}

impl Ord for BeatInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

// TODO check if there are standardized bands for this/conventions
#[derive(Debug, Copy, Clone)]
pub enum FrequencyBand {
    /// 25-70Hz. Bass beat.
    Low,
    /// 80-250Hz. Clap beat.
    Middle,
}
