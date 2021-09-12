use crate::util::RingBufferWithSerialSliceAccess;

/// Default/recommended buffer size for the audio history. This equals half of a second with a
/// sampling rate of 44100 Hz and a little less with 48000 Hz. Envelopes (beats) are up to 400ms
/// long, and thus a small window it not sufficient enough to detection them properly.
pub const AUDIO_HISTORY_DEFAULT_BUFFER_SIZE: usize = 22500;

/// Keeps state about an ongoing audio signal. Keeps the latest X seconds and updates the
/// internal time. The time is determined by the amount of consumed samples and the time per
/// sample. Since a audio analysis is ongoing, the internal relative time correlates to the actual
/// passed time.
///
/// Initially, the audio buffer is filled with zeroes.
///
/// Helper struct for beat detection.
///
/// Expects that the sampling rate stays constant during the runtime.
#[derive(Debug)]
pub(crate) struct AudioHistory<const N: usize = AUDIO_HISTORY_DEFAULT_BUFFER_SIZE> {
    /// Contains the recorded history of audio data.
    ring_buffer: RingBufferWithSerialSliceAccess<f32, N>,
    meta: AudioHistoryMeta,
}

impl<const N: usize> AudioHistory<N> {
    /// Constructor.
    pub fn new(sampling_rate: f32) -> Self {
        Self {
            ring_buffer: RingBufferWithSerialSliceAccess::new(),
            meta: AudioHistoryMeta::new(N, sampling_rate),
        }
    }

    /// Updates the internal state by receiving the next slice of new audio data.
    ///
    /// Uses the internal sampling rate as reference for calculations.
    pub fn update(&mut self, samples: &[f32]) {
        self.ring_buffer.extend_from_slice(samples);
        self.meta.update(samples);
    }

    /// Returns a continuous slice of the latest audio data kept inside the buffer. The latest
    /// audio data is at the highest index.
    ///
    /// Needs a mutable reference because the internal buffer needs to be rearranged. Does not
    /// affect meta data.
    pub fn latest_audio(&mut self) -> &[f32] {
        &self.ring_buffer.continuous_slice()
    }

    /// Wrapper around [`AudioHistoryMeta::time_per_sample`].
    #[allow(unused)]
    pub fn time_per_sample(&self) -> f64 {
        self.meta.time_per_sample()
    }

    /// Wrapper around [`AudioHistoryMeta::total_relative_time`].
    #[allow(unused)]
    pub fn total_relative_time(&self) -> f64 {
        self.meta.total_relative_time()
    }

    /// Wrapper around [`AudioHistoryMeta::amount_new_samples_on_latest_update`].
    #[allow(unused)]
    pub fn amount_new_samples_on_latest_update(&self) -> usize {
        self.meta.amount_new_samples_on_latest_update()
    }

    /// Wrapper around [`AudioHistoryMeta::amount_total_samples`].
    #[allow(unused)]
    pub fn amount_total_samples(&self) -> usize {
        self.meta.amount_total_samples()
    }

    /// Returns the capacity of the underlying ringbuffer.
    #[allow(unused)]
    pub fn capacity(&self) -> usize {
        N
    }

    /// Wrapper around [`AudioHistoryMeta::time_of_sample`].
    #[track_caller]
    #[allow(unused)]
    pub fn time_of_sample(&self, index: usize) -> f64 {
        self.meta.time_of_sample(index)
    }

    /// Wrapper around [`AudioHistoryMeta::audio_history_time`].
    #[allow(unused)]
    pub fn audio_history_time(&self) -> f64 {
        self.meta.audio_history_time()
    }

    /// Wrapper around [`AudioHistoryMeta::len`].
    #[allow(unused)]
    pub fn len(&self) -> usize {
        self.meta.len()
    }

    /// Wrapper around [`AudioHistoryMeta::calc_index_after_update`].
    #[allow(unused)]
    pub fn calc_index_after_update(&self, index: usize) -> Option<usize> {
        self.meta.calc_index_after_update(index)
    }

