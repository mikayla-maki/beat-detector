//! Module for [`EnvelopeDetector`].

use crate::audio_history::AudioHistoryMeta;
use crate::peak::{Peak, PeakDetector};
use crate::BeatIntensity;

/// Higher level wrapper around [`PeaksDetector`]. Finds the envelop of a beat. This is the
/// range of a signal where a significant amount of energy is found, i.e., the range from where
/// the signal suddenly has a strong amplitude and fades back to a smaller one again. As a shortcut,
/// i.e., to discover beats with low latency, the detector does not wait for the actual end of an
/// envelope but until it is clear enough.
///
/// The detector has an internal state and is intended to be re-used as new data comes in.
#[derive(Debug)]
pub(crate) struct EnvelopeDetector {
    /// Contains the index at the end of the previously detected envelope. Accelerates the search
    /// because new envelopes will only be searched in new data (since the previous analysis),
    /// i.e., after that.
    ///
    /// Once this is `Some`, it can become none again
    previous_envelope_end_peak_index: Option<usize>,
}

impl EnvelopeDetector {
    /// The criteria how many times the biggest peak must be higher than a
    /// previous peak. Found out by testing.
    const PEAK_IS_BEAT_CRITERIA: f32 = 2.1;

    /// Creates a new envelope detector.
    pub const fn new() -> Self {
        Self {
            previous_envelope_end_peak_index: None,
        }
    }

    /// Detects if an envelope is found in the given samples array. An detected envelope is a beat.
    ///
    /// An envelope (of a beat) is detected by finding the highest peak of the samples at first.
    /// From then, the function performs a backwards-search from the maximum to find the beginning
    /// of the envelope. Afterwards, it performs a forward-search from the maximum to find the
    /// end of the envelope.
    ///
    /// The function performs an internal tracking of previous envelopes to prevent the detection
    /// of the same envelope twice.
    ///
    /// # Parameters
    /// - `audio_history`: [`AudioHistory`] for information about the audio history (such as time)
    /// - `samples`: Array of samples to operate on. This is usually the audio data from
    ///              `audio_history` after a band filter was applied to it.
    ///
    /// # Returns
    /// Maybe an envelope. Currently, there can never be multiple envelopes be detected at the same
    /// time. This makes no sense because we only want to detect one beat at a time. However, a
    /// frequent detection with new data should enable the detection of all envelopes.
    pub fn detect_envelope(
        &mut self,
        audio_meta: &AudioHistoryMeta,
        samples: &[f32],
    ) -> Option<Envelope> {
        // number 512 chosen at will: seems to work well
        // I rely on that the peaks detector already filters out irrelevant stuff/noise.

        const WORKAROUND_CONST: usize = PeakDetector::DEFAULT_STACK_VEC_CAPACITY;

        // We start the search of peaks at the index where the last envelope ended. This
        // accelerates lookup because less peaks need to be iterated (only new data). We do not
        // iterate the peaks of already discovered envelopes multiple times. We start at the end of
        // the previous envelope because (right now) the end of an envelope can never be the
        // beginning of a next one. Maybe this is not accurate enough; we will see in the future.
        let start_index = self
            .previous_envelope_end_peak_index
            .map(|index| audio_meta.calc_index_after_update(index))
            .flatten();
        self.previous_envelope_end_peak_index = start_index;

        // all peaks were we want to look for envelopes. To accelerate search, we skip all peaks
        // that are before the end of the previously found envelope
        let peaks =
            PeakDetector::detect_peaks::<WORKAROUND_CONST>(samples, audio_meta, start_index);

        // 1) find envelope by maximum absolute peak
        let max_peak = self.find_max_abs(&peaks)?;

        // 2) from there: find begin
        let begin = Self::find_envelope_begin(&peaks, &max_peak)?;

        // 3) and end
        let end = Self::find_envelope_end(&peaks, &max_peak)?;

        /*if let Some(previous) = self.previous_envelope_end_peak_index {
            debug_assert!(previous.end.relative_time < begin.sample_index);
        }*/
        debug_assert!(begin.relative_time < max_peak.relative_time);
        debug_assert!(max_peak.relative_time < end.relative_time);

        let envelope = Envelope::new(begin, end, max_peak);
        self.previous_envelope_end_peak_index
            .replace(envelope.end.sample_index);
        Some(envelope)
    }

