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
use crossbeam::utils::CachePadded;
use pixels::{Pixels, SurfaceTexture};
use rayon::iter::{IntoParallelRefMutIterator as _, ParallelIterator as _};
use std::{
    cmp::Ordering,
    collections::VecDeque,
    env,
    path::Path,
    process::ExitCode,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering as AtomicOrdering},
        mpsc,
    },
    thread,
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
    config::{Config, ConfigWatcher, DEFAULT_CONFIG},
    frame::Frame,
    grid::Grid,
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

/// Maximum Number of SPS (Steps Per Second) samples to keep around.
const SPS_HISTORY_MAX: usize = 60;

/// Time per rendered frame.
#[expect(clippy::cast_possible_truncation, reason = "this is a const...")]
const FRAME_TIME: Duration =
    Duration::from_nanos(Duration::from_secs(1).as_nanos() as u64 / MAX_FPS);

/// Simulation App state.
/// Contains all state needed to render a frame.
struct App<'app, 'frame, const PALETTE_RES: usize> {
    /// The config watcher, used for live config changes.
    config_watcher: ConfigWatcher,

    /// The Frame which represents the render buffer.
    frame: Option<Frame<'frame>>,

    /// The Window we are rendering to.
    window: Option<Arc<Window>>,

    /// The Window attributes of our window.
    window_attributes: WindowAttributes,

    /// The color palette.
    palette: Palette<PALETTE_RES>,

    /// Grid which is rendered to the screen.
    render_grid: &'app Mutex<Grid>,

    /// Flag to signal the simulation thread to terminate.
    is_running: &'app AtomicBool,

    /// Flag to request the simulation thread to update the render grid.
    request_update: &'app AtomicBool,

    /// Channel to wait for the simulation thread to finish updating the render grid.
    grid_done_rx: mpsc::Receiver<()>,

    /// Channel to send configuration updates to the simulation thread.
    config_tx: mpsc::Sender<Config>,

    /// Time at which the next frame is due to be rendered.
    frame_timeout: Instant,
}

impl<'app, const PALETTE_RES: usize> App<'app, '_, PALETTE_RES> {
    /// Initialize the App.
    fn new(
        render_grid: &'app Mutex<Grid>,
        is_running: &'app AtomicBool,
        request_update: &'app AtomicBool,
        grid_done_rx: mpsc::Receiver<()>,
        config_tx: mpsc::Sender<Config>,
        config_watcher: ConfigWatcher,
        palette: Palette<PALETTE_RES>,
    ) -> Self {
        let window_attributes = WindowAttributes::default()
            .with_title("Trippy Ants (Space: save screenshot, Esc: quit)")
            .with_inner_size(PhysicalSize::new(WIDTH, HEIGHT))
            .with_resizable(false);

        Self {
            config_watcher,
            frame: None,
            window: None,
            window_attributes,
            palette,
            render_grid,
            is_running,
            request_update,
            grid_done_rx,
            config_tx,
            frame_timeout: Instant::now() + FRAME_TIME,
        }
    }

    /// Update the config if the `config_watcher` found an updated config.
    fn update_config(&mut self) {
        if let Some(new_config) = self.config_watcher.watch_for_update() {
            println!("config updated");
            if let Ok(config_str) = ser::to_string(&new_config) {
                println!("loaded config:\n{config_str}");
            }

            self.palette = Palette::<PALETTE_RES>::new(&new_config.colors);

            // Send config ownership to the simulation thread
            _ = self.config_tx.send(new_config);
        }
    }

    /// Helper to cleanly exit.
    fn exit(&self, event_loop: &ActiveEventLoop) {
        self.is_running.store(false, AtomicOrdering::Relaxed);
        event_loop.exit();
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "We want to ignore all other events"
)]
impl<const PALETTE_RES: usize> ApplicationHandler for App<'_, '_, PALETTE_RES> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let win = Arc::from(
            event_loop
                .create_window(self.window_attributes.clone())
                .expect("Failed to create window"),
        );
        let window_size = win.inner_size();

        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, Arc::clone(&win));
        let pixels = Pixels::new(u32::from(WIDTH), u32::from(HEIGHT), surface_texture)
            .expect("Failed to initialize Pixels buffer");

        self.frame = Some(Frame::new(WIDTH, HEIGHT, pixels));
        self.window = Some(win);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if Instant::now() >= self.frame_timeout
            && let Some(window) = &self.window
        {
            window.request_redraw();
        }
        // This instructs the winit event loop to sleep the thread until the exact timeout.
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.frame_timeout));
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
            render_grid,
            request_update,
            grid_done_rx,
            frame_timeout,
            ..
        } = self;

        let Some(frame) = frame.as_mut() else {
            return;
        };

        let Some(_window) = window.as_ref() else {
            return;
        };

        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "We want to ignore all other events"
        )]
        match event {
            WindowEvent::CloseRequested => self.exit(event_loop),
            WindowEvent::KeyboardInput {
                event: input_event, ..
            } if input_event.state.is_pressed() => match input_event.physical_key {
                PhysicalKey::Code(KeyCode::Escape) => self.exit(event_loop),
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
                // Request grid update from simulation thread
                request_update.store(true, AtomicOrdering::Release);

                // Wait for the simulation thread to finish the copy
                if grid_done_rx.recv().is_ok() {
                    let grid = render_grid.lock().expect("Failed to lock state for render");
                    frame.update(&grid, palette);
                }

                if let Err(err) = frame.render() {
                    eprintln!("pixels.render() failed: {err}");
                    self.exit(event_loop);
                    return;
                }

                // Advance the timeout. A loop prevents queueing instant back-to-back renders
                // if the application lagged momentarily.
                let now = Instant::now();
                while *frame_timeout <= now {
                    *frame_timeout += FRAME_TIME;
                }

                self.update_config();
            }
            _ => {}
        }
    }
}

