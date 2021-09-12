/// Iterator over the zeroes of a function. A zero of a function is the point where the graph
/// crosses the zero line. For sequences that start and consist only of `0.0`, the iterator skips
/// those and searches for the next increase/decrease of the graph and starts it search from there.
#[derive(Debug)]
pub(super) struct ZeroOfFunctionIterator<'a> {
    /// Samples to search in.
    samples: &'a [f32],
    /// Progress. Somewhere between `0` and `samples.len()`. Holds the iteration progress.
    index: usize,
}

impl<'a> ZeroOfFunctionIterator<'a> {
    /// Creates a new [`ZeroOfFunctionIterator`].
    ///
    /// # Parameters
    /// - `samples`     - graph/amplitude to operate on. Expects that only valid numbers, i.e. not
    ///                   NaN or infinite, are provided.
    /// - `preferred_start_index` - Optional start index. Always from the beginning, even if
    ///                             direction is specified as [`Direction::Backward`]
    pub(super) fn new(samples: &'a [f32], preferred_start_index: Option<usize>) -> Self {
        debug_assert!(
            samples.iter().all(|x| x.is_finite()),
            "only regular/normal f32 samples allowed!"
        );
        if let Some(index) = preferred_start_index {
            assert!(index < samples.len());
        }
        Self {
            samples,
            index: preferred_start_index.unwrap_or(0),
        }
    }
}

impl<'a> Iterator for ZeroOfFunctionIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        // An iterator that iterates over (n, n+1)-pairs of the samples array. Skips as many
        // elements as specified.
        let pairwise_iterator = self
            .samples
            .iter()
            .enumerate()
            // skip the elements that we already checked
            .skip(self.index)
            .zip(
                self.samples
                    .iter()
                    // skip the elements that we already checked
                    .skip(self.index + 1),
            );

        let index = pairwise_iterator
            // skip elements that are 0.0 from our current start index
            // attention! not use filter here! We only want to skip zeroes as long as there
            // are zeroes. If after a series of zeroes other values were found, zeroes are valid
            // after that.
            .skip_while(|((_, current), _)| **current == 0.0)
            // now check if the next element crosses the zero line
            .find(|((_index, current), next)| {
                // left: crosses from positive to negative
                // right: crosses from negative to positive
                **current > 0.0 && **next <= 0.0 || **current < 0.0 && **next >= 0.0
            })
            // Add plus one because the next element is a zero => we want the index of it
            .map(|((current_element_index, _), _)| current_element_index + 1)?;

        self.index = index;

        Some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // general test
    #[test]
    fn test_zero_of_function_iterator() {
        let input = [0.0, 0.0, 0.0, 0.0];
        let mut iterator = ZeroOfFunctionIterator::new(&input, None);
        (0..4).for_each(|_index| {
            assert_eq!(iterator.next(), None);
        });

        let input = [0.0, 1.0, 0.0, 0.0];
        let mut iterator = ZeroOfFunctionIterator::new(&input, None);
        assert_eq!(iterator.next(), Some(2));
        assert_eq!(iterator.next(), None);

        let input = [2.0, 1.0, 0.0, 0.0];
        let mut iterator = ZeroOfFunctionIterator::new(&input, None);
        assert_eq!(iterator.next(), Some(2));
        assert_eq!(iterator.next(), None);

        let input = [0.0, -2.0, 4.0, -8.0, 0.1, 0.0];
        let mut iterator = ZeroOfFunctionIterator::new(&input, None);
        assert_eq!(iterator.next(), Some(2));
        assert_eq!(iterator.next(), Some(3));
        assert_eq!(iterator.next(), Some(4));
        assert_eq!(iterator.next(), Some(5));
        assert_eq!(iterator.next(), None);

        let input = [0.0, -0.5, -0.5, 0.0, 0.5, 0.5, 0.0];
        let mut iterator = ZeroOfFunctionIterator::new(&input, None);
        assert_eq!(iterator.next(), Some(3));
        assert_eq!(iterator.next(), Some(6));
        assert_eq!(iterator.next(), None);
    }

    // test that the preferred start index value works as expected
    #[test]
    fn test_preferred_begin_index() {
        let test_data = [0.0, -0.2, -0.4, -0.2, 0.0, 0.2, 0.4, 0.2, 0.0];

        for start_index in 0..3 {
            let mut iterator = ZeroOfFunctionIterator::new(&test_data, Some(start_index));
            assert_eq!(iterator.next().unwrap(), 4);
            assert_eq!(iterator.next().unwrap(), 8);
            assert_eq!(iterator.next(), None);
        }

        for start_index in 4..7 {
            let mut iterator = ZeroOfFunctionIterator::new(&test_data, Some(start_index));
            assert_eq!(iterator.next().unwrap(), 8);
            assert_eq!(iterator.next(), None);
        }

        let mut iterator = ZeroOfFunctionIterator::new(&test_data, Some(8));
        assert_eq!(iterator.next(), None);
    }
}