    /// Returns a owned copy of [`AudioHistoryMeta`] that matches the current state.
    pub fn meta(&self) -> AudioHistoryMeta {
        self.meta.clone()
    }
}

/// Relevant meta data for [`AudioHistory`] without a const parameter. This simplifies code that
/// needs access to meta data (such as time progress) without need access to the audio buffer or
/// the const parameter `N`.
///
/// This is copy so that other code can have a mutable reference to the audio buffer of
/// [`AudioHistory`] while also being able to read the corresponding meta data.
#[derive(Debug, Clone)]
pub struct AudioHistoryMeta {
    /// Buffer capacity of the corresponding audio buffer.
    buffer_capacity: usize,
    /// Sampling frequency.
    sampling_rate: f32,
    /// Time per sample. `1/sampling_rate`.
    time_per_sample: f64,
    /// The total passed relative time in seconds.
    total_relative_time: f64,
    /// The count how many samples were added to the ringbuffer during the last update.
    amount_new_samples_on_latest_update: usize,
    /// Total amount of consumed samples.
    amount_total_consumed_samples: usize,
    /// Describes the number of elements that faded out from the ring buffer in the last iteration.
    /// This is `>0` if after an update, the ringbuffer is completely filled and old elements needs
    /// to be removed.
    amount_outfaded_elements: usize,
}

impl AudioHistoryMeta {
    fn new(buffer_capacity: usize, sampling_rate: f32) -> Self {
        let time_per_sample = 1.0 / sampling_rate as f64;
        Self {
            buffer_capacity,
            sampling_rate,
            time_per_sample,
            total_relative_time: 0.0,
            amount_new_samples_on_latest_update: 0,
            amount_total_consumed_samples: 0,
            amount_outfaded_elements: 0,
        }
    }

    /// Returns the time per sample.
    pub fn time_per_sample(&self) -> f64 {
        self.time_per_sample
    }

    /// The total passed relative time in seconds.
    ///
    /// Updated by the `update`-method.
    pub fn total_relative_time(&self) -> f64 {
        self.total_relative_time
    }

    /// Returns the amount how many samples were added during the last update of the
    /// audio data ring buffer.
    ///
    /// Updated by the `update`-method.
    pub fn amount_new_samples_on_latest_update(&self) -> usize {
        self.amount_new_samples_on_latest_update
    }

    /// Returns the amount how many samples were processed in total.
    ///
    /// Updated by the `update`-method.
    pub fn amount_total_samples(&self) -> usize {
        self.amount_total_consumed_samples
    }

    /// Returns the capacity of the underlying ringbuffer.
    pub fn capacity(&self) -> usize {
        self.buffer_capacity
    }

    /// Returns the sampling rate.
    #[allow(unused)]
    pub fn sampling_rate(&self) -> f32 {
        self.sampling_rate
    }

    /// Calculates the point in time of a given sample. The time is relative to the start of the
    /// audio history but the index relative to the audio inside the audio buffer.
    ///
    /// If the audio buffer only contains a single element, index 0 corresponds to the latest data.
    /// If the audio buffer is full, index 0 corresponds to the oldest data.
    ///
    /// Note that the highest index corresponds to the latest audio data.
    /// If the buffer is yet smaller than the provided index, it returns 0.0.
    #[track_caller]
    pub fn time_of_sample(&self, index: usize) -> f64 {
        assert!(
            index < self.buffer_capacity,
            "index {} out of range [0..{}]!",
            index,
            self.buffer_capacity
        );

        assert!(
            self.amount_total_consumed_samples > 0,
            "this method only makes sense if the audio buffer contains at least one single element! index is {}",
            index
        );

        let times = (self.len() - index - 1) as f64;
        let time = self.total_relative_time - self.time_per_sample * times;
        assert!(time > 0.0, "time of a sample must be bigger than zero!");
        time
    }

    /// Returns the time of recorded audio in seconds.
    pub fn audio_history_time(&self) -> f64 {
        self.len() as f64 * self.time_per_sample
    }

