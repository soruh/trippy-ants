use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};

use crate::{
    Agent,
    config::{Config, GridTopology, WorldConfig},
};

#[derive(Default, Clone, Copy)]
pub(crate) struct Cell {
    pub(crate) level: f32,
}

pub(crate) struct Grid {
    width: usize,
    height: usize,
    pub(crate) cells: Vec<Cell>,
    topology: GridTopology,
}

impl Grid {
    fn new(width: usize, height: usize, topology: GridTopology) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width * height],
            topology,
        }
    }

    fn map_row(&self, y: i32) -> usize {
        let height = self.height as i32;
        match self.topology {
            GridTopology::Torus => {
                if (0..height).contains(&y) {
                    y as usize
                } else {
                    y.rem_euclid(height) as usize
                }
            }
            GridTopology::Plane => y.clamp(0, height - 1) as usize,
        }
    }

    fn map_col(&self, x: i32) -> usize {
        let width = self.width as i32;
        match self.topology {
            GridTopology::Torus => {
                if (0..width).contains(&x) {
                    x as usize
                } else {
                    x.rem_euclid(width) as usize
                }
            }
            GridTopology::Plane => x.clamp(0, width - 1) as usize,
        }
    }

    pub(crate) fn row(&self, y: i32) -> &[Cell] {
        &self.cells[self.map_row(y) * self.width..][..self.width]
    }

    fn row_mut(&mut self, y: i32) -> &mut [Cell] {
        let mapped_row = self.map_row(y);
        &mut self.cells[mapped_row * self.width..][..self.width]
    }

    fn index(&self, x: f32, y: f32) -> usize {
        let x = (x.round() as usize).clamp(0, self.width - 1);
        let y = (y.round() as usize).clamp(0, self.height - 1);
        y * self.width + x
    }

    pub(crate) fn cell(&self, x: f32, y: f32) -> &Cell {
        let index = self.index(x, y);
        &self.cells[index]
    }

    fn cell_mut(&mut self, x: f32, y: f32) -> &mut Cell {
        let index = self.index(x, y);
        &mut self.cells[index]
    }

    fn blur(&mut self, read_buffer: &Grid, decay_factor: f32) {
        self.cells
            .par_chunks_exact_mut(self.width)
            .enumerate()
            .for_each(|(y, write_row)| {
                // 5 rows around the current row
                let y = y as i32;
                let row = [
                    read_buffer.row(y - 1),
                    read_buffer.row(y),
                    read_buffer.row(y + 1),
                ];
                for (x, write_cell) in write_row.iter_mut().enumerate() {
                    // column indices for the 3 columns around x
                    let x = x as i32;
                    let col = [
                        read_buffer.map_col(x - 1),
                        read_buffer.map_col(x),
                        read_buffer.map_col(x + 1),
                    ];

                    // filter kernel (weight sum = 16)
                    // 1 2 1
                    // 2 4 2
                    // 1 2 1

                    let value00 = row[0][col[0]].level; // top left
                    let value01 = row[0][col[1]].level; // top center
                    let value02 = row[0][col[2]].level; // top right
                    let value10 = row[1][col[0]].level; // left center
                    let value11 = row[1][col[1]].level; // center
                    let value12 = row[1][col[2]].level; // right center
                    let value20 = row[2][col[0]].level; // bottom left
                    let value21 = row[2][col[1]].level; // bottom center
                    let value22 = row[2][col[2]].level; // bottom right

                    // sum up smallest values first for improved numerical stability
                    let corners = (value00 + value02 + value20 + value22) * 16.0_f32.recip();
                    let sides = (value01 + value10 + value21 + value12) * 8.0_f32.recip();
                    let center = value11 * 4.0_f32.recip();
                    let sum = corners + sides + center;
                    let level = sum * decay_factor;

                    // avoid sub-normal numbers for performance reasons
                    write_cell.level = if level.is_normal() { level } else { 0.0 };
                }
            });
    }
}

pub(crate) struct Simulation {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) read_buffer: Grid,
    pub(crate) write_buffer: Grid,
    pub(crate) wall_value: Option<f32>,
    pub(crate) decay_factor: f32,
}

impl Simulation {
    pub(crate) fn new(width: usize, height: usize, config: &Config) -> Self {
        let WorldConfig {
            wall_value,
            topology,
            decay_factor,
        } = config.world;

        Self {
            width,
            height,
            read_buffer: Grid::new(width, height, topology),
            write_buffer: Grid::new(width, height, topology),
            wall_value,
            decay_factor,
        }
    }

    pub(crate) fn blur(&mut self) {
        self.write_buffer.blur(&self.read_buffer, self.decay_factor);
    }

    pub(crate) fn update(&mut self, agents: &mut [Agent]) {
        for agent in agents.iter() {
            let level = &mut self.write_buffer.cell_mut(agent.x, agent.y).level;
            *level = (*level + agent.value).clamp(-1.0, 1.0);
        }

        let max_y = self.height - 1;
        let max_x = self.width - 1;

        // repulse from walls
        if let Some(value) = self.wall_value {
            self.write_buffer.row_mut(0).iter_mut().for_each(|cell| {
                cell.level = value;
            });
            self.write_buffer
                .row_mut(max_y as i32)
                .iter_mut()
                .for_each(|cell| {
                    cell.level = value;
                });
            for y in 0..self.height {
                let row_index = y * self.width;
                self.write_buffer.cells[row_index].level = value;
                self.write_buffer.cells[row_index + max_x].level = value;
            }
        }

        // let row = (self.height / 2 - 1) * self.width;
        // for x in 0..self.width {
        //     // Hot coals + noise along the base.
        //     let bump = rand_u32(rng) % 96;
        //     self.cells[row + x].level = (220.0 - bump as f32) / 255.0;
        // }
    }

    pub(crate) fn swap_buffers(&mut self) {
        std::mem::swap(&mut self.read_buffer, &mut self.write_buffer);
    }
}
