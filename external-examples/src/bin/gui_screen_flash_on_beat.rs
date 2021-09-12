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
use beat_detector::record::cpal;
use beat_detector::record::CondVarSpinlock;
use beat_detector::BeatInfo;
use minifb::{Key, Window, WindowOptions};
use std::cell::Cell;
use std::io::stdin;
use std::sync::{Arc, Mutex};
use std::thread::{spawn, JoinHandle};
use std::time::Duration;

const WINDOW_H: usize = 600;
const WINDOW_W: usize = 600;

const REFRESH_RATE: f64 = 144.0;
const REFRESH_MS: f64 = 1.0 / REFRESH_RATE;

const COLOR_MAGENTA_RGB: (u8, u8, u8) = (0xf6, 0x53, 0xa6);
const COLOR_CYAN_RGB: (u8, u8, u8) = (0, 0xff, 0xff);
const COLOR_RED_RGB: (u8, u8, u8) = (0xff, 0xff, 0xff);
const COLOR_YELLOW_RGB: (u8, u8, u8) = (0xff, 0xff, 0x00);
const COLORS: [(u8, u8, u8); 4] = [
    COLOR_CYAN_RGB,
    COLOR_RED_RGB,
    COLOR_YELLOW_RGB,
    COLOR_MAGENTA_RGB,
];

/// This example opens a GUI window and updates it with a flash on each beat.
fn main() {
    let keep_recording = Arc::new(CondVarSpinlock::new());
    setup_ctrlc(keep_recording.clone());
    let in_dev = select_input_device();

    // each pixel has "0RGB" format. First 8 bits ignored, then red, green, and blue
    let pixel_buf = vec![0; 4 * WINDOW_H * WINDOW_W];
    let pixel_buf = Arc::new(Mutex::new(pixel_buf));
    let pixel_buf_t = pixel_buf.clone();

    let next_color_counter = Cell::new(0);
    let on_beat = move |_info: BeatInfo| {
        let color = COLORS[next_color_counter.get()];
        let color = /*0 << 24 | */(color.0 as u32) << 16 | (color.1 as u32) << 8 | color.2 as u32;
        let mut pixel_buf = pixel_buf_t.lock().unwrap();
        pixel_buf.fill(color);
        next_color_counter.replace((next_color_counter.get() + 1) % COLORS.len());
    };

    let t_handle = start_recording_thread(in_dev, keep_recording.clone(), on_beat);
    let mut window = open_gui_window();

    window.limit_update_rate(Some(Duration::from_secs_f64(REFRESH_MS)));
    while window.is_open() && !window.is_key_down(Key::Escape) {
        if keep_recording.is_stopped() {
            break;
        }
        let mut pixel_buf = pixel_buf.lock().unwrap();
        let pixels_bytes = pixel_buf.as_mut_ptr().cast::<u8>();
        let pixels_bytes =
            unsafe { std::slice::from_raw_parts_mut(pixels_bytes, pixel_buf.len() * 4) };
        for byte in pixels_bytes {
            *byte = (*byte as f32 * 0.97) as u8
        }
        let pixel_buf_cpy = pixel_buf.clone();
        // lock must be dropped, before we call update()
        // because update itself will sleep for several ms but we should not block the recording
        drop(pixel_buf);
        window
            .update_with_buffer(&pixel_buf_cpy, WINDOW_W, WINDOW_H)
            .unwrap();
    }
    println!("window loop gone");
    // do this here in case "ESC" was pressed or window closed
    keep_recording.stop_work();

    t_handle.join().unwrap();
}

fn setup_ctrlc(keep_recording: Arc<CondVarSpinlock>) {
    ctrlc::set_handler(move || {
        eprintln!("Stop recording");
        keep_recording.stop_work();
    })
    .expect("Ctrl-C handler doesn't work");
}

fn open_gui_window() -> Window {
    #[allow(clippy::field_reassign_with_default)]
    Window::new("Flash on Beat", WINDOW_W, WINDOW_H, {
        let mut options = WindowOptions::default();
        options.resize = true;
        options
    })
    .unwrap()
}

fn start_recording_thread(
    dev: cpal::Device,
    recording: Arc<CondVarSpinlock>,
    on_beat_cb: impl Fn(BeatInfo) + Send + 'static,
) -> JoinHandle<()> {
    spawn(move || {
        beat_detector::record::start_listening(Some(dev), None, recording, on_beat_cb).unwrap();
    })
}

/// Selects a audio input device. If multiple are available, user
/// is prompted on console to choose one. If only one is available,
/// it is used by default. If not device is found, the code panics.
fn select_input_device() -> cpal::Device {
    let devs = beat_detector::record::audio_input_device_list();
    if devs.is_empty() {
        panic!("No audio input devices found!")
    }
    // if only device; choose this
    if devs.len() == 1 {
        return devs.into_iter().next().unwrap().1;
    };

    println!("Available audio devices:");
    for (i, (name, _)) in devs.iter().enumerate() {
        println!("  [{}] {}", i, name);
    }
    println!("Select audio device: input device number and enter:");
    let mut input = String::new();
    stdin().read_line(&mut input).unwrap();
    let input = input
        .trim()
        .parse::<u8>()
        .expect("Input must be a valid number!");
    devs.into_iter()
        .enumerate()
        .filter(|(i, _)| *i == input as usize)
        .map(|(_i, (_name, dev))| dev)
        .take(1)
        .next()
        .unwrap()
}
