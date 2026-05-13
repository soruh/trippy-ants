use std::f32::consts::{PI, TAU};

use crate::{
    config::{AgentConfig, Config},
    grid::Simulation,
    random::{rand_f32, rand_u32},
};

pub(crate) struct Agent {
    pub(crate) x: f32,
    pub(crate) y: f32,
    direction: f32,
    speed: f32,
    pub(crate) value: f32,
    rng: u32,
    sensor_width: f32,
    sensor_distance: f32,
    anti_speed_factor: f32,
}

impl Agent {
    pub(crate) fn new(config: &Config, width: usize, height: usize, rng: &mut u32) -> Self {
        let AgentConfig {
            count: _,
            ref value,
            ref speed,
            sensor_width,
            sensor_distance,
            anti_percentage,
            anti_speed_factor,
        } = config.agent;

        // compute an individual seed
        let mut rng = rand_u32(rng) ^ rand_u32(rng);
        let mut random = || rand_f32(&mut rng);

        let x = random() * width as f32;
        let y = random() * height as f32;
        let l = f32::hypot(x - width as f32 * 0.5, y - height as f32 * 0.5);
        let r = height as f32 * 0.1 / l;
        let x = (x - width as f32 * 0.5) * r + width as f32 * 0.5;
        let y = (y - height as f32 * 0.5) * r + height as f32 * 0.5;
        // let (sin, cos) = (direction + PI).sin_cos();
        // let x = width as f32 * 0.5 + cos * r;
        // let y = height as f32 * 0.5 + sin * r;

        // let x = width as f32 * 0.5;
        // let y = height as f32 * 0.5;

        // let direction = random() * TAU;
        let direction = f32::atan2(y - height as f32 * 0.5, x - width as f32 * 0.5) - PI * 0.5;
        let direction = if direction.is_nan() { 0.0 } else { direction };
        // let direction = PI / 2.0;

        let speed = speed.start + random() * (speed.end - speed.start);
        let value = value.start + random() * (value.end - value.start);
        let sign = if random() > anti_percentage {
            1.0
        } else {
            -1.0
        };

        Self {
            x,
            y,
            direction,
            speed,
            value: value * sign,
            rng,
            sensor_width,
            sensor_distance,
            anti_speed_factor,
        }
    }

    pub(crate) fn update(&mut self, simulation: &Simulation) {
        self.update_direction(simulation);
        self.update_position(simulation);
    }

    fn update_position(&mut self, simulation: &Simulation) {
        let mut random = || rand_f32(&mut self.rng);
        let (sin, cos) = self.direction.sin_cos();

        let scale = if self.value > 0.0 {
            1.0
        } else {
            self.anti_speed_factor
        };

        let new_x = self.x + cos * self.speed * scale;
        let new_y = self.y + sin * self.speed * scale;

        let width = simulation.width as f32;
        let height = simulation.height as f32;

        // while new_x >= width as f32 {
        //     new_x -= width as f32;
        // }
        // while new_x < 0.0 {
        //     new_x += width as f32;
        // }
        // while new_y >= height as f32 {
        //     new_y -= height as f32;
        // }
        // while new_y < 0.0 {
        //     new_y += height as f32;
        // }
        // self.x = new_x;
        // self.y = new_y;

        if new_x < 0.0 {
            self.x = random() * width;
            self.y = random() * width;
            self.direction = random() * TAU;
            // self.value = -self.value;
            // self.x = 0.0;
        } else if new_x > width {
            self.x = random() * width;
            self.y = random() * width;
            self.direction = random() * TAU;
            // self.value = -self.value;
            // self.x = width;
        } else {
            self.x = new_x;
        }

        if new_y < 0.0 {
            self.x = random() * width;
            self.y = random() * width;
            self.direction = random() * TAU;
            // self.value = -self.value;
            // self.y = 0.0;
        } else if new_y > height {
            self.x = random() * width;
            self.y = random() * width;
            self.direction = random() * TAU;
            // self.value = -self.value;
            // self.y = height;
        } else {
            self.y = new_y;
        }

        // if new_x < 0.0 {
        //     self.direction = 0.0 * TAU; // random() * TAU;
        //     self.x = 0.0;
        // } else if new_x > width {
        //     self.direction = 0.5 * TAU; // random() * TAU;
        //     self.x = width;
        // } else {
        //     self.x = new_x;
        // }

        // if new_y < 0.0 {
        //     self.direction = 0.25 * TAU; // random() * TAU;
        //     self.y = 0.0;
        // } else if new_y > height {
        //     self.direction = 0.75 * TAU; // random() * TAU;
        //     self.y = height;
        // } else {
        //     self.y = new_y;
        // }
    }

    fn update_direction(&mut self, simulation: &Simulation) {
        let mut random = || rand_f32(&mut self.rng);

        let sniff = |angle: f32| {
            let (sin, cos) = angle.sin_cos();
            simulation
                .read_buffer
                .cell(
                    self.x + cos * self.sensor_distance,
                    self.y + sin * self.sensor_distance,
                )
                .level
        };

        #[expect(clippy::neg_multiply, reason = "improves readability")]
        let sniffs = [
            self.sensor_width * -1.0,
            self.sensor_width * -0.5,
            self.sensor_width * 0.5,
            self.sensor_width * 1.0,
        ];

        let mut angle_sum = 0.0;
        for angle in sniffs {
            angle_sum += sniff(self.direction + angle) * angle;
        }

        let delta = angle_sum; // / weight_sum;
        self.direction += delta * 0.5 + (random() * 2.0 - 1.0) * self.sensor_width * 0.3;

        //     let sniff_right = sniff(self.direction + self.sensor_width);
        //     let sniff_left = sniff(self.direction - self.sensor_width);
        //     let sniff_right2 = sniff(self.direction + self.sensor_width * 2.0);
        //     let sniff_left2 = sniff(self.direction - self.sensor_width * 2.0);

        //     let distract_weight = 0.4;
        //     let distract = (random() * 2.0 - 1.0) * self.sensor_width * distract_weight;

        //     let intensity = sniff_right2 + sniff_left2 + sniff_right + sniff_left + distract_weight;
        //     if intensity > 0.0 {
        //         let delta = (sniff_right2 * 0.2 * self.sensor_width
        //             + sniff_left2 * -2.0 * self.sensor_width
        //             + sniff_right * self.sensor_width
        //             + sniff_left * -self.sensor_width
        //             + distract)
        //             / intensity;
        //         self.direction += delta * 0.1;
        //     }
    }
}
