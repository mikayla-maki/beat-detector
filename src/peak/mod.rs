mod local_min_max_iterator;
mod peak_detector;
mod zero_of_function_iterator;

use crate::audio_history::AudioHistoryMeta;
use core::cmp::Ordering;

pub use peak_detector::*;

/// A peak is a local minimum or maximum in a wave.
#[derive(Debug, Clone, Copy)]
pub struct Peak {
    /// The relative time since the beginning of the recoding of audio at `sample_index`.
    pub(crate) relative_time: f64,
    /// Index in the array of samples at that the peak was detected.
    ///
    /// INTERNAL. IRRELEVANT FOR PUBLIC API.
    pub(crate) sample_index: usize,
    /// The value of the peak in range `[-1, 1]`.
    pub(crate) value: f32,
    /// Number of the peak in the array of peaks from the analysis that it originates from. There
    /// exist a total order and a relation between peak number and time. Higher numbers correspond
    /// to "younger" peaks. The peak index is only valid within the window of samples that it
    /// originates from.
    ///
    /// INTERNAL. IRRELEVANT FOR PUBLIC API.
    pub(crate) peak_number: usize,
}

impl Peak {
    #[track_caller]
    pub fn new(
        sample_index: usize,
        value: f32,
        peak_number: usize,
        audio_meta: &AudioHistoryMeta,
    ) -> Self {
        Self {
            sample_index,
            value,
            peak_number: peak_number,
            relative_time: audio_meta.time_of_sample(sample_index),
        }
    }

    /// Index in the array of samples at that the peak was detected.
    ///
    /// INTERNAL USAGE. Irrelevant for public API.
    pub fn sample_index(&self) -> usize {
        self.sample_index
    }

    /// The value of the peak in range `[-1, 1]`.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// The absolute value of the peak in range `[0, 1]`.
    pub fn abs_value(&self) -> f32 {
        libm::fabsf(self.value)
    }

    /// Number of the peak in the array of peaks from the analysis that it originates from. There
    /// exist a total order and a relation between peak number and time. Higher numbers correspond
    /// to "younger" peaks. The peak index is only valid within the window of samples that it
    /// originates from.
    ///
    /// INTERNAL USAGE. Irrelevant for public API.
    pub fn peak_number(&self) -> usize {
        self.peak_number
    }

    /// The relative time since the beginning of the recoding of audio at `sample_index`.
    pub fn relative_time(&self) -> f64 {
        self.relative_time
    }
}

impl PartialEq for Peak {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.sample_index == other.sample_index
    }
}

impl PartialOrd for Peak {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.relative_time.partial_cmp(&other.relative_time)
    }
}