    /// First step in detecting an envelope: Finding the maximum peak in the signal. This takes
    /// the field `self.previous_envelope` into account to accelerate the search and prevent
    /// double detection.
    ///
    /// Finds the absolute maximum peak/amplitude of an envelope. Returns the index of
    /// the peak in the array of peaks and the peak object itself.
    fn find_max_abs(&self, peaks: &[Peak]) -> Option<Peak> {
        let mut maybe_max_peak = None;
        for peak in peaks.iter() {
            if maybe_max_peak.is_none() {
                maybe_max_peak.replace(*peak);
            }

            let max_peak = maybe_max_peak.unwrap();

            if max_peak.abs_value() < peak.abs_value() {
                maybe_max_peak.replace(*peak);
            }
        }

        maybe_max_peak
    }

    /// Finds the begin of the envelope. To do this, it takes the maximum of the envelope and then
    /// looks at previous peaks (backwards search). It moves to the left, i.e., from the maximum
    /// peak into the history.
    fn find_envelope_begin(peaks: &[Peak], max_peak: &Peak) -> Option<Peak> {
        /// The envelope can only be a beat if it suddenly starts rising from a low value.
        /// Thus, I require that a peak within the first X peaks must be significantly below
        /// the maximum peak. 7 chosen at will/by testing. I looked at beat envelopes in audacity
        /// and think this value is sufficient.
        // TODO probably good for low beats but not for clap beats (1000hz?)
        const MAX_PEAK_DISTANCE_TO_BEGIN: usize = 7;

        // I reverse the iterator. So I skip all elements that are after the maximum peak.
        // => This way, I can iterate peak by peak "into the past"
        let count_items_after_max = peaks.len() - max_peak.peak_number();

        peaks
            .iter()
            .rev()
            .skip(count_items_after_max)
            // must be close to maximum peak (not too far away)
            .take(MAX_PEAK_DISTANCE_TO_BEGIN)
            // predicate: return the first value that is significantly smaller then the max
            .find(|peak| peak.abs_value() * Self::PEAK_IS_BEAT_CRITERIA < max_peak.abs_value())
            .copied()
    }

    /// Finds the end of the envelope. To do this, it takes the maximum peak (in the "middle" of
    /// the envelope) and then looks at succeeding peaks. Once the peak is below a certain
    /// threshold, a peak was detected.
    fn find_envelope_end(peaks: &[Peak], max_peak: &Peak) -> Option<Peak> {
        // how many peaks we have to skip in the `peaks` slice
        let peaks_to_skip = max_peak.peak_number() + 1;

        let peak_small_enough_fn =
            |peak: &Peak| peak.abs_value() * Self::PEAK_IS_BEAT_CRITERIA < max_peak.abs_value();

        let pairwise_iter = peaks.iter().zip(peaks.iter().skip(1));

        pairwise_iter
            .skip(peaks_to_skip)
            // skip all elements that are not small enough yet
            .skip_while(|(current_peak, _next_peak)| !peak_small_enough_fn(current_peak))
            // The first element that passes this now is small enough and fulfils the criteria.
            // Now we skip elements as long as next_peak is lower then the current peak with the
            // exception that the last peak is always valid.
            .skip_while(|(current_peak, next_peak)| {
                current_peak >= next_peak && next_peak != &peaks.last().unwrap()
            })
            // Only peaks that fulfil the criteria are here.
            // This either returns the last peak before the peaks are rising again or the last peak
            // that was detected.
            .next()
            .map(|(current_peak, _)| current_peak)
            .copied()
    }
}

