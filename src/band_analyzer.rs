use crate::audio_history::AudioHistoryMeta;
use crate::envelope_detector::{Envelope, EnvelopeDetector};
use crate::util::RingBufferWithSerialSliceAccess;
use biquad::{Biquad, ToHertz, Type};

/// Helper struct for [`crate::BeatDetector`]. Takes the original audio data, applies a band filter
/// on it with the given frequency boundaries, and analyzes the lowpassed data with a
/// [`EnvelopeDetector`]. With that information, it tries to find a beat from the peaks in the given
/// frequency band of the original audio data.
///
///
/// The underlying [`EnvelopeDetector`] ensures that the same beat is never detected twice.
#[derive(Debug)]
pub(crate) struct BandAnalyzer {
    /// Lower frequency of the band.
    lower_frequency: f32,
    /// Higher frequency of the band.
    higher_frequency: f32,
    sampling_frequency: f32,
    envelope_detector: EnvelopeDetector,
}

impl BandAnalyzer {
    /// Constructor.
    pub fn new(lower_frequency: f32, higher_frequency: f32, sampling_frequency: f32) -> Self {
        debug_assert!(lower_frequency.is_normal());
        debug_assert!(higher_frequency.is_normal());
        debug_assert!(sampling_frequency.is_normal());
        debug_assert!(
            higher_frequency <= sampling_frequency / 2.0,
            "Nyquist theorem: high frequency to high"
        );
        debug_assert!(
            lower_frequency < higher_frequency,
            "higher frequency must be higher"
        );

        Self {
            lower_frequency,
            higher_frequency,
            sampling_frequency,
            envelope_detector: EnvelopeDetector::new(),
        }
    }

    /// Constructor with default parameters for a low pass filter.
    pub fn new_low(sampling_rate: f32) -> Self {
        Self::new(25.0, 70.0, sampling_rate)
    }

    /// Wrapper that connects [`AudioHistory`], a band filter, and the [`EnvelopeDetector`].
    /// Returns the result of [`EnvelopeDetector::detect_envelope`].
    ///
    /// Needs access to a ring buffer where it can store the low passed
    pub fn detect_envelope<const N: usize>(
        &mut self,
        original_samples: &[f32],
        band_pass_samples_buffer: &mut RingBufferWithSerialSliceAccess<f32, N>,
        audio_meta: &AudioHistoryMeta,
    ) -> Option<Envelope> {
        debug_assert!(original_samples.len() <= band_pass_samples_buffer.capacity());

        // store band passed data in mutable buffer
        self.apply_band_filter(original_samples, band_pass_samples_buffer);

        // get slice of band passed data
        let band_passed_samples_slice = band_pass_samples_buffer.continuous_slice();

        self.envelope_detector
            .detect_envelope(audio_meta, band_passed_samples_slice)
    }

