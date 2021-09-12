/// A special custom ringbuffer implementation entirely on the stack suited for the use case in
/// [`crate::audio_history::AudioHistory`]. It always allows serial access to the data in a
/// dedicated slice.
#[derive(Debug)]
pub(crate) struct RingBufferWithSerialSliceAccess<T: Default + Copy, const BUF_LEN: usize> {
    /// Buffer for the actual data inside the ring buffer.
    buffer: [T; BUF_LEN],
    /// Memory used to rearrange entries from the buffer to be continuous.
    /// The oldest value stands at the lowest index. The newest value stands
    /// at the highest index (`BUF_LEN - 1`).
    continuous_slice_buffer: [T; BUF_LEN],
    /// Tells whether `continuous_slice_buffer` equals the data inside the ringbuffer or not.
    continuous_slice_buffer_valid: bool,
    /// Write pointer for the ring buffer. Points to the oldest element in the
    /// collection, i.e., the one to overwrite next.
    write_index: usize,
    /// Number of elements in the buffer. Initially 0 and eventually `BUF_LEN` (capacity).
    len: usize,
}

impl<T: Default + Copy, const BUF_LEN: usize> RingBufferWithSerialSliceAccess<T, BUF_LEN> {
    /// Initializes a new ring buffer on the stack. It is filled with the default value of T.
    /// The length immediately equals the capacity.
    pub fn new() -> Self {
        #[cfg(test)]
        eprintln!(
            "RingBuffer::new(): consumes {} bytes on the stack.",
            core::mem::size_of::<Self>()
        );
        log::trace!(
            "RingBuffer::new(): consumes {} bytes on the stack.",
            core::mem::size_of::<Self>()
        );
        Self {
            buffer: [T::default(); BUF_LEN],
            continuous_slice_buffer: [T::default(); BUF_LEN],
            continuous_slice_buffer_valid: true,
            write_index: 0,
            len: 0,
        }
    }

    /// Pushes a new element and forgets the oldest element.
    pub fn push(&mut self, item: T) {
        self.buffer[self.write_index] = item;
        self.continuous_slice_buffer_valid = false;
        self.write_index = (self.write_index + 1) % BUF_LEN;
        if self.len < self.capacity() {
            self.len += 1;
        }
    }

    /*/// Returns a reference to the last item written.
    pub fn latest(&self) -> &T {
        let index = if self.write_index == 0 {
            BUF_LEN - 1
        } else {
            (self.write_index - 1) % BUF_LEN
        };
        &self.buffer[index]
    }*/

    /// Extends the ring buffer from a slice. Clones each element..
    pub fn extend_from_slice(&mut self, new_data: &[T]) {
        for val in new_data {
            self.push(*val);
        }
    }

    /// Resets the state as if the buffer is empty.
    pub fn clear(&mut self) {
        self.write_index = 0;
        self.continuous_slice_buffer_valid = false;
        self.len = 0;
    }

    /// Returns a continuous slice of the underlying data. The oldest data is on the lowest
    /// index and the newest data on the highest index.
    ///
    /// Needs mutable self because the continuous slice needs to be created at first.
    pub fn continuous_slice(&mut self) -> &[T] {
        // small optimization :) - rather rare but saves a memcpy()

        let skip_elements = self.capacity() - self.len;

        if self.write_index == 0 {
            // #[cfg(test)]
            // eprintln!("continuous slice optimization works!");
            &self.buffer[skip_elements..skip_elements + self.len]
        } else {
            self.prepare_continuous_slice();
            &self.continuous_slice_buffer[skip_elements..skip_elements + self.len]
        }
    }

    /// Returns the capacity.
    pub fn capacity(&self) -> usize {
        BUF_LEN
    }

    /*/// Returns the length.
    pub const fn len(&self) -> usize {
        self.len
    }*/

    /// Prepares the continuous slice by storing all values inside the ring buffer in a
    /// continuous memory region.
    ///
    /// Needs mutable self because the slice needs to be created at first.
    fn prepare_continuous_slice(&mut self) {
        if self.continuous_slice_buffer_valid {
            // already valid, fast return
            return;
        }

        // copy step 1/2: copy oldest data to begin of slice
        (self.write_index..BUF_LEN)
            .enumerate()
            .for_each(|(slice_index, data_index)| {
                self.continuous_slice_buffer[slice_index] = self.buffer[data_index]
            });
        // copy step 2/2: copy freshest data to end of slice
        (0..self.write_index)
            .enumerate()
            .map(|(slice_index, data_index)| {
                // map slice index to end of continuous slice
                (slice_index + (self.write_index..BUF_LEN).len(), data_index)
            })
            .for_each(|(slice_index, data_index)| {
                self.continuous_slice_buffer[slice_index] = self.buffer[data_index]
            });

        self.continuous_slice_buffer_valid = true;
    }
}

#[cfg(test)]
mod tests {

    use crate::util::audio_ring_buffer::RingBufferWithSerialSliceAccess;

    #[test]
    fn test_ring_buffer_1() {
        let mut buf = RingBufferWithSerialSliceAccess::<u8, 4>::new();
        assert_eq!(buf.continuous_slice(), &[]);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.continuous_slice(), &[1, 2, 3]);
        buf.push(4);
        assert_eq!(buf.continuous_slice(), &[1, 2, 3, 4]);
        buf.push(5);
        assert_eq!(buf.continuous_slice(), &[2, 3, 4, 5]);
        buf.extend_from_slice(&[6, 7, 8]);
        assert_eq!(buf.continuous_slice(), &[5, 6, 7, 8]);
    }

    #[test]
    fn test_ring_buffer_2() {
        let mut buf = RingBufferWithSerialSliceAccess::<u8, 2>::new();
        buf.push(1);
        buf.push(2);
        buf.push(3);
        buf.push(4);
        assert_eq!(buf.continuous_slice(), &[3, 4]);
        // flag not touched because I used the optimization in continuous_slice()
        assert!(!buf.continuous_slice_buffer_valid);
        buf.push(5);
        assert_eq!(buf.continuous_slice(), &[4, 5]);
        // flag touched because the optimization was not taken
        assert!(buf.continuous_slice_buffer_valid);
    }

    /*#[test]
    fn test_ring_buffer_latest() {
        let mut buf = RingBufferWithSerialSliceAccess::<u8, 3>::new();
        buf.push(1);
        assert_eq!(buf.latest(), &1);
        buf.push(2);
        assert_eq!(buf.latest(), &2);
        buf.push(3);
        assert_eq!(buf.latest(), &3);
        buf.push(4);
        assert_eq!(buf.latest(), &4);
        buf.push(5);
        assert_eq!(buf.latest(), &5);
    }*/
}