/// Information about an envelope. A envelope is a collection of multiple peaks in the signal
/// that determine the sudden begin and possibly the fading out of a beat.
///
/// Envelopes can never overlap.
///
/// An overlap
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Envelope {
    /// Relative begin index inside the processed array of samples of the envelope.
    begin: Peak,
    highest: Peak,
    end: Peak,
    intensity: BeatIntensity,
    /// Clarity is the ratio between the highest peak value and the begin of the envelope.
    /// Rounded to three decimal places.
    clarity_begin: f32,
    /// Clarity is the ratio between the highest peak value and the end of the envelope.
    /// Rounded to three decimal places.
    clarity_end: f32,
}

impl Envelope {
    #[track_caller]
    fn new(begin: Peak, end: Peak, highest: Peak) -> Self {
        assert!(begin < highest);
        assert!(highest < end);

        let clarity_begin = highest.abs_value() / begin.abs_value();
        let clarity_begin = libm::roundf(clarity_begin * 1000.0) / 1000.0;

        let clarity_end = highest.abs_value() / end.abs_value();
        let clarity_end = libm::roundf(clarity_end * 1000.0) / 1000.0;

        Self {
            begin,
            end,
            highest,
            intensity: BeatIntensity::new(highest.abs_value()),
            clarity_begin,
            clarity_end,
        }
    }

    pub fn begin(&self) -> Peak {
        self.begin
    }

    pub fn highest(&self) -> Peak {
        self.highest
    }

    pub fn end(&self) -> Peak {
        self.end
    }

    pub fn intensity(&self) -> BeatIntensity {
        self.intensity
    }

    pub fn clarity_begin(&self) -> f32 {
        self.clarity_begin
    }

    // todo doc
    pub fn clarity_end(&self) -> f32 {
        self.clarity_end
    }
}

#[cfg(test)]
mod tests {
    use crate::audio_history::AudioHistory;
    use crate::envelope_detector::{Envelope, EnvelopeDetector};
    use crate::test_util::read_wav_to_mono;

    use crate::peak::Peak;
    use crate::BeatIntensity;
    use heapless::Vec;

    #[test]
    fn test_envelope_detection_on_real_data_1() {
        // count of samples of the wav file
        const SAMPLES_COUNT: usize = 14806;
        let (samples, wav_header) = read_wav_to_mono("res/sample_1_single_beat.wav");

        let mut audio_history = AudioHistory::<SAMPLES_COUNT>::new(wav_header.sampling_rate as f32);
        audio_history.update(&samples);

        let maybe_envelope = EnvelopeDetector::new()
            .detect_envelope(&audio_history.meta(), audio_history.latest_audio());
        let envelope = maybe_envelope.unwrap();

        // I got this by: dbg!(envelope)
        // => I verified in audacity if it is ok
        let expected = Envelope {
            begin: Peak {
                relative_time: 0.029,
                sample_index: 1278,
                value: -0.278,
                peak_number: 1,
            },
            highest: Peak {
                relative_time: 0.051,
                sample_index: 2269,
                value: -0.814,
                peak_number: 5,
            },
            end: Peak {
                relative_time: 0.083,
                sample_index: 3666,
                value: -0.317,
                peak_number: 9,
            },
            intensity: BeatIntensity::new(0.814),
            clarity_begin: 2.928,
            clarity_end: 2.568,
        };

        assert_eq!(expected, envelope);
    }

