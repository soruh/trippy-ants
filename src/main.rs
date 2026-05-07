#![warn(clippy::all, clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use chrono::Local;
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use std::{
    f32::consts::{PI, TAU},
    path::Path,
    time::{Duration, Instant},
};

const ENABLE_WALLS: bool = false;

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;
const SENSOR_WIDTH: f32 = 0.6;
const SENSOR_DISTANCE: f32 = 20.0;
const AGENT_COUNT: usize = 10_000;
const LIMIT: f32 = 1.0;

#[derive(Default, Clone)]
struct Cell {
    level: f32,
}

struct Grid {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Grid {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width * height],
        }
    }

    fn row(&self, y: usize) -> &[Cell] {
        &self.cells[y * self.width..][..self.width]
    }

    fn index(&self, x: f32, y: f32) -> usize {
        let x = (x.round() as usize).clamp(0, self.width - 1);
        let y = (y.round() as usize).clamp(0, self.height - 1);
        y * self.width + x
    }

    fn cell(&self, x: f32, y: f32) -> &Cell {
        let index = self.index(x, y);
        &self.cells[index]
    }
    fn cell_mut(&mut self, x: f32, y: f32) -> &mut Cell {
        let index = self.index(x, y);
        &mut self.cells[index]
    }

    fn update(&mut self, read: &Self, agents: &mut [Agent], rng: &mut u32) {
        let max_y = self.height - 1;
        let max_x = self.width - 1;

        self.cells
            .par_chunks_exact_mut(self.width)
            .enumerate()
            .for_each(|(y, write_row)| {
                // 5 rows around the current row
                let row = [
                    read.row(y.saturating_sub(2)),
                    read.row(y.saturating_sub(1)),
                    read.row(y),
                    read.row((y + 1).min(max_y)),
                    read.row((y + 2).min(max_y)),
                ];
                for (x, write_cell) in write_row.iter_mut().enumerate() {
                    // column indices for the 5 columns around x
                    let col = [
                        x.saturating_sub(2),
                        x.saturating_sub(1),
                        x,
                        (x + 1).min(max_x),
                        (x + 2).min(max_x),
                    ];

                    let mut sum = 0.0;
                    // sum += row[0][col[1]].level;
                    // sum += row[0][col[2]].level;
                    // sum += row[0][col[3]].level;

                    // sum += row[1][col[0]].level;
                    sum += row[1][col[1]].level * 1.0;
                    sum += row[1][col[2]].level * 2.0;
                    sum += row[1][col[3]].level * 1.0;
                    // sum += row[1][col[4]].level;

                    // sum += row[2][col[0]].level;
                    sum += row[2][col[1]].level * 2.0;
                    sum += row[2][col[2]].level * 4.0;
                    sum += row[2][col[3]].level * 2.0;
                    // sum += row[2][col[4]].level;

                    // sum += row[3][col[0]].level;
                    sum += row[3][col[1]].level * 1.0;
                    sum += row[3][col[2]].level * 2.0;
                    sum += row[3][col[3]].level * 1.0;
                    // sum += row[3][col[4]].level;

                    // sum += row[4][col[1]].level;
                    // sum += row[4][col[2]].level;
                    // sum += row[4][col[3]].level;

                    let level = (sum / 16.0) * 0.99;

                    write_cell.level = if level.is_normal() { level } else { 0.0 };
                }
            });

        for agent in agents.iter_mut() {
            let sniff = |angle: f32| {
                let (sin, cos) = angle.sin_cos();
                read.cell(
                    agent.x + cos * SENSOR_DISTANCE,
                    agent.y + sin * SENSOR_DISTANCE,
                )
                .level
            };

            let sniff_right = sniff(agent.direction + SENSOR_WIDTH);
            let sniff_left = sniff(agent.direction - SENSOR_WIDTH);
            let sniff_right2 = sniff(agent.direction + SENSOR_WIDTH * 2.0);
            let sniff_left2 = sniff(agent.direction - SENSOR_WIDTH * 2.0);

            let distract = (rand_f32(rng) - 0.5) * 0.5;

            let intensity = sniff_right2 + sniff_left2 + sniff_right + sniff_left;
            if intensity > 0.0 {
                let delta = (sniff_right2 * 0.2 * SENSOR_WIDTH
                    + sniff_left2 * -2.0 * SENSOR_WIDTH
                    + sniff_right * SENSOR_WIDTH
                    + sniff_left * -SENSOR_WIDTH
                    + distract)
                    / intensity;
                agent.direction += delta * 0.1;
            }
        }

        for agent in agents.iter_mut() {
            agent.update(self.width, self.height, rng);
        }

        for agent in agents.iter() {
            let level = &mut self.cell_mut(agent.x, agent.y).level;
            *level = (*level + agent.value).min(LIMIT);
        }

        // for angle in 0..1000 {
        //     if (angle / 125) % 2 == 0 {
        //         continue;
        //     }
        //     let a = angle as f32 / 1000.0;
        //     let angle = a * TAU;
        //     let (sin, cos) = angle.sin_cos();
        //     let r = self.height as f32 * 0.25; // - angle * 10.0;
        //     let level = &mut self
        //         .cell_mut(
        //             self.width as f32 * 0.5 + cos * r,
        //             self.height as f32 * 0.5 + sin * r,
        //         )
        //         .level;
        //     *level = 1.0; //level.max(1.0 - a);
        // }

        // let mut draw_line = |x1: f32, y1: f32, x2: f32, y2: f32| {
        //     let (dx, dy) = (x2 - x1, y2 - y1);
        //     let steps = (dx.abs().max(dy.abs()) as usize).max(1);
        //     for i in 0..steps {
        //         let x = x1 + dx * i as f32 / steps as f32;
        //         let y = y1 + dy * i as f32 / steps as f32;
        //         self.cell_mut(x, y).level = 1.0;
        //     }
        // };

        // let w = WIDTH as f32;
        // let h = HEIGHT as f32;

        // draw_line(w * 0.5, h * 0.2, w * 0.7, h * 0.8);
        // draw_line(w * 0.5, h * 0.2, w * 0.3, h * 0.8);
        // draw_line(w * 0.3, h * 0.8, w * 0.7, h * 0.8);

        // repulse from walls
        if ENABLE_WALLS {
            let value = 0.0;
            self.cells[0..self.width].iter_mut().for_each(|cell| {
                cell.level = value;
            });
            self.cells[self.width * (self.height - 1)..self.width * self.height]
                .iter_mut()
                .for_each(|cell| {
                    cell.level = value;
                });
            for y in 0..self.height {
                let row_index = y * self.width;
                self.cells[row_index].level = value;
                self.cells[row_index + self.width - 1].level = value;
            }
        }

        // let row = (self.height / 2 - 1) * self.width;
        // for x in 0..self.width {
        //     // Hot coals + noise along the base.
        //     let bump = rand_u32(rng) % 96;
        //     self.cells[row + x].level = (220.0 - bump as f32) / 255.0;
        // }
    }
}

