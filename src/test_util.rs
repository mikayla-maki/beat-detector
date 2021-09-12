use std::fs::File;
use std::path::Path;
use std::vec::Vec;
use wav::{BitDepth, Header};

/// Reads a WAV file to mono audio. Returns the samples as mono audio. Additionally, it returns
/// the sampling rate of the file.
///
/// I prefer to use WAV because I experienced timing issues with MP3.. in the sense that the
/// timing in Audacity (and similar tools) does not match the timing when I decode it in Rust.
/// This is better with wav.
pub fn read_wav_to_mono<T: AsRef<Path>>(file: T) -> (Vec<f32>, wav::Header) {
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

// I use this test to check if my stereo=>mono conversion really works. You can open the final
// WAV-file in audacity to do this verification.
#[ignore]
#[test]
fn test_read_wav_to_mono() {
    let (audio, header) = read_wav_to_mono("res/sample_1.wav");

    let mut out_file = File::create("__test_sample_1_mono.wav").unwrap();
    let header = Header {
        channel_count: 1,
        ..header
    };

    // wav::write(header, &BitDepth::ThirtyTwoFloat(audio), &mut out_file).unwrap();
    // I don't know why but I can not get it work in float format.
    let i32_vec = audio
        .iter()
        .map(|sample| (*sample * i16::MAX as f32) as i16)
        .collect();
    wav::write(header, &BitDepth::Sixteen(i32_vec), &mut out_file).unwrap();
}