    /// Test that checks if two beats /two envelopes are found in the double beat wav file.
    #[test]
    fn test_envelope_detection_on_real_data_2() {
        let (samples, wav_header) = read_wav_to_mono("res/sample_1_double_beat.wav");

        // at first, I execute the test "statically" (all data already inside the buffer)
        // => I call envelope detector on the data as long as it doesnt find any more
        let mut audio_history: AudioHistory = AudioHistory::new(wav_header.sampling_rate as f32);

        let mut envelope_detector = EnvelopeDetector::new();

        // simulate that we "listen" to all the audio and update the audio history structure
        // during that process
        let envelopes = samples
            .chunks(256)
            .map(|chunk| {
                audio_history.update(chunk);
                // pretend that we lowpass the data here (this already happened)
                let meta = audio_history.meta();
                let samples = audio_history.latest_audio();
                envelope_detector.detect_envelope(&meta, samples)
            })
            .flatten()
            .collect::<Vec<_, 10>>();

        assert_eq!(envelopes.len(), 2, "must find two envelopes!");

        // I got this value by printing "dbg!(envelopes")
        // => I checked the value in audacity => looks good
        let expected = [
            Envelope {
                begin: Peak {
                    relative_time: 0.0620,
                    sample_index: 2730,
                    value: -0.099,
                    peak_number: 0,
                },
                highest: Peak {
                    relative_time: 0.085,
                    sample_index: 3762,
                    value: -0.442,
                    peak_number: 4,
                },
                end: Peak {
                    relative_time: 0.117,
                    sample_index: 5180,
                    value: -0.200,
                    peak_number: 8,
                },
                intensity: BeatIntensity::new(0.442),
                clarity_begin: 4.467,
                clarity_end: 2.210,
            },
            Envelope {
                begin: Peak {
                    relative_time: 0.232,
                    sample_index: 10237,
                    value: 0.158,
                    peak_number: 23,
                },
                highest: Peak {
                    relative_time: 0.252,
                    sample_index: 11091,
                    value: -0.508,
                    peak_number: 26,
                },
                end: Peak {
                    relative_time: 0.323,
                    sample_index: 14259,
                    value: 0.238,
                    peak_number: 26,
                },
                intensity: BeatIntensity::new(0.508),
                clarity_begin: 3.215,
                clarity_end: 2.134,
            },
        ];

        assert_eq!(envelopes, expected);
    }

