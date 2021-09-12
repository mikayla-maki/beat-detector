/*
MIT License

Copyright (c) 2021 Philipp Schuster

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/
//! Module for audio recording from an audio input device via the [`cpal`]-crate.
//! This needs `std`-functionality. Publicly re-exports [`cpal`].

use crate::record::util::CondVarSpinlock;
use crate::{BeatDetector, BeatInfo};
use alloc::string::String;
use alloc::vec::Vec;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host};
// export the used cpal version
pub use cpal;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// Returns a [`cpal`] input stream object, that calls the closure `on_audio_cb`
/// everytime new audio data is available from the audio source.
///
/// # Parameters
/// - `dev` [`cpal::Device`] to open the audio input stream with
/// - `cfg` [`cpal::StreamConfig`] specifies how the input stream should be opened
///
/// # Return
/// [`cpal::Stream`]
fn get_cpal_input_stream(
    dev: cpal::Device,
    cfg: cpal::StreamConfig,
    mut on_audio_cb: impl FnMut(&[f32]) + Send + 'static,
) -> Result<cpal::Stream, ()> {
    // TODO probably I have to check if the supported input stream config
    //  supports f32. I found out that there are some devices that only support i16..
    let stream = dev
        .build_input_stream::<f32, _, _>(
            &cfg,
            move |samples, _info| {
                on_audio_cb(samples);
            },
            |e| {
                panic!("stream error: {:?}", e);
            },
        )
        .map_err(|_| ())?;
    Ok(stream)
}

/// Starts listening on the audio stream and blocks, until `keep_recording` is false.
/// Hence, this operation is blocking. The provided `strategy`will be used to detect beats.
/// If a beat is found, `on_beat_cb` gets invoked.
///
/// # Parameters
/// - `preferred_dev` Preferred audio input [`cpal::Device`]. If not set, the default input device will be used.
/// - `preferred_input_cfg` Preferred audio input [`cpal::Device`]. If not set, the default input device will be used.
/// - `stream` Audio Stream (e.g. from [`get_cpal_input_stream`])
/// - `strategy` [`StrategyKind`]
/// - `keep_recording` True as long as the recording didn't stopped.
///                    The recording may be stopped by CTRL+C.
/// - `on_beat_cb` Callback to invoke on each beat.
#[allow(clippy::result_unit_err)]
pub fn start_listening(
    preferred_dev: Option<cpal::Device>,
    preferred_input_cfg: Option<cpal::StreamConfig>,
    keep_recording: Arc<CondVarSpinlock>,
    on_beat_cb: impl Fn(BeatInfo) + Send + 'static,
) -> Result<(), ()> {
    let default_in_dev = cpal::default_host().default_input_device();
    if preferred_dev.is_none() && default_in_dev.is_none() {
        return Err(() /*TODO*/);
    }
    let in_dev = preferred_dev.unwrap_or_else(|| default_in_dev.unwrap());

    let default_in_cfg = in_dev.default_input_config();
    if preferred_input_cfg.is_none() && default_in_cfg.is_err() {
        return Err(() /*TODO*/);
    }
    let cfg = preferred_input_cfg.unwrap_or_else(|| default_in_cfg.unwrap().config());
    assert!(
        cfg.channels == 1 || cfg.channels == 2,
        "only supports one or two channels (mono or stereo)"
    );
    let is_mono = cfg.channels == 1;

    let mut detector = BeatDetector::new(cfg.sample_rate.0 as f32);
    // input stream that connects the audio data callback with the on_beat-callback
    let stream = get_cpal_input_stream(in_dev, cfg, move |samples| {
        // Stereo is a bit more expensive here, because it needs to copy data to a new vec.
        // Interleaving is LRLR (de-facto standard?)
        if is_mono {
            if let Some(beat) = detector.on_new_audio(samples) {
                on_beat_cb(beat);
            }
        } else {
            // stereo is a bit more expensive (but negligible) .. but we can't rely on, that each input device supports mono data input..
            let mono_samples = samples
                .chunks_exact(2)
                .map(|vals| (vals[0] + vals[1]) / 2.0)
                .collect::<Vec<_>>();
            if let Some(beat) = detector.on_new_audio(&mono_samples) {
                on_beat_cb(beat);
            }
        }
    })?;
    stream.play().map_err(|_e| ())?;
    keep_recording.block_until_stopped();
    stream.pause().map_err(|_e| ())?;
    Ok(())
}

/// Convenient function which helps you to select from a number of
/// audio devices using "cpal" audio library.
pub fn audio_input_device_list() -> BTreeMap<String, Device> {
    let host = cpal::default_host();
    let mut map = BTreeMap::new();
    for (i, dev) in host.input_devices().unwrap().enumerate() {
        map.insert(dev.name().unwrap_or(format!("Unknown device #{}", i)), dev);
    }
    map
}

/// Convenient function which helps you to get capabilities of
/// each audio device covered by "cpal" audio library.
pub fn print_audio_input_device_configs() {
    let host = cpal::default_host();
    for (i, dev) in host.input_devices().unwrap().enumerate() {
        eprintln!("--------");
        let name = dev.name().unwrap_or(format!("Unknown device #{}", i));
        eprintln!("[{}] default config:", name);
        eprintln!("{:#?}", dev.default_input_config().unwrap());
        // eprintln!("[{}] available input configs:", name);
        // eprintln!("{:#?}", dev.supported_input_configs().unwrap());
    }
}

pub fn get_backends() -> HashMap<String, Host> {
    cpal::available_hosts()
        .into_iter()
        .map(|id| (format!("{:?}", id), cpal::host_from_id(id).unwrap()))
        .collect::<HashMap<_, _>>()
}

#[cfg(test)]
mod tests {
    // use super::*;
}
