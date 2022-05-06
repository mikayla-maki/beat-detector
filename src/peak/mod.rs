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
    /// Rounded to three decimal places.
    pub(crate) relative_time: f32,
    /// The value of the peak in range `[-1, 1]`.
    /// Rounded to three decimal places.
    pub(crate) value: f32,
}

impl Peak {
    #[track_caller]
    pub fn new(sample_index: usize, value: f32, audio_meta: &AudioHistoryMeta) -> Self {
        let relative_time = audio_meta.time_of_sample(sample_index);

        // round two three decimal places
        let relative_time = libm::roundf(relative_time * 1000.0) / 1000.0;

        // round two three decimal places
        let value = libm::roundf(value * 1000.0) / 1000.0;

        Self {
            value,
            relative_time,
        }
    }

    /// The value of the peak in range `[-1, 1]`.
    pub fn value(&self) -> f32 {
        self.value
    }

    /// The absolute value of the peak in range `[0, 1]`.
    pub fn abs_value(&self) -> f32 {
        libm::fabsf(self.value)
    }

    /// The relative time since the beginning of the recoding of audio at `sample_index`.
    pub fn relative_time(&self) -> f32 {
        self.relative_time
    }
}

impl PartialEq for Peak {
    fn eq(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(&other), Some(Ordering::Equal))
    }
}

impl PartialOrd for Peak {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.relative_time.partial_cmp(&other.relative_time)
    }
}

/// Internal version of a peak that holds additional information about where a peak was found.
// This is a dedicated struct to simplify testing in higher level abstractions.
#[derive(Debug, Clone, Copy)]
pub struct InternalPeak {
    /// Index in the array of samples at that the peak was detected.
    ///
    /// Only valid for one iteration of the algorithm because after new data was received, the
    /// index changed.
    pub(crate) sample_index: usize,

    /// Number of the peak in the array of peaks from the analysis that it originates from. There
    /// exist a total order and a relation between peak number and time. Higher numbers correspond
    /// to "younger" peaks.
    ///
    /// Only valid for one iteration of the algorithm because after new data was received, the
    /// index changed.
    pub(crate) peak_number: usize,

    /// [`Peak`].
    pub(crate) peak: Peak,
}

impl InternalPeak {
    #[track_caller]
    pub fn new(
        sample_index: usize,
        value: f32,
        peak_number: usize,
        audio_meta: &AudioHistoryMeta,
    ) -> Self {
        Self {
            sample_index,
            peak_number,
            peak: Peak::new(sample_index, value, audio_meta),
        }
    }

    /// Transforms the [`InternalPeak`] into [`Peak`].
    pub fn to_peak(self) -> Peak {
        self.peak
    }
}

impl PartialEq for InternalPeak {
    fn eq(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(&other), Some(Ordering::Equal))
    }
}

impl PartialOrd for InternalPeak {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.peak
            .relative_time
            .partial_cmp(&other.peak.relative_time)
    }
}
