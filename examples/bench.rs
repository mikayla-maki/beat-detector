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
use beat_detector::BeatDetector;
use std::fs::File;
use std::path::Path;
use std::time::Instant;

fn main() {
    let (sample_1_audio_data, wav_header) = read_wav_to_mono("res/sample_1.wav");
    let mut detector = BeatDetector::new(wav_header.sampling_rate as f32);

    let begin = Instant::now();

    let mut count = 0;

    for chunk in sample_1_audio_data.chunks(256) {
        detector.on_new_audio(chunk);
        count += 1;
    }

    let end = Instant::now();

    println!("iterations              : {}", count);
    println!(
        "time per iteration      : {}us",
        (end - begin).as_micros() / count
    );
    println!(
        "corresponding audio time: {}s",
        256.0 / (wav_header.sampling_rate as f32)
    );
}

/// Copy from the crate-internal test utility.
fn read_wav_to_mono<T: AsRef<Path>>(file: T) -> (Vec<f32>, wav::Header) {
    let mut file = File::open(file).unwrap();
    let (header, data) = wav::read(&mut file).unwrap();

    // owning vector with original data in f32 format
    let original_data_f32 = if data.is_sixteen() {
        data.as_sixteen()
            .unwrap()
            .iter()
            .map(|sample| i16_sample_to_f32_sample(*sample))
            .collect()
    } else if data.is_thirty_two_float() {
        data.as_thirty_two_float().unwrap().clone()
    } else {
        panic!("unsupported format");
    };

    assert!(
        !original_data_f32.iter().any(|x| x.abs() > 1.0),
        "float audio data must be in interval [-1, 1]."
    );

    if header.channel_count == 1 {
        (original_data_f32, header)
    } else if header.channel_count == 2 {
        let mut mono_audio = Vec::new();
        for sample in original_data_f32.chunks(2) {
            let mono_sample = (sample[0] + sample[1]) / 2.0;
            mono_audio.push(mono_sample);
        }
        (mono_audio, header)
    } else {
        panic!("unsupported format!");
    }
}

fn i16_sample_to_f32_sample(val: i16) -> f32 {
    if val == 0 {
        0.0
    } else {
        val as f32 / i16::MAX as f32
    }
}
