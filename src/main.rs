//! Trippy Ants.
//!
//! A visually attractive simulation based on cellular automata and particle systems.
//!
//! This is the main entry point for the simulation.
//!
//! It creates the window, initializes the simulation, and runs the main loop.

#![warn(clippy::all, clippy::pedantic)]

mod agent;
mod config;
mod frame;
mod grid;
mod palette;
mod random;
mod simulation;

use chrono::Local;
use pixels::{Pixels, SurfaceTexture};
use rayon::iter::{IntoParallelRefMutIterator as _, ParallelIterator as _};
use std::{
    cmp::Ordering,
    collections::VecDeque,
    env,
    path::Path,
    process::ExitCode,
    sync::Arc,
    time::{Duration, Instant},
};
use toml::ser;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

use crate::{
    agent::Agent,
    config::{ConfigWatcher, DEFAULT_CONFIG},
    frame::Frame,
    palette::Palette,
    simulation::Simulation,
};

/// Width of the simulation and frame buffer in pixels.
const WIDTH: u16 = 1920;

/// Height of the simulation and frame buffer in pixels.
const HEIGHT: u16 = 1080;

/// Maximum framerate for displaying updates.
/// This saves on CPU for the actual computation.
const MAX_FPS: u64 = 30;

/// Maximum Number of FPS samples to keep around.
const FPS_HISTORY_MAX: usize = 60;

/// Simulation App state.
/// Contains all state needed to render a frame.
struct App<'frame, const PALETTE_RES: usize> {
    /// The config watcher, used for live config changes.
    config_watcher: ConfigWatcher,

    /// The Frame which represents the render buffer.
    frame: Option<Frame<'frame>>,

    /// The Window we are rendering to.
    window: Option<Arc<Window>>,

    /// The Window attributes of our window.
    window_attributes: WindowAttributes,

    /// The `FPS_HISTORY_MAX` most recent FPS measurements.
    fps_samples: VecDeque<f64>,

    /// Temporary buffer used to compute the median FPS.
    median_buffer: Vec<f64>,

    /// The color palette.
    palette: Palette<PALETTE_RES>,

    /// The simulation state.
    simulation: Simulation,

    /// The active agents.
    agents: Vec<Agent>,

    /// Time at which the last frame was render to the surface.
    frame_timeout: Instant,

    /// Time since the FPS was last calulcated.
    last_fps_calculation: Instant,

    /// Number of frames which elapsed since the last FPS calculation.
    frames_in_window: u32,
}