fn main() -> ExitCode {
    let mut config_watcher = ConfigWatcher::new();

    let config = if let Some(config_file) = env::args().nth(1) {
        match config_watcher.load_config(config_file) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        println!("no config file provided, using default config");
        DEFAULT_CONFIG
    };

    if let Ok(config_str) = ser::to_string(&config) {
        println!("loaded config:\n{config_str}");
    }

    let palette = Palette::<1024>::new(&config.colors);
    let mut simulation = Simulation::new(WIDTH, HEIGHT, &config.world);
    let mut agents = (0..config.agent.count)
        .map(|index| {
            let index = u32::try_from(index).unwrap_or(u32::MAX);
            Agent::new(&config, WIDTH, HEIGHT, index)
        })
        .collect::<Vec<_>>();

    let render_grid = Mutex::new(Grid::new(WIDTH, HEIGHT, config.world.topology));
    let is_running = CachePadded::new(AtomicBool::new(true));
    let request_update = CachePadded::new(AtomicBool::new(true));

    let (grid_done_tx, grid_done_rx) = mpsc::channel();
    let (config_tx, config_rx) = mpsc::channel::<Config>();

    let Ok(event_loop) = EventLoop::new() else {
        eprintln!("Failed to initialize window event loop");
        return ExitCode::FAILURE;
    };

    thread::scope(|scope| {
        let render_grid = &render_grid;
        let is_running = &is_running;
        let request_update = &request_update;

        let simulation_thread = scope.spawn(
            

            move || {
            let mut steps_in_window = 0_u32;
            let mut last_sps_calculation = Instant::now();
            let mut sps_samples = VecDeque::with_capacity(SPS_HISTORY_MAX);
            let mut median_buffer = Vec::with_capacity(SPS_HISTORY_MAX);

            // Accumulators for profiling
            let mut time_grid_update = Duration::ZERO;
            let mut time_config_update = Duration::ZERO;
            let mut time_swap = Duration::ZERO;
            let mut time_blur = Duration::ZERO;
            let mut time_agents = Duration::ZERO;
            let mut time_apply_agents = Duration::ZERO;
            let mut time_bc = Duration::ZERO;

       
            while is_running.load(AtomicOrdering::Relaxed) {
                // Check if the render thread is waiting for a frame
                let t_grid = Instant::now();
                if request_update.swap(false, AtomicOrdering::Acquire) {
                    let mut grid = render_grid.lock().expect("Failed to lock render grid");
                    grid.cells_mut()
                        .copy_from_slice(simulation.write_buffer.cells());
                    _ = grid_done_tx.send(()); // Signal that the grid is updated
                }
                time_grid_update += t_grid.elapsed();

                // Check if a new config is available
                let t_config = Instant::now();
                while let Ok(new_config) = config_rx.try_recv() {
                    for (index, agent) in agents.iter_mut().enumerate() {
                        let index = u32::try_from(index).unwrap_or(u32::MAX);
                        agent.update_config(&new_config.agent, index);
                    }
                    simulation.update_config(&new_config.world);

                    while agents.len() < new_config.agent.count {
                        let index = u32::try_from(agents.len()).unwrap_or(u32::MAX);
                        agents.push(Agent::new(&new_config, WIDTH, HEIGHT, index));
                    }
                    agents.truncate(new_config.agent.count);
                }
                time_config_update += t_config.elapsed();

                let t_start = Instant::now();
                simulation.swap_buffers();
                time_swap += t_start.elapsed();

                let t_blur = Instant::now();
                simulation.blur();
                time_blur += t_blur.elapsed();

                let t_agents = Instant::now();
                agents
                    .par_iter_mut()
                    .for_each(|agent| agent.update(&simulation));
                time_agents += t_agents.elapsed();

                let t_apply_agents = Instant::now();
                simulation.apply_agents(&agents);
                time_apply_agents += t_apply_agents.elapsed();

                let t_bc = Instant::now();
                simulation.apply_bc();
                time_bc += t_bc.elapsed();

                steps_in_window += 1;
                let elapsed = last_sps_calculation.elapsed();

                // Track & print actual simulation Steps Per Second
                if elapsed.as_secs_f64() >= 1.0 {
                    let sps = f64::from(steps_in_window) / elapsed.as_secs_f64();

                    while sps_samples.len() >= SPS_HISTORY_MAX {
                        _ = sps_samples.pop_front();
                    }
                    sps_samples.push_back(sps);

                    median_buffer.clear();
                    median_buffer.extend(sps_samples.iter().copied());

                    #[expect(clippy::min_ident_chars, reason = "these names are fine")]
                    median_buffer.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Less));

                    let i_mid = median_buffer.len() / 2;
                    #[expect(clippy::indexing_slicing, reason = "checked above")]
                    let sps_average = if median_buffer.len().is_multiple_of(2) {
                        median_buffer[i_mid - 1].midpoint(median_buffer[i_mid])
                    } else {
                        median_buffer[i_mid]
                    };

                    // Calculate profile percentages based on elapsed time window
                    let total_secs = elapsed.as_secs_f64();
                    let pct_grid = (time_grid_update.as_secs_f64() / total_secs) * 100.0;
                    let pct_config = (time_config_update.as_secs_f64() / total_secs) * 100.0;
                    let pct_swap = (time_swap.as_secs_f64() / total_secs) * 100.0;
                    let pct_blur = (time_blur.as_secs_f64() / total_secs) * 100.0;
                    let pct_agents = (time_agents.as_secs_f64() / total_secs) * 100.0;
                    let pct_apply_agents = (time_apply_agents.as_secs_f64() / total_secs) * 100.0;
                    let pct_bc = (time_bc.as_secs_f64() / total_secs) * 100.0;

                    let pct_idle = 100.0
                        - (pct_grid
                            + pct_config
                            + pct_swap
                            + pct_blur
                            + pct_agents
                            + pct_apply_agents
                            + pct_bc);

                    let pct_remainder = pct_swap + pct_bc +  pct_config + pct_idle;

                    println!(
                        "{sps:>6.1} SPS | {sps_average:>6.1} MEDIAN | Blur: {pct_blur:>4.1}% | Sim Agents: {pct_agents:>4.1}% | Apply Agents: {pct_apply_agents:>4.1}% | Copy Grid: {pct_grid:>4.1}% | Remainder: {pct_remainder:>4.1}%"
                    );

                    // Reset accumulators
                    steps_in_window = 0;
                    time_grid_update = Duration::ZERO;
                    time_config_update = Duration::ZERO;
                    time_swap = Duration::ZERO;
                    time_blur = Duration::ZERO;
                    time_agents = Duration::ZERO;
                    time_apply_agents = Duration::ZERO;
                    time_bc = Duration::ZERO;

                    last_sps_calculation += Duration::from_secs(1);
                }
            }
        });

        let mut app = App::<1024>::new(
            render_grid,
            is_running,
            request_update,
            grid_done_rx,
            config_tx,
            config_watcher,
            palette,
        );

        let app_error = event_loop.run_app(&mut app).is_err();

        is_running.store(false, AtomicOrdering::Relaxed);

        let simulation_error = simulation_thread.join().is_err();

        if app_error || simulation_error {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        }
    })
}
