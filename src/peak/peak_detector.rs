//! Module for [`PeaksDetector`].

use crate::audio_history::AudioHistoryMeta;
use crate::peak::local_min_max_iterator::LocalMinMaxIterator;
use crate::peak::Peak;
use heapless::Vec;

/// Detects all peaks (local minimums and maximums) in a wave. A peak is the highest (or lowest)
/// amplitude value after that the wave goes back to zero (crosses the x axis). The peak detector
/// expects to operate on float samples in range `[-1, 1]`.
///
/// A peak may be belong to a beat. It is a lower level analysis primitive of my beat detection.
///
/// The peak detector does not have an internal state. It is the responsibility of the next higher
/// level wrapper to connect peaks to a envelope and to do this as new samples come in continuously.
///
/// Wrapper around [`LocalMinMaxIterator`].
pub struct PeakDetector;

impl PeakDetector {
    /// Default capacity for the [`Vec`] returned by [`Self::detect_peaks`]
    pub const DEFAULT_STACK_VEC_CAPACITY: usize = 512;

    /// The minimum absolute peak to distinguish sound from noise.
    const MINIMUM_PEAK: f32 = 0.05;

    /// Detects all peaks (local minimums and maximums) in a signal. Expects the input data
    /// to be in interval `[-1, 1]`. Will ignore very small values (noise). The return type is a
    /// tuple of type (a,b) where a is the index in the array of samples and b the amplitude value
    /// of the peak.
    ///
    /// Only returns real peaks and ignores noise.
    ///
    /// Parameters:
    /// - `const N`: number of elements
    /// - `samples`: audio samples where all values are valid in interval `[-1; 1]` (never NaN or
    ///              infinite)
    /// - `meta`   : stats about the audio recording
    /// - `preferred_start_index`: Start index in `samples` array. Can be used to accelerate the
    ///                            search (only search for new peaks)
    pub fn detect_peaks<const N: usize>(
        samples: &[f32],
        meta: &AudioHistoryMeta,
        preferred_start_index: Option<usize>,
    ) -> Vec<Peak, N> {
        debug_assert!(
            samples.iter().all(|x| x.is_finite()),
            "only regular/normal f32 samples allowed!"
        );
        debug_assert!(
            samples.iter().all(|x| libm::fabsf(*x) <= 1.0),
            "only values in range [-1, 1] allowed!"
        );

        LocalMinMaxIterator::new(samples, preferred_start_index)
            .filter(|local_min_max| libm::fabsf(local_min_max.value) >= Self::MINIMUM_PEAK)
            .enumerate()
            .map(|(peak_num, local_min_max)| {
                Peak::new(local_min_max.index, local_min_max.value, peak_num, meta)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_history::AudioHistory;
    use crate::test_util::read_wav_to_mono;

    // test that verifies if noise is ignored
    #[test]
    fn test_peaks_detector_ignores_really_small_values() {
        let test_data = [-0.01, 0.01, -0.01, 0.01, -0.5, 0.6, -1.0, 0.0];
        let mut audio_history = AudioHistory::<1024>::new(1.0);
        audio_history.update(&test_data);

        let peaks = PeakDetector::detect_peaks::<4>(&test_data, &audio_history.meta(), None);

        let mut expected = Vec::<_, 3>::new();
        expected.extend(&[
            Peak {
                sample_index: 4,
                relative_time: 4.0,
                value: -0.5,
                peak_number: 0,
            },
            Peak {
                sample_index: 5,
                relative_time: 5.0,
                value: 0.6,
                peak_number: 1,
            },
            Peak {
                sample_index: 6,
                relative_time: 6.0,
                value: -1.0,
                peak_number: 2,
            },
        ]);

        peaks.iter().enumerate().for_each(|(peak_index, peak)| {
            assert_eq!(peak, expected[peak_index]);
        });
    }

    #[test]
    fn test_preferred_begin_index() {
        let test_data = [0.0, -0.2, -0.4, -0.2, 0.0, 0.2, 0.4, 0.2, 0.0];
        let mut audio_history = AudioHistory::<100>::new(1.0);
        audio_history.update(&test_data);
        let meta = audio_history.meta();
        let all_peaks = PeakDetector::detect_peaks::<10>(audio_history.latest_audio(), &meta, None);
        let all_peaks_expected = [
            Peak {
                relative_time: 3.0,
                sample_index: 2,
                value: -0.4,
                peak_number: 0,
            },
            Peak {
                relative_time: 7.0,
                sample_index: 6,
                value: 0.4,
                peak_number: 1,
            },
        ];
        assert_eq!(&all_peaks, &all_peaks_expected);

        let peaks = PeakDetector::detect_peaks::<10>(audio_history.latest_audio(), &meta, Some(1));
        assert_eq!(&peaks, &all_peaks_expected[1..]);
        let peaks = PeakDetector::detect_peaks::<10>(audio_history.latest_audio(), &meta, Some(2));
        assert_eq!(&peaks, &all_peaks_expected[1..]);
        let peaks = PeakDetector::detect_peaks::<10>(audio_history.latest_audio(), &meta, Some(3));
        assert_eq!(&peaks, &all_peaks_expected[1..]);
        let peaks = PeakDetector::detect_peaks::<10>(audio_history.latest_audio(), &meta, Some(4));
        assert_eq!(&peaks, &all_peaks_expected[1..]);

        let peaks = PeakDetector::detect_peaks::<10>(audio_history.latest_audio(), &meta, Some(5));
        assert!(peaks.is_empty());
    }

    /// Tests the peaks detector against a real sample and checks if the amplitudes
    /// and timings are at the right positions.
    #[test]
    fn test_peaks_on_real_data_1() {
        // count of samples of the wav file
        const SAMPLES_COUNT: usize = 14806;
        let (samples, wav_header) = read_wav_to_mono("res/sample_1_single_beat.wav");

        let mut audio_history = AudioHistory::<SAMPLES_COUNT>::new(wav_header.sampling_rate as f32);
        audio_history.update(&samples);

        let meta = audio_history.meta();
        let samples = audio_history.latest_audio();
        let peaks = PeakDetector::detect_peaks::<40>(samples, &meta, None);

        assert_eq!(peaks.len(), 40);

        // I got these by printing out:
        // dbg!(&peaks[0..10]);
        // I verified the results in audacity and the timings do match
        const EXPECTED_PEAKS: &[Peak] = &[
            Peak {
                relative_time: 0.026258503401360545,
                sample_index: 1157,
                value: 0.10766931,
                peak_number: 0,
            },
            Peak {
                relative_time: 0.029002267573696117,
                sample_index: 1278,
                value: -0.2778405,
                peak_number: 1,
            },
            Peak {
                relative_time: 0.03170068027210882,
                sample_index: 1397,
                value: 0.5884732,
                peak_number: 2,
            },
            Peak {
                relative_time: 0.037414965986394544,
                sample_index: 1649,
                value: -0.7123936,
                peak_number: 3,
            },
            Peak {
                relative_time: 0.043877551020408134,
                sample_index: 1934,
                value: 0.599353,
                peak_number: 4,
            },
            Peak {
                relative_time: 0.05147392290249431,
                sample_index: 2269,
                value: -0.8136845,
                peak_number: 5,
            },
            Peak {
                relative_time: 0.05945578231292514,
                sample_index: 2621,
                value: 0.69907224,
                peak_number: 6,
            },
            Peak {
                relative_time: 0.06746031746031744,
                sample_index: 2974,
                value: -0.5265206,
                peak_number: 7,
            },
            Peak {
                relative_time: 0.07510204081632649,
                sample_index: 3311,
                value: 0.39107943,
                peak_number: 8,
            },
            Peak {
                relative_time: 0.08315192743764172,
                sample_index: 3666,
                value: -0.31708732,
                peak_number: 9,
            },
        ];

        assert_eq!(&peaks[0..10], EXPECTED_PEAKS);
    }
}