    /// Returns the length of the audio history. This is either `< capacity` at the beginning of
    /// the recording or `capacity` if the buffer is full. Once full, the buffer will never be
    /// less full. Old elements will be replaced by new ones.
    pub fn len(&self) -> usize {
        if self.amount_total_consumed_samples < self.capacity() {
            self.amount_total_consumed_samples
        } else {
            self.capacity()
        }
    }

    /// Calculates the index of a sample that it has after [`Self::update`] was called. Only works
    /// of update was called once between calls to this function. There are a few corner cases:
    ///   - as long as the underlying ringbuffer is not full, the index will stay constant
    ///   - if the ring buffer is full, the index will slowly decrease (from high to low)
    ///     which describes the "transition into history/the past". This uses the amount of new
    ///     samples per call to [`Self::update`]
    ///
    /// Returns None if the index was present before but latest update but now faded out of the
    /// buffer. Returns the current index of a sample after the previous update. Panics if the
    /// index does not correspond to a sample that was previously in the buffer.
    pub fn calc_index_after_update(&self, index: usize) -> Option<usize> {
        if index < self.amount_outfaded_elements {
            None
        } else {
            let index = index - self.amount_outfaded_elements;
            assert!(index < self.len());
            Some(index)
        }
    }

    /// Updates the internal state by receiving the next slice of new audio data.
    ///
    /// Uses the internal sampling rate as reference for calculations.
    fn update(&mut self, samples: &[f32]) {
        let old_len = self.len();

        let passed_time = samples.len() as f64 * self.time_per_sample;
        self.total_relative_time += passed_time;
        self.amount_new_samples_on_latest_update = samples.len();
        self.amount_total_consumed_samples += samples.len();

        // # Prepare that calls to `calc_index_after_update` work as expected
        // 1) no elements removed from ringbuffer so far
        if old_len + samples.len() <= self.capacity() {
            self.amount_outfaded_elements = 0;
        }
        // 2) just began to fade out elements
        else if old_len <= self.capacity() && old_len + samples.len() > self.capacity() {
            self.amount_outfaded_elements = old_len + samples.len() - self.capacity();
        } else {
            self.amount_outfaded_elements = samples.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::audio_history::AudioHistory;
    use crate::test_util::read_wav_to_mono;

    #[test]
    fn test_audio_history() {
        let mut audio_history = AudioHistory::<10>::new(1.0);
        assert_eq!(audio_history.total_relative_time(), 0.0);
        assert_eq!(audio_history.amount_new_samples_on_latest_update(), 0);

        audio_history.update(&[0.0]);
        assert_eq!(audio_history.total_relative_time(), 1.0);
        assert_eq!(audio_history.amount_new_samples_on_latest_update(), 1);
        assert_eq!(audio_history.amount_total_samples(), 1);

        audio_history.update(&[0.0]);
        assert_eq!(audio_history.total_relative_time(), 2.0);
        assert_eq!(audio_history.amount_new_samples_on_latest_update(), 1);
        assert_eq!(audio_history.amount_total_samples(), 2);
    }

    #[test]
    fn test_audio_history_time_of_sample() {
        let mut audio_history = AudioHistory::<10>::new(1.0);
        assert_eq!(audio_history.len(), 0);
        audio_history.update(&[0.0; 3]);
        assert_eq!(audio_history.len(), 3);

        // we added 3 elements, they are at indices 0..3 (latest data).
        assert_eq!(audio_history.time_of_sample(0), 1.0);
        assert_eq!(audio_history.time_of_sample(1), 2.0);
        assert_eq!(audio_history.time_of_sample(2), 3.0);

        audio_history.update(&[0.0; 10]);
        assert_eq!(audio_history.len(), 10);
        assert_eq!(audio_history.amount_total_samples(), 13);
        assert_eq!(audio_history.total_relative_time(), 13.0);
        assert_eq!(audio_history.time_of_sample(0), 4.0);
        assert_eq!(audio_history.time_of_sample(9), 13.0);
    }

    #[test]
    fn test_audio_history_timings_on_real_audio() {
        let (samples, wav_header) = read_wav_to_mono("res/sample_1.lowpassed.wav");

        let mut audio_history = AudioHistory::<4096>::new(wav_header.sampling_rate as f32);

        // pretend we analyze the whole song
        samples
            .chunks(256)
            .for_each(|chunk| audio_history.update(chunk));

        assert_eq!(
            (audio_history.total_relative_time() * 1000.0).round() / 1000.0,
            7.999
        );
        assert_eq!(
            (audio_history.time_of_sample(audio_history.capacity() - 1) * 1000.0).round() / 1000.0,
            7.999
        );
    }

    /// Tests the audio history struct against a wav file. Tests if the length
    /// of the file equals the length inside the audio history.
    #[test]
    fn test_on_real_data() {
        let (audio, _wav_header) = read_wav_to_mono("res/sample_1_single_beat.wav");
        // ensure that our file corresponds to the test
        const SAMPLES_COUNT: usize = 14806;

        let mut audio_history = AudioHistory::<SAMPLES_COUNT>::new(44100.0);

        // ################################################
        let chunk_size = 25;
        audio
            .chunks(chunk_size)
            .for_each(|samples| audio_history.update(samples));
        let time_per_sample = 1.0 / 44100.0;
        assert!(
            (audio_history.total_relative_time() - time_per_sample * SAMPLES_COUNT as f64)
                < f64::EPSILON,
            "the timings in the file must correspond to the real duration of the file!"
        );
        assert_eq!(
            audio_history.amount_new_samples_on_latest_update(),
            SAMPLES_COUNT % chunk_size
        );
        assert_eq!(audio_history.amount_total_samples(), SAMPLES_COUNT);

        // ################################################
        let chunk_size = 512;
        let mut audio_history = AudioHistory::<SAMPLES_COUNT>::new(44100.0);
        audio
            .chunks(chunk_size)
            .for_each(|samples| audio_history.update(samples));
        let time_per_sample = 1.0 / 44100.0;
        assert!(
            (audio_history.total_relative_time() - time_per_sample * SAMPLES_COUNT as f64).abs()
                < f64::EPSILON,
            "the timings in the file must correspond to the real duration of the file!"
        );
        assert_eq!(
            audio_history.amount_new_samples_on_latest_update(),
            SAMPLES_COUNT % chunk_size
        );
        assert_eq!(audio_history.amount_total_samples(), SAMPLES_COUNT);
    }

    #[test]
    fn test_calc_index_after_update() {
        let mut audio_history = AudioHistory::<4>::new(1.0);

        audio_history.update(&[0.0]);
        audio_history.update(&[1.0]);
        audio_history.update(&[2.0]);
        assert_eq!(audio_history.meta.amount_outfaded_elements, 0);
        assert_eq!(
            audio_history.calc_index_after_update(0),
            Some(0),
            "must still be in buffer because buffer is not full yet"
        );
        assert_eq!(
            audio_history.calc_index_after_update(1),
            Some(1),
            "must still be in buffer because buffer is not full yet"
        );
        // should panic
        // assert_eq!(audio_history.calc_index_after_update(2), None);

        audio_history.update(&[3.0]);
        assert_eq!(audio_history.meta.amount_outfaded_elements, 0);
        assert_eq!(
            audio_history.calc_index_after_update(2),
            Some(2),
            "must still be in buffer because buffer is not full yet"
        );
        // should panic
        // assert_eq!(audio_history.calc_index_after_update(3), None);

        audio_history.update(&[4.0, 5.0]);
        assert_eq!(
            audio_history.calc_index_after_update(0),
            None,
            "index 0 must fade out of buffer"
        );
        assert_eq!(
            audio_history.calc_index_after_update(1),
            None,
            "index 0 must fade out of buffer"
        );
        assert_eq!(
            audio_history.calc_index_after_update(2),
            Some(0),
            "index 2 must become index 0"
        );
        assert_eq!(
            audio_history.calc_index_after_update(3),
            Some(1),
            "index 3 must become index 1"
        );

        audio_history.update(&[4.0, 5.0, 6.0, 7.0, 8.0]);
        (0..4).for_each(|index| {
            assert_eq!(
                audio_history.calc_index_after_update(index),
                None,
                "must fade out all indices because so many new samples were added"
            );
        });
    }
}
