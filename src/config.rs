use std::ops::Range;

pub(crate) const DEFAULT_CONFIG: Config = CONFIG;

const CONFIG: Config = Config {
    world: WorldConfig {
        wall_value: None,
        topology: GridTopology::Plane,
        decay_factor: 0.99,
    },
    agent: AgentConfig {
        count: 20_000,
        value: 0.3..1.2,
        speed: 1.0..1.7,
        sensor_width: 0.6,
        sensor_distance: 20.0,
        anti_percentage: 0.0,
        anti_speed_factor: 0.5,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(unused, reason = "not all configuration options are used all the time")]
pub(crate) enum GridTopology {
    /// The grid is a torus. Data from one edge will blur into the opposite edge.
    ///
    /// Use this if you want to stitch the screenshot together to form a seamless image.
    Torus,
    /// The grid is a plane. Data will not blur into the opposite edge but will be clipped.
    Plane,
}

pub(crate) struct Config {
    pub(crate) world: WorldConfig,
    pub(crate) agent: AgentConfig,
}

pub(crate) struct AgentConfig {
    // Number of ants
    pub(crate) count: usize,
    pub(crate) value: Range<f32>,
    pub(crate) speed: Range<f32>,
    pub(crate) sensor_width: f32,
    pub(crate) sensor_distance: f32,

    // Number of ants that will be spawned as anti-ants.
    ///
    /// - 0.0 means that no anti-ants will be spawned.
    /// - 1.0 means that all ants will be spawned as anti-ants.
    /// - 0.5 means that half of the ants will be spawned as anti-ants.
    pub(crate) anti_percentage: f32,

    /// Factor by which the speed of anti-ants will be reduced.
    ///
    /// - 0.0 means that anti-ants won't move.
    /// - 1.0 means that anti-ants will move at the same speed as normal ants.
    /// - 0.5 means that anti-ants will move at half the speed of normal ants.
    pub(crate) anti_speed_factor: f32,
}

pub struct WorldConfig {
    /// Constant value for the outermost pixel rows and columns.
    ///
    /// Can be used to repel or attract the ants.
    pub(crate) wall_value: Option<f32>,

    /// Determines how the edge of the frame will be handled.
    pub(crate) topology: GridTopology,

    /// Determines how much of the pheromones will remain after each frame.
    ///
    /// - 0.999 means that 99.9% of the pheromones will remain after each frame.
    /// - 0.99 means that 99% of the pheromones will remain after each frame.
    pub(crate) decay_factor: f32,
}
