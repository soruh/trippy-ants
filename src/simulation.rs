//! The current run-time state of the entire simulation.

use std::{iter, mem, sync::Mutex};

use rayon::iter::{IntoParallelRefIterator as _, ParallelIterator as _};

use crate::{
    agent::Agent,
    config::WorldConfig,
    grid::{Cell, Grid},
};

/// The current run-time state of the entire simulation.
pub(crate) struct Simulation {
    /// Width of the grid in cells/pixels.
    pub(crate) width: u16,

    /// Height of the grid in cells/pixels.
    pub(crate) height: u16,

    /// The buffer that is read from in the current frame.
    ///
    /// This will be swapped with the write buffer after each frame.
    pub(crate) read_buffer: Grid,

    /// The buffer that is written to in the current frame.
    ///
    /// This will be swapped with the read buffer after each frame.
    pub(crate) write_buffer: Grid,

    /// One lock per cell to allow actors to write to cells in parallel.
    pub(crate) write_locks: Vec<Mutex<()>>,

    /// The value to use for the outermost pixel rows and columns.
    ///
    /// This will repel or attract the ants from the edges of the grid.
    /// A value of `None` means that the edges have no special effect.
    pub(crate) wall_value: Option<f32>,

    /// The decay factor to use for the pheromone levels.
    pub(crate) decay_factor: f32,
}

impl Simulation {
    /// Create a new simulation with the given width and height in cells/pixels.
    pub(crate) fn new(width: u16, height: u16, config: &WorldConfig) -> Self {
        let WorldConfig {
            wall_value,
            topology,
            decay_factor,
        } = *config;

        Self {
            width,
            height,
            read_buffer: Grid::new(width, height, topology),
            write_buffer: Grid::new(width, height, topology),
            write_locks: iter::repeat_with(|| Mutex::new(()))
                .take(width as usize * height as usize)
                .collect(),
            wall_value,
            decay_factor,
        }
    }

    /// Update the simulation by blurring the pheromone levels of the read buffer and writing them to the write buffer.
    pub(crate) fn blur(&mut self) {
        self.write_buffer.blur(&self.read_buffer, self.decay_factor);
    }

    /// Update the simulation by adding the pheromone levels of the agents to the write buffer.
    #[expect(
        clippy::missing_panics_doc,
        reason = "These panics don't happen through invalid input"
    )]
    pub(crate) fn apply_agents(&mut self, agents: &[Agent]) {
        {
            #[expect(
                trivial_casts,
                clippy::ptr_as_ptr,
                clippy::ref_as_ptr,
                reason = "we want to pointer to the cell vec without keeping any temporary references to it"
            )]
            let cell_ptr = self.write_buffer.cells_mut() as *mut [Cell] as *mut Cell as usize;

            agents.par_iter().for_each(|agent| {
                // use the read buffer for index computation. The result will be the same but we don't risk aliasing
                // during the unsafe writes
                let index = self.read_buffer.index(agent.x, agent.y);

                let _guard = self
                    .write_locks
                    .get(index)
                    .expect("invalid agent index")
                    .lock()
                    .expect("Write lock is poisoned");

                #[expect(
                    clippy::multiple_unsafe_ops_per_block,
                    unsafe_code,
                    reason = "doing this without unsafe would probably require a layout change which would pessimise Grid::blur"
                )]
                // Safety: we have exclusive access to this cell, guaranteed by `_guard`
                unsafe {
                    let cell_ptr = cell_ptr as *mut Cell;
                    let level = &mut (*cell_ptr.add(index)).level;
                    *level = (*level + agent.value).clamp(-1.0, 1.0);
                }
            });
        };
    }

    /// Apply the wall value to the outermost pixel rows and columns.
    pub(crate) fn apply_bc(&mut self) {
        // repulse from or attract to walls
        if let Some(value) = self.wall_value {
            // top wall
            self.write_buffer
                .first_row_mut()
                .iter_mut()
                .for_each(|cell| {
                    cell.level = value;
                });

            // bottom wall
            self.write_buffer
                .last_row_mut()
                .iter_mut()
                .for_each(|cell| {
                    cell.level = value;
                });

            // sides
            for row in self.write_buffer.rows_mut() {
                if let Some(first) = row.first_mut() {
                    // left wall
                    first.level = value;
                }
                if let Some(last) = row.last_mut() {
                    // right wall
                    last.level = value;
                }
            }
        }

        // let row = (self.height / 2 - 1) * self.width;
        // for x in 0..self.width {
        //     // Hot coals + noise along the base.
        //     let bump = rand_u32(rng) % 96;
        //     self.cells[row + x].level = (220.0 - bump as f32) / 255.0;
        // }
    }

    /// Swap the read and write buffers.
    ///
    /// This will be called after each frame to prepare for the next frame.
    pub(crate) const fn swap_buffers(&mut self) {
        mem::swap(&mut self.read_buffer, &mut self.write_buffer);
    }

    /// Update the simulation with the new configuration.
    pub(crate) const fn update_config(&mut self, new_config: &WorldConfig) {
        let WorldConfig {
            wall_value,
            topology,
            decay_factor,
        } = *new_config;
        self.wall_value = wall_value;
        self.decay_factor = decay_factor;
        self.read_buffer.topology = topology;
        self.write_buffer.topology = topology;
    }
}
