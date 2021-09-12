use crate::peak::zero_of_function_iterator::ZeroOfFunctionIterator;
/// Iterator over the local maxima and minima of a function between zeroes of a function. To get
/// clear results, it ignores values before the first and the last zero of the function. Makes no
/// assumptions about the range of values, i.e., it will also include very low minima and maxima.
/// It is the responsibility of the next higher level abstraction to cope with this.
///
/// Example: In `[0, 1, 2, 1, 0]` `2` is a local maximum. In `[0, 2, 1, 3, 2, 1, 0]` `3` is the
/// local maximum.
#[derive(Debug)]
pub(super) struct LocalMinMaxIterator<'a> {
    /// Samples to search in.
    samples: &'a [f32],
    /// Progress. Somewhere between `0` and `samples.len()`. Holds the iteration progress.
    /// Always points to a index where the amplitude crossed the zero line (compared to the
    /// previous index).
    next_start_index: usize,
    zero_of_function_iterator: ZeroOfFunctionIterator<'a>,
}

impl<'a> LocalMinMaxIterator<'a> {
    /// Creates a new [`LocalMinMaxIterator`].
    ///
    /// # Parameters
    /// - `samples` - graph/amplitude to operate on. Expects that only valid numbers, i.e. not
    ///               NaN or infinite, are provided.
    /// - `preferred_start_index` - Optional start index to start the search for the next local
    ///                             min/max.
    pub(super) fn new(samples: &'a [f32], preferred_start_index: Option<usize>) -> Self {
        debug_assert!(
            samples.iter().all(|x| x.is_finite()),
            "only regular/normal f32 samples allowed!"
        );

        if let Some(index) = preferred_start_index {
            assert!(index < samples.len());
        }

        let mut zero_of_function_iterator =
            ZeroOfFunctionIterator::new(samples, preferred_start_index);
        let start_index = preferred_start_index.unwrap_or(0);
        // this init is important: we must guarantee that local minima and maxima are only found
        // if they are clearly encapsulated by two zeros of a function. Otherwise, we get odd
        // peaks that are no actual peaks. Background: Because audio is updated over time, peaks
        // slowly fade out of the the window but the border might be in the middle of an rising
        // amplitude.
        let start_index = if samples[start_index] == 0.0 {
            start_index
        } else {
            // by using `samples.len()` as fall back we make sure that the next call to
            // `.next` returns `None`
            zero_of_function_iterator.next().unwrap_or(samples.len())
        };

        Self {
            samples,
            next_start_index: start_index,
            zero_of_function_iterator,
        }
    }
}

impl<'a> Iterator for LocalMinMaxIterator<'a> {
    type Item = LocalMinMax;

    fn next(&mut self) -> Option<Self::Item> {
        // start of the current iteration (this is a zero of a function) (left bound)
        let start_index = self.next_start_index;
        // end of the current iteration (this is a zero of the function) (right bound)
        // -> this is also the begin of the next iteration
        let next_zero_of_function_index = self.zero_of_function_iterator.next()?;
        // -> so we update it
        self.next_start_index = next_zero_of_function_index;

        debug_assert!(
            start_index < next_zero_of_function_index,
            "should always be true, otherwise error in algorithm: {} not < {}",
            start_index,
            next_zero_of_function_index
        );

        // Find the minimum or maximum by using a reduce operation.
        self.samples
            .iter()
            .enumerate()
            // I chose this way over "samples[a..b]" because I need the proper index of each element!
            .skip(start_index)
            .take(next_zero_of_function_index - start_index)
            .reduce(|(index_l, val_l), (index_r, val_r)| {
                if libm::fabsf(*val_l) > libm::fabsf(*val_r) {
                    (index_l, val_l)
                } else {
                    (index_r, val_r)
                }
            })
            .map(|(index, val)| LocalMinMax::new(index, *val))
    }
}

/// Internal version of a [`crate::peak::Peak`].
#[derive(Debug, PartialEq)]
pub(super) struct LocalMinMax {
    /// The index of the value in the array of samples.
    pub(super) index: usize,
    /// The amplitude at `index` inside the samples array.
    pub(super) value: f32,
}