    /// Applies the band filter and updates the internal data structure that contains the
    /// filtered amplitude.
    fn apply_band_filter<const N: usize>(
        &mut self,
        samples: &[f32],
        band_pass_samples_buffer: &mut RingBufferWithSerialSliceAccess<f32, N>,
    ) {
        assert!(
            samples.len() <= N,
            "samples length must equal to the internal buffer length"
        );
        assert!(
            samples.len() > 10,
            "samples length must be more then 10 samples long - everything else makes no sense!"
        );

        // This clear is necessary because in the beginning the buffer behind `self.audio_history`
        // is not full yet => thus not all indices would be overwritten => inconsistent data
        band_pass_samples_buffer.clear();

        let high_pass_coefficients = biquad::Coefficients::<f32>::from_params(
            Type::HighPass,
            self.sampling_frequency.hz(),
            self.lower_frequency.hz(),
            biquad::Q_BUTTERWORTH_F32,
        )
        .unwrap();
        let mut high_pass = biquad::DirectForm1::<f32>::new(high_pass_coefficients);

        let low_pass_coefficients = biquad::Coefficients::<f32>::from_params(
            Type::LowPass,
            self.sampling_frequency.hz(),
            self.higher_frequency.hz(),
            biquad::Q_BUTTERWORTH_F32,
        )
        .unwrap();
        let mut low_pass = biquad::DirectForm1::<f32>::new(low_pass_coefficients);

        for sample in samples.iter() {
            let high_passed_sample = high_pass.run(*sample);
            let band_passed_sample = low_pass.run(high_passed_sample);
            band_pass_samples_buffer.push(band_passed_sample);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::audio_history::{AudioHistory, AUDIO_HISTORY_DEFAULT_BUFFER_SIZE};
    use crate::band_analyzer::BandAnalyzer;

    use crate::test_util::read_wav_to_mono;
    use crate::util::RingBufferWithSerialSliceAccess;

    #[test]
    fn test_highpass_removes_all_amplitudes() {
        let (audio, wav_header) = read_wav_to_mono("res/sample_1_single_beat.wav");
        // ensure that our file corresponds to the test
        const SAMPLES_COUNT: usize = 14806;

        let mut audio_history = AudioHistory::<SAMPLES_COUNT>::new(wav_header.sampling_rate as f32);
        audio_history.update(&audio);

        let mut analyzer = BandAnalyzer::new(1000.0, 22050.0, wav_header.sampling_rate as f32);

        let mut audio_buf = RingBufferWithSerialSliceAccess::<_, SAMPLES_COUNT>::new();

        let meta = audio_history.meta();
        assert!(analyzer
            .detect_envelope(audio_history.latest_audio(), &mut audio_buf, &meta)
            .is_none());
    }

    #[test]
    fn test_beat_detected_real_audio_single_beat() {
        let (audio, wav_header) = read_wav_to_mono("res/sample_1_single_beat.wav"); // ensure that our file corresponds to the test
        const SAMPLES_COUNT: usize = 14806;

        let mut audio_history = AudioHistory::<SAMPLES_COUNT>::new(wav_header.sampling_rate as f32);
        audio_history.update(&audio);

        let mut analyzer = BandAnalyzer::new(25.0, 70.0, wav_header.sampling_rate as f32);

        let mut audio_buf = RingBufferWithSerialSliceAccess::<_, SAMPLES_COUNT>::new();

        let meta = audio_history.meta();

        let envelope = analyzer
            .detect_envelope(audio_history.latest_audio(), &mut audio_buf, &meta)
            .unwrap();

        // you can look at the waveform in audacity and verify these values
        let expected = (0.054, -0.535);
        assert_eq!(expected.0, envelope.highest().relative_time);
        assert_eq!(expected.1, envelope.highest().value);
    }

    #[test]
    fn test_beat_detected_real_audio_sample_1() {
        let (audio, wav_header) = read_wav_to_mono("res/sample_1.wav"); // ensure that our file corresponds to the test

        let mut audio_history =
            AudioHistory::<AUDIO_HISTORY_DEFAULT_BUFFER_SIZE>::new(wav_header.sampling_rate as f32);

        let mut analyzer = BandAnalyzer::new_low(wav_header.sampling_rate as f32);

        let mut audio_buf =
            RingBufferWithSerialSliceAccess::<_, AUDIO_HISTORY_DEFAULT_BUFFER_SIZE>::new();

        let actual = audio
            .chunks(256)
            .map(|samples| {
                audio_history.update(samples);
                let meta = audio_history.meta();
                analyzer.detect_envelope(audio_history.latest_audio(), &mut audio_buf, &meta)
            })
            .flatten()
            .map(|envelope| {
                (
                    envelope.highest().relative_time,
                    envelope.highest().abs_value(),
                )
            })
            .collect::<std::vec::Vec<_>>();

        // I got this values by: dbg!(actual)
        // => I checked in audacity if the values are correct
        let expected = [
            (0.292, 0.535),
            (2.133, 0.424),
            (2.299, 0.505),
            (4.298, 0.514),
            (6.146, 0.424),
            (6.313, 0.472),
        ];

        assert_eq!(actual.len(), expected.len());
        assert_eq!(
            actual
                .into_iter()
                .map(|(time, intensity)| (
                    (time * 1000.0).round() / 1000.0,
                    (intensity * 1000.0).round() / 1000.0
                ))
                .collect::<std::vec::Vec<_>>(),
            expected
        );
    }
}
