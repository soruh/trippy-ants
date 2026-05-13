#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]

mod agent;
mod config;
mod frame;
mod grid;
mod palette;
mod random;

use chrono::Local;
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use std::{
    path::Path,
    time::{Duration, Instant},
};

use crate::{
    agent::Agent, config::DEFAULT_CONFIG, frame::Frame, grid::Simulation, palette::Palette,
};

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;

/// Maximum framerate for displaying updates.
/// This saves on CPU for the actual computation.
const MAX_FPS: u64 = 30;

fn main() {
    let config = DEFAULT_CONFIG;
    let mut rng = 0xfeed_face_u32;

    let palette = Palette::<1024>::new(config.limit);

    let mut window = Window::new(
        "Trippy Ants (Space: save screenshot, Esc: quit)",
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: false,
            scale: minifb::Scale::X1,
            ..WindowOptions::default()
        },
    )
    .expect("window");

    window.set_target_fps(0); // no sleep between polls — FPS reflects CPU fire + blit cost

    let mut frames_in_window = 0u32;
    let mut window_start = Instant::now();

    let mut buffer = Simulation::new(WIDTH, HEIGHT, &config);
    let mut frame = Frame::new(WIDTH, HEIGHT);
    let mut agents = (0..config.agent_count)
        .map(|_| Agent::new(&config, WIDTH, HEIGHT, &mut rng))
        .collect::<Vec<_>>();

    let mut frame_timeout = Instant::now();
    while window.is_open() && !window.is_key_pressed(Key::Escape, KeyRepeat::No) {
        buffer.swap_buffers();
        buffer.blur();

        // limit display framerate
        if frame_timeout.elapsed() >= Duration::from_millis(1000 / MAX_FPS) {
            frame.update(&buffer.write_buffer, &palette);
            frame_timeout = Instant::now();
        }
        agents.par_iter_mut().for_each(|agent| {
            agent.update(&buffer);
        });
        buffer.update(&mut agents);

        frame.update_window(&mut window);

        if window.is_key_pressed(Key::Space, KeyRepeat::No) {
            let filename = format!(
                "trippy-ants_{}.png",
                Local::now().format("%Y-%m-%d_%H-%M-%S")
            );
            match frame.save_png(Path::new(&filename)) {
                Ok(()) => println!("saved {filename}"),
                Err(e) => eprintln!("failed to save {filename}: {e}"),
            }
        }

        frames_in_window += 1;
        let elapsed = window_start.elapsed();
        if elapsed.as_secs_f64() >= 1.0 {
            let fps = frames_in_window as f64 / elapsed.as_secs_f64();
            println!("{fps:.1} FPS");
            frames_in_window = 0;
            window_start += Duration::from_secs(1);
        }
    }
}
