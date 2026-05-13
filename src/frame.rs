use std::path::Path;

use minifb::Window;
use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};

use crate::{grid::Grid, palette::Palette};

/// Holds the current colorized image representation of the simulation.
pub(crate) struct Frame {
    /// Width of the frame in pixels.
    width: usize,

    /// Height of the frame in pixels.
    height: usize,

    /// minifb: 0x00RRGGBB per pixel, row-major.
    pub(crate) pixels: Vec<u32>,
}

impl Frame {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0u32; width * height],
        }
    }

    /// Update the frame with the current state of the simulation by colorizing the stored cell-values
    pub(crate) fn update<const RESOLUTION: usize>(
        &mut self,
        grid: &Grid,
        palette: &Palette<RESOLUTION>,
    ) {
        self.pixels
            .par_chunks_exact_mut(self.width)
            .enumerate()
            .for_each(|(y, pixels)| {
                for (pixel, cell) in pixels.iter_mut().zip(grid.row(y as i32)) {
                    *pixel = palette.get_color(cell.level);
                }
            });
    }

    /// Update the window with the current state of the frame.
    pub(crate) fn update_window(&self, window: &mut Window) {
        window
            .update_with_buffer(&self.pixels, self.width, self.height)
            .expect("update");
    }

    /// store the current image as a PNG file.
    pub(crate) fn save_png(&self, path: &Path) -> Result<(), image::ImageError> {
        let rgb = self
            .pixels
            .iter()
            .flat_map(|rgb| {
                let [b, g, r, _] = rgb.to_le_bytes();
                [r, g, b]
            })
            .collect::<Vec<_>>();
        image::save_buffer(
            path,
            &rgb,
            self.width as u32,
            self.height as u32,
            image::ColorType::Rgb8,
        )
    }
}
