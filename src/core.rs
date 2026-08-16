use bevy::prelude::*;
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
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
// `default` fills in anything the file omits; `deny_unknown_fields` makes anything the file
// invents an error instead of a silent no-op. Without the latter a stale or mistyped key just
// disappears, and the game runs with settings the config claims it isn't using.
#[serde(default, deny_unknown_fields)]
#[reflect(Resource)]
pub struct MazeConfig {
    #[pyo3(get, set)]
    pub agent: agent::AgentConfig,
    #[pyo3(get, set)]
    pub flags: flag::FlagConfig,
    #[pyo3(get, set)]
    pub capture_points: flag::CapturePointConfig,
    #[pyo3(get, set)]
    pub camera: camera::CameraConfig,
    #[pyo3(get, set)]
    pub maze_generation: scene::MazeGenerationConfig,
    #[pyo3(get, set)]
    pub debug: bool,
    #[pyo3(get, set)]
    pub headless: bool,
    #[pyo3(get, set)]
    pub use_true_map: bool,

    /// When true the agent is driven by the keyboard: `WASD` to move, `Space` to pick up and drop
    /// flags. The Python policy still runs every tick — so a mapping agent keeps building its
    /// occupancy grid while you drive — but the `Action::Move` it returns is ignored.
    #[pyo3(get, set)]
    pub teleop: bool,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StartupSets {
    Walls,
    FlagsAndCapturePoints,
    Agents,
}

#[pymethods]
impl MazeConfig {
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
    }
}