    /// Test that checks if two beats /two envelopes are found in the double beat wav file.
    #[test]
    fn test_envelope_detection_on_real_data_3() {
        let (samples, wav_header) = read_wav_to_mono("res/sample_1.lowpassed.wav");

        // at first, I execute the test "statically" (all data already inside the buffer)
        // => I call envelope detector on the data as long as it doesnt find any more
        let mut audio_history: AudioHistory = AudioHistory::new(wav_header.sampling_rate as f32);

        let mut envelope_detector = EnvelopeDetector::new();

        // simulate that we "listen" to all the audio and update the audio history structure
        // during that process
        let envelopes = samples
            .chunks(256)
            .map(|chunk| {
                audio_history.update(chunk);
                // pretend that we lowpass the data here (this already happened)
                let meta = audio_history.meta();
                let samples = audio_history.latest_audio();
                envelope_detector.detect_envelope(&meta, samples)
            })
            .flatten()
            .collect::<Vec<_, 10>>();

        assert_eq!(envelopes.len(), 6, "must find six envelopes!");

        // I got this value by printing "dbg!(envelopes")
        // => I checked the value in audacity => looks good
        let expected = [
            Envelope {
                begin: Peak {
                    relative_time: 0.268,
                    sample_index: 11804,
                    value: -0.128,
                    peak_number: 0,
                },
                highest: Peak {
                    relative_time: 0.291,
                    sample_index: 12832,
                    value: -0.561,
                    peak_number: 4,
                },
                end: Peak {
                    relative_time: 0.323,
                    sample_index: 14254,
                    value: -0.228,
                    peak_number: 8,
                },
                intensity: BeatIntensity::new(0.561),
                clarity_begin: 4.369,
                clarity_end: 2.460,
            },
            Envelope {
                begin: Peak {
                    relative_time: 2.101,
                    sample_index: 19415,
                    value: -0.0989,
                    peak_number: 0,
                },
                highest: Peak {
                    relative_time: 2.125,
                    sample_index: 20447,
                    value: -0.442,
                    peak_number: 4,
                },
                end: Peak {
                    relative_time: 2.157,
                    sample_index: 21864,
                    value: -0.200,
                    peak_number: 8,
                },
                intensity: BeatIntensity::new(0.442),
                clarity_begin: 4.466,
                clarity_end: 2.210,
            },
            Envelope {
                begin: Peak {
                    relative_time: 2.271,
                    sample_index: 17704,
                    value: 0.159,
                    peak_number: 14,
                },
                highest: Peak {
                    relative_time: 2.291,
                    sample_index: 18560,
                    value: -0.508,
                    peak_number: 17,
                },
                end: Peak {
                    relative_time: 2.363,
                    sample_index: 21726,
                    value: 0.238,
                    peak_number: 26,
                },
                intensity: BeatIntensity::new(0.508),
                clarity_begin: 3.213,
                clarity_end: 2.137,
            },
            Envelope {
                begin: Peak {
                    relative_time: 4.274,
                    sample_index: 19484,
                    value: -0.134,
                    peak_number: 0,
                },
                highest: Peak {
                    relative_time: 4.297,
                    sample_index: 20507,
                    value: -0.540,
                    peak_number: 4,
                },
                end: Peak {
                    relative_time: 4.330,
                    sample_index: 21947,
                    value: -0.23654896,
                    peak_number: 8,
                },
                intensity: BeatIntensity::new(0.539),
                clarity_begin: 4.020,
                clarity_end: 2.279,
            },
            Envelope {
                begin: Peak {
                    relative_time: 6.114,
                    sample_index: 19508,
                    value: -0.099,
                    peak_number: 0,
                },
                highest: Peak {
                    relative_time: 6.138,
                    sample_index: 20542,
                    value: -0.441,
                    peak_number: 4,
                },
                end: Peak {
                    relative_time: 6.170,
                    sample_index: 21965,
                    value: -0.202,
                    peak_number: 8,
                },
                intensity: BeatIntensity::new(0.441),
                clarity_begin: 4.445,
                clarity_end: 2.187,
            },
            Envelope {
                begin: Peak {
                    relative_time: 6.285,
                    sample_index: 17058,
                    value: 0.154,
                    peak_number: 14,
                },
                highest: Peak {
                    relative_time: 6.305,
                    sample_index: 17910,
                    value: -0.476,
                    peak_number: 17,
                },
                end: Peak {
                    relative_time: 6.393,
                    sample_index: 21790,
                    value: 0.215,
                    peak_number: 28,
                },
                intensity: BeatIntensity::new(0.476),
                clarity_begin: 3.102,
                clarity_end: 2.216,
            },
        ];

        assert_eq!(envelopes, expected);
    }

    #[ignore]
    #[test]
    fn test_print_envelopes() {
        let (samples, wav_header) = read_wav_to_mono("res/ausschnitt-holiday-lowpassed.wav");

        // at first, I execute the test "statically" (all data already inside the buffer)
        // => I call envelope detector on the data as long as it doesnt find any more
        let mut audio_history: AudioHistory = AudioHistory::new(wav_header.sampling_rate as f32);

        let mut envelope_detector = EnvelopeDetector::new();

        // simulate that we "listen" to all the audio and update the audio history structure
        // during that process
        let envelopes = samples
            .chunks(256)
            .map(|chunk| {
                audio_history.update(chunk);
                // pretend that we lowpass the data here (this already happened)
                let meta = audio_history.meta();
                let samples = audio_history.latest_audio();
                envelope_detector.detect_envelope(&meta, samples)
            })
            .flatten()
            .collect::<std::vec::Vec<_>>();

        dbg!(envelopes.len());
        dbg!(envelopes);
    }
}