impl<const PALETTE_RES: usize> App<'_, PALETTE_RES> {
    /// Initialize the App, optionally with a config path to load.
    ///
    /// # Errors
    /// - returns an error if the Config at `config_path` could not be loaded.
    fn new(config_path: Option<String>) -> Result<Self, String> {
        let window_attributes = WindowAttributes::default()
            .with_title("Trippy Ants (Space: save screenshot, Esc: quit)")
            .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT))
            .with_resizable(false);

        let mut config_watcher = ConfigWatcher::new();

        let config = if let Some(config_file) = config_path {
            config_watcher.load_config(config_file)?
        } else {
            println!("no config file provided, using default config");
            DEFAULT_CONFIG
        };

        if let Ok(config_str) = ser::to_string(&config) {
            println!("loaded config:\n{config_str}");
        }

        let palette = Palette::<PALETTE_RES>::new(&config.colors);

        let frames_in_window = 0_u32;
        let window_start = Instant::now();

        let simulation = Simulation::new(WIDTH, HEIGHT, &config.world);
        let agents = (0..config.agent.count)
            .map(|index| {
                let index = u32::try_from(index).unwrap_or(u32::MAX);
                Agent::new(&config, WIDTH, HEIGHT, index)
            })
            .collect::<Vec<_>>();

        let frame_timeout = Instant::now();

        Ok(Self {
            config_watcher,
            frame: None,
            window: None,
            window_attributes,
            fps_samples: VecDeque::with_capacity(FPS_HISTORY_MAX),
            median_buffer: Vec::with_capacity(FPS_HISTORY_MAX),
            palette,
            simulation,
            agents,
            frame_timeout,
            last_fps_calculation: window_start,
            frames_in_window,
        })
    }

    /// Update the config if the `config_watcher` found an updated config.
    fn update_config(&mut self) {
        if let Some(new_config) = self.config_watcher.watch_for_update() {
            println!("config updated");
            if let Ok(config_str) = ser::to_string(&new_config) {
                println!("loaded config:\n{config_str}");
            }
            for (index, agent) in self.agents.iter_mut().enumerate() {
                let index = u32::try_from(index).unwrap_or(u32::MAX);
                agent.update_config(&new_config.agent, index);
            }
            self.simulation.update_config(&new_config.world);

            self.palette = Palette::<PALETTE_RES>::new(&new_config.colors);

            while self.agents.len() < new_config.agent.count {
                let index = u32::try_from(self.agents.len()).unwrap_or(u32::MAX);
                self.agents
                    .push(Agent::new(&new_config, WIDTH, HEIGHT, index));
            }
            self.agents.truncate(new_config.agent.count);
        }
    }

    /// Update the FPS counter after rending a frame.
    fn update_fps(&mut self) {
        self.frames_in_window += 1;

        let elapsed = self.last_fps_calculation.elapsed();
        if elapsed.as_secs_f64() >= 1.0 {
            let fps = f64::from(self.frames_in_window) / elapsed.as_secs_f64();

            while self.fps_samples.len() > FPS_HISTORY_MAX {
                _ = self.fps_samples.pop_front();
            }
            self.fps_samples.push_back(fps);

            self.median_buffer.clear();
            for &sample in &self.fps_samples {
                self.median_buffer.push(sample);
            }

            #[expect(clippy::min_ident_chars, reason = "these names are fine")]
            self.median_buffer
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Less));

            let i_mid = self.median_buffer.len() / 2;
            #[expect(clippy::indexing_slicing, reason = "checked above")]
            let fps_average = if self.median_buffer.len().is_multiple_of(2) {
                self.median_buffer[i_mid - 1].midpoint(self.median_buffer[i_mid])
            } else {
                self.median_buffer[i_mid]
            };
            println!("{fps:.1} FPS | {fps_average:.1} MEDIAN");
            self.frames_in_window = 0;
            self.last_fps_calculation += Duration::from_secs(1);
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "We want to ignore all other events"
)]
impl<const PALETTE_RES: usize> ApplicationHandler for App<'_, PALETTE_RES> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let win = Arc::from(
            event_loop
                .create_window(self.window_attributes.clone())
                .expect("Failed to create window"),
        );
        let window_size = win.inner_size();

        // Build Pixels surface pinned safely to the new Window Arc reference clone
        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, Arc::clone(&win));
        let pixels = Pixels::new(u32::from(WIDTH), u32::from(HEIGHT), surface_texture)
            .expect("Failed to initialize Pixels buffer");

        self.frame = Some(Frame::new(WIDTH, HEIGHT, pixels));
        self.window = Some(win);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        #[expect(
            clippy::pattern_type_mismatch,
            reason = "We want a mutable refernce to each field"
        )]
        let Self {
            frame,
            window,
            palette,
            simulation,
            agents,
            frame_timeout,
            ..
        } = self;

        let Some(frame) = frame.as_mut() else {
            return;
        };

        let Some(window) = window.as_ref() else {
            return;
        };

        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "We want to ignore all other events"
        )]
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: input_event, ..
            } if input_event.state.is_pressed() => match input_event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                PhysicalKey::Code(KeyCode::Space) => {
                    let filename = format!(
                        "trippy-ants_{}.png",
                        Local::now().format("%Y-%m-%d_%H-%M-%S")
                    );
                    match frame.save_png(Path::new(&filename)) {
                        Ok(()) => println!("saved {filename}"),
                        Err(error) => eprintln!("failed to save {filename}: {error}"),
                    }
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                // immediately request another redraw to render as many frames as possible
                window.request_redraw();

                simulation.swap_buffers();
                simulation.blur();

                if frame_timeout.elapsed() >= Duration::from_millis(1000 / MAX_FPS) {
                    *frame_timeout = Instant::now();

                    if let Err(err) = frame.update(&simulation.write_buffer, palette) {
                        eprintln!("pixels.render() failed: {err}");
                        event_loop.exit();
                    }
                }

                agents
                    .par_iter_mut()
                    .for_each(|agent| agent.update(simulation));

                simulation.update(agents);

                self.update_fps();
                self.update_config();
            }
            _ => {}
        }
    }
}

fn main() -> ExitCode {
    let mut app = match App::<1024>::new(env::args().nth(1)) {
        Ok(app) => app,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let Ok(event_loop) = EventLoop::new() else {
        eprintln!("Failed to initialize window event loop");
        return ExitCode::FAILURE;
    };
    event_loop.set_control_flow(ControlFlow::Poll);
    let run_result = event_loop.run_app(&mut app);

    match run_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
