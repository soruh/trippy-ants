pub(crate) struct Palette<const RESOLUTION: usize> {
    colors: [u32; RESOLUTION],
    anti_colors: [u32; RESOLUTION],
}

impl<const RESOLUTION: usize> Palette<RESOLUTION> {
    pub(crate) fn new() -> Self {
        let colors = Self::build_palette(0.8, 3.0, 2.0);
        let anti_colors = Self::build_palette(2.0, 1.6, 1.0);
        Self {
            colors,
            anti_colors,
        }
    }

    pub(crate) fn get_color(&self, level: f32) -> u32 {
        if level >= 0.0 {
            let index = (level * self.colors.len() as f32) as usize;
            self.colors
                .get(index)
                .copied()
                .unwrap_or(self.colors.last().copied().unwrap())
        } else {
            let index = (-level * self.anti_colors.len() as f32) as usize;
            self.anti_colors
                .get(index)
                .copied()
                .unwrap_or(self.anti_colors.last().copied().unwrap())
        }
    }

    fn build_palette(r_exp: f64, g_exp: f64, b_exp: f64) -> [u32; RESOLUTION] {
        let mut result = [0; RESOLUTION];
        let index_scale = ((result.len() - 1) as f64).recip();
        for (index, color) in result.iter_mut().enumerate() {
            // map index to 0.0..=1.0
            let t = index as f64 * index_scale;

            // color curve bending
            let red = t.powf(r_exp);
            let green = t.powf(g_exp);
            let blue = t.powf(b_exp);

            // map and clamp colors to 0..=255
            let red = (red * 256.0).clamp(0.0, 255.0) as u32;
            let green = (green * 256.0).clamp(0.0, 255.0) as u32;
            let blue = (blue * 256.0).clamp(0.0, 255.0) as u32;

            // combine colors into a single u32 (0x00RRGGBB)
            *color = (red << 16) | (green << 8) | blue;
        }
        result
    }
}
