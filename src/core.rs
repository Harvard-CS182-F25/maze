use bevy::prelude::*;
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use serde::{Deserialize, Serialize};

use crate::agent;
use crate::camera;
use crate::character_controller;
use crate::flag;
use crate::interaction_range;
use crate::occupancy_grid;
use crate::playback;
use crate::scene;
use crate::teleop;

#[gen_stub_pyclass]
#[pyclass(name = "MazeConfig")]
#[derive(Debug, Clone, Resource, Reflect, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
// Reject unknown config fields so typos fail fast.
#[serde(default, deny_unknown_fields)]
#[reflect(Resource)]
pub struct MazeConfig {
    #[pyo3(get, set)]
    pub agent: agent::AgentConfig,
    #[pyo3(get, set)]
    pub flags: flag::FlagConfig,
    #[pyo3(get, set)]
    pub maze_generation: scene::MazeGenerationConfig,
    #[pyo3(get, set)]
    pub use_true_map: bool,
    pub teleop: bool,
    #[pyo3(get, set)]
    pub debug: bool,
    #[pyo3(get, set)]
    pub headless: bool,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StartupSets {
    Walls,
    FlagsAndCapturePoints,
    Agents,
}

/// Ordered stages of one fixed simulation tick.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimulationSets {
    Policy,
    Controller,
    Interactions,
    Capture,
    TrueGrid,
    Metrics,
}

#[gen_stub_pymethods]
#[pymethods]
impl MazeConfig {
    /// When true the agent is driven by the keyboard: `Arrows` or `WASD` to move, `Space` to pick
    /// up and drop flags. The Python policy still runs every tick so a mapping agent keeps building
    /// its occupancy grid while you drive, but its actions are ignored.
    #[getter]
    fn teleop(&self) -> bool {
        self.teleop
    }

    #[setter]
    fn set_teleop(&mut self, value: bool) {
        self.teleop = value;
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("MazeConfig({})", self.__str__()?))
    }

    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to serialize MazeConfig: {}",
                e
            ))
        })
    }
}

pub struct MazePlugin {
    pub config: MazeConfig,
}

impl Plugin for MazePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());

        app.add_plugins((
            camera::CameraPlugin,
            character_controller::CharacterControllerPlugin,
            agent::AgentPlugin,
            flag::FlagPlugin,
            interaction_range::InteractionRangePlugin,
            scene::ScenePlugin,
            playback::PlaybackPlugin,
            teleop::TeleopPlugin,
            occupancy_grid::OccupancyGridPlugin {
                config: self.config.clone(),
            },
        ));

        app.configure_sets(
            Startup,
            (
                StartupSets::Walls,
                StartupSets::FlagsAndCapturePoints,
                StartupSets::Agents,
            )
                .chain(),
        );
        app.configure_sets(
            FixedUpdate,
            (
                SimulationSets::Policy,
                SimulationSets::Controller,
                SimulationSets::Interactions,
            )
                .chain(),
        );
        app.configure_sets(
            FixedPostUpdate,
            (
                SimulationSets::Capture,
                SimulationSets::TrueGrid,
                SimulationSets::Metrics,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::MazeConfig;

    #[test]
    fn config_schema_groups_flag_settings_and_rejects_removed_fields() {
        let config: MazeConfig = serde_yaml::from_str(
            "flags:\n  flag_count: 2\n  capture_point_count: 3\nmaze_generation:\n  world_width: 80\n  world_height: 60\n",
        )
        .unwrap();

        assert_eq!(config.flags.flag_count, 2);
        assert_eq!(config.flags.capture_point_count, 3);
        assert_eq!(config.maze_generation.world_width, 80.0);
        assert_eq!(config.maze_generation.world_height, 60.0);
        assert!(serde_yaml::from_str::<MazeConfig>("camera:\n  scale: -0.15\n").is_err());
        assert!(serde_yaml::from_str::<MazeConfig>("capture_points:\n  number: 1\n").is_err());
        assert!(serde_yaml::from_str::<MazeConfig>("maze_generation:\n  width: 100\n").is_err());
    }
}