impl LocalMinMax {
    /// Constructor.
    const fn new(index: usize, value: f32) -> Self {
        Self { value, index }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    // general test
    #[test]
    fn test_find_next_local_minmax() {
        let input = [0.0, 0.0];
        let mut iterator = LocalMinMaxIterator::new(&input, None);
        let maybe_peak = iterator.next();
        assert!(
            maybe_peak.is_none(),
            "only zeroes - no local minimum or maximum!"
        );

        let input = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut iterator = LocalMinMaxIterator::new(&input, None);
        let maybe_peak = iterator.next();
        assert!(
            maybe_peak.is_none(),
            "only zeroes - no local minimum or maximum!"
        );

        let input = [0.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        let mut iterator = LocalMinMaxIterator::new(&input, None);
        let maybe_peak = iterator.next();
        assert!(
            maybe_peak.is_none(),
            "no clear minimum or maximum detectable at end of samples array"
        );

        let input = [0.0, 0.0, 0.0, -0.1, 1.0, 1.1, 0.0, 0.0, -2.0, 0.0];
        let mut iterator = LocalMinMaxIterator::new(&input, None);
        let maybe_peak = iterator.next();
        assert_eq!(
            LocalMinMax {
                index: 3,
                value: -0.1,
            },
            maybe_peak.unwrap(),
            "must skip zeroes at beginning and return local maximum at end of wave"
        );

        // ---------

        let input = [1.0, 2.0, 1.0, 3.0, 0.0];
        let mut iterator = LocalMinMaxIterator::new(&input, None);
        let maybe_peak = iterator.next();
        assert_eq!(
            None, maybe_peak,
            "no clear zeros of function available, thus no peaks"
        );

        // ---------

        let input = [0.0, 1.1, 2.2, 1.1, 0.0, -1.1, -2.2, -1.1, 0.0];
        let mut iterator = LocalMinMaxIterator::new(&input, None);

        let maybe_peak = iterator.next();
        assert_eq!(
            LocalMinMax {
                index: 2,
                value: 2.2,
            },
            maybe_peak.unwrap()
        );

        let maybe_peak = iterator.next();
        assert_eq!(
            LocalMinMax {
                index: 6,
                value: -2.2,
            },
            maybe_peak.unwrap()
        );

        let maybe_peak = iterator.next();
        assert!(maybe_peak.is_none());

        // ---------

        let input = [0.0, 0.1, 0.2, 0.3, 14.0, 0.1, -0.1];
        let mut iterator = LocalMinMaxIterator::new(&input, None);
        let maybe_peak = iterator.next();
        assert_eq!(
            LocalMinMax {
                index: 4,
                value: 14.0,
            },
            maybe_peak.unwrap()
        );

        // ---------

        let input = [0.0, 1.0, 0.8, 0.6, 0.5, 0.2, -0.5];
        let mut iterator = LocalMinMaxIterator::new(&input, None);
        let maybe_peak = iterator.next();
        assert_eq!(
            LocalMinMax {
                index: 1,
                value: 1.0,
            },
            maybe_peak.unwrap()
        );
    }

    // test that the preferred start index value works as expected
    #[test]
    fn test_preferred_begin_index() {
        let test_data = [0.0, -0.2, -0.4, -0.2, 0.0, 0.2, 0.4, 0.2, 0.0];

        let mut iterator = LocalMinMaxIterator::new(&test_data, Some(0));
        assert_eq!(iterator.next().unwrap().index, 2);
        assert_eq!(iterator.next().unwrap().index, 6);
        assert_eq!(iterator.next(), None);

        for start_index in 1..=4 {
            let mut iterator = LocalMinMaxIterator::new(&test_data, Some(start_index));
            assert_eq!(iterator.next().unwrap().index, 6);
            assert_eq!(iterator.next(), None);
        }

        for start_index in 5..9 {
            let mut iterator = LocalMinMaxIterator::new(&test_data, Some(start_index));
            assert_eq!(iterator.next(), None);
        }
    }
}