/// [`Frame::pixels`] / minifb: 0x00RRGGBB per pixel, row-major.
fn save_png(
    pixels: &[u32],
    width: usize,
    height: usize,
    path: &Path,
) -> Result<(), image::ImageError> {
    let mut rgb = Vec::with_capacity(width * height * 3);
    for px in pixels {
        rgb.push(((px >> 16) & 0xFF) as u8);
        rgb.push(((px >> 8) & 0xFF) as u8);
        rgb.push((px & 0xFF) as u8);
    }
    image::save_buffer(
        path,
        &rgb,
        width as u32,
        height as u32,
        image::ColorType::Rgb8,
    )
}

fn main() {
    let palette = Palette::new();
    let mut rng = 0xfeed_face_u32;

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

    let mut read_buffer = Grid::new(WIDTH, HEIGHT);
    let mut write_buffer = Grid::new(WIDTH, HEIGHT);
    let mut frame = Frame::new(WIDTH, HEIGHT, palette);
    let mut agents = (0..AGENT_COUNT)
        .map(|_| {
            let direction = rand_f32(&mut rng) * std::f32::consts::TAU;
            let r = HEIGHT as f32 * 0.0;
            let (sin, cos) = (direction + PI).sin_cos();
            let x = WIDTH as f32 * 0.5 + cos * r;
            let y = HEIGHT as f32 * 0.5 + sin * r;

            Agent::new(
                // x,
                // y,
                WIDTH as f32 * 0.5,
                HEIGHT as f32 * 0.5,
                // rand_f32(&mut rng) * WIDTH as f32,
                // rand_f32(&mut rng) * HEIGHT as f32,
                direction,
                rand_f32(&mut rng) * 0.7 + 0.3,
                rand_f32(&mut rng) * 0.9 + 1.0,
            )
        })
        .collect::<Vec<_>>();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        write_buffer.update(&read_buffer, &mut agents, &mut rng);

        std::mem::swap(&mut read_buffer, &mut write_buffer);
        frame.update(&read_buffer.cells);
        window
            .update_with_buffer(&frame.pixels, WIDTH, HEIGHT)
            .expect("update");

        if window.is_key_pressed(Key::Space, KeyRepeat::No) {
            let filename = format!(
                "trippy-ants_{}.png",
                Local::now().format("%Y-%m-%d_%H-%M-%S")
            );
            match save_png(&frame.pixels, WIDTH, HEIGHT, Path::new(&filename)) {
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

/// Mulberry32 — deterministic, fast, no extra crates.
fn rand_u32(state: &mut u32) -> u32 {
    let mut z = *state;
    z = z.wrapping_add(0x6d2b_79f5);
    let mut t = z;
    t = t ^ (t >> 15);
    t = t.wrapping_mul(z | 1);
    t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 0x3d));
    *state = z;
    t ^ (t >> 14)
}

fn rand_f64(state: &mut u32) -> f64 {
    rand_u32(state) as f64 / 0x1_0000_0000_u64 as f64
}

fn rand_f32(state: &mut u32) -> f32 {
    rand_f64(state) as f32
}

struct Frame {
    _width: usize,
    _height: usize,
    pixels: Vec<u32>,
    palette: Palette,
}

impl Frame {
    fn new(width: usize, height: usize, palette: Palette) -> Self {
        Self {
            _width: width,
            _height: height,
            pixels: vec![0u32; width * height],
            palette,
        }
    }

    fn update(&mut self, cells: &[Cell]) {
        for (pixel, cell) in self.pixels.iter_mut().zip(cells.iter()) {
            *pixel = self.palette.get_color(cell.level);
        }
    }
}

struct Palette {
    colors: [u32; 256],
}

impl Palette {
    fn new() -> Self {
        Self {
            colors: Self::build_palette(),
        }
    }

    fn get_color(&self, level: f32) -> u32 {
        self.colors[((level.abs().sqrt() / LIMIT * 256.0) as usize).clamp(0, 255)]
    }

    /// Saturated red → yellow → white, black at index 0 (90s demo look).
    fn build_palette() -> [u32; 256] {
        let red_curve = Curve::new(0.5, 0.5);
        let green_curve = Curve::new(0.5, 0.5);
        let blue_curve = Curve::new(0.5, 0.5);
        let mut result = [0; 256];
        for (index, color) in result.iter_mut().enumerate() {
            let t = index as f64 / 255.0;
            let red = red_curve.get_value(t as f32);
            let green = green_curve.get_value(t as f32);
            let blue = blue_curve.get_value(t as f32);

            let red = (red * 256.0).clamp(0.0, 255.0) as u32;
            let green = (green * 256.0).clamp(0.0, 255.0) as u32;
            let blue = (blue * 256.0).clamp(0.0, 255.0) as u32;
            let red = ((t + 0.0).powf(1.5) * 256.0).clamp(0.0, 255.0) as u32;
            let green = ((t + 0.0).powf(1.3) * 256.0).clamp(0.0, 255.0) as u32;
            let blue = ((t + 0.0).powf(1.0) * 256.0).clamp(0.0, 255.0) as u32;
            // let red = (255.0 * t.powf(0.85)).min(255.0) as u32;
            // let green = (255.0f64 * (t - 0.15).max(0.0) / 0.85).powf(1.1).min(255.0) as u32;
            // let blue = (255.0f64 * (t - 0.45).max(0.0) / 0.55)
            //     .powf(1.25)
            //     .min(255.0) as u32;
            *color = (red << 16) | (green << 8) | blue;
        }
        result
    }
}

struct Curve {
    a: f32,
    b: f32,
    c: f32,
}

impl Curve {
    fn new(x: f32, y: f32) -> Self {
        // compute the three coefficients for a parabola through the points (0, 0), (x, y), (1, 1)
        let a = 2.0 * y / (x * (x - 1.0));
        let b = -2.0 * y / (x - 1.0);
        let c = y;
        Self { a, b, c }
    }

    fn get_value(&self, t: f32) -> f32 {
        self.a * t * t + self.b * t + self.c
    }
}

struct Agent {
    x: f32,
    y: f32,
    direction: f32,
    speed: f32,
    value: f32,
}

impl Agent {
    fn new(x: f32, y: f32, direction: f32, speed: f32, value: f32) -> Self {
        Self {
            x,
            y,
            direction,
            speed,
            value,
        }
    }

    fn update(&mut self, width: usize, height: usize, rng: &mut u32) {
        let (sin, cos) = self.direction.sin_cos();

        let mut new_x = self.x + cos * self.speed;
        let mut new_y = self.y + sin * self.speed;

        // while new_x > width as f32 {
        //     new_x -= width as f32;
        // }
        // while new_x < 0.0 {
        //     new_x += width as f32;
        // }
        // while new_y > height as f32 {
        //     new_y -= height as f32;
        // }
        // while new_y < 0.0 {
        //     new_y += height as f32;
        // }
        // self.x = new_x;
        // self.y = new_y;
        if new_x < 0.0 {
            self.direction = rand_f32(rng) * TAU;
            self.x = 0.0;
        } else if new_x > width as f32 {
            self.direction = rand_f32(rng) * TAU;
            self.x = width as f32;
        } else {
            self.x = new_x;
        }

        if new_y < 0.0 {
            self.direction = rand_f32(rng) * TAU;
            self.y = 0.0;
        } else if new_y > height as f32 {
            self.direction = rand_f32(rng) * TAU;
            self.y = height as f32;
        } else {
            self.y = new_y;
        }
    }
}
