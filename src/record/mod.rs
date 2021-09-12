//! Recording module. Publicly re-exports [`cpal`].

mod audio_input;
mod util;

pub use audio_input::*;
pub use util::*;
