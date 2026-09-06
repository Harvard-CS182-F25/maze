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

/// The fixed rate the simulation advances at. The policy is queried on whichever of these ticks
/// its own `policy_hz` lands on, so physics behaves the same however often the policy runs.
pub(crate) const SIMULATION_HZ: f32 = 60.0;

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

impl MazeConfig {
    /// Checks every setting that does not depend on how the config will be run, so that parsing a
    /// config rejects the values that would otherwise panic deep inside the engine.
    pub(crate) fn validate_settings(&self) -> Result<(), String> {
        self.agent.validate()?;
        self.maze_generation.validate()?;

        let (columns, rows) = self.occupancy_grid_dimensions();
        if columns < 1 || rows < 1 {
            return Err(format!(
                "agent.occupancy_grid_cell_size {} is too large for a {}x{} world: it leaves a {}x{} occupancy grid",
                self.agent.occupancy_grid_cell_size,
                self.maze_generation.world_width,
                self.maze_generation.world_height,
                columns,
                rows
            ));
        }

        Ok(())
    }

    pub(crate) fn occupancy_grid_dimensions(&self) -> (i32, i32) {
        (
            (self.maze_generation.world_width / self.agent.occupancy_grid_cell_size).round() as i32,
            (self.maze_generation.world_height / self.agent.occupancy_grid_cell_size).round() as i32,
        )
    }

    /// Everything, including the rules that only a run can settle. Parsing checks
    /// [`Self::validate_settings`] alone, since a caller may still set `teleop` before running.
    pub(crate) fn validate(&self) -> Result<(), String> {
        self.validate_settings()?;
        if self.agent.active_policy_hz().is_none() && (self.headless || !self.teleop) {
            return Err(
                "agent.policy_hz = 0 disables the policy, which only works with windowed teleop"
                    .to_string(),
            );
        }
        Ok(())
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl MazeConfig {
    /// Drive the agent from the keyboard: `Arrows` or `WASD` to move, `Space` to pick up and drop
    /// flags. The policy is still queried, so a mapping agent keeps building its occupancy grid
    /// while you drive, but its actions are ignored. To skip the policy entirely — a teleop
    /// demonstration with an agent that is not written yet — set `agent.policy_hz` to zero in the
    /// config file.
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

    #[test]
    fn settings_that_would_panic_the_engine_are_rejected() {
        let ok = MazeConfig::default();
        assert!(ok.validate_settings().is_ok());

        // A motionless agent and a noiseless sensor are degenerate, not invalid.
        let mut degenerate = MazeConfig::default();
        degenerate.agent.max_speed = 0.0;
        degenerate.agent.position_stddev = 0.0;
        assert!(degenerate.validate_settings().is_ok());

        let cases: [(&str, fn(&mut MazeConfig)); 7] = [
            ("negative max_speed", |c| c.agent.max_speed = -1.0),
            ("negative stddev", |c| c.agent.position_stddev = -1.0),
            ("zero grid cell", |c| c.agent.occupancy_grid_cell_size = 0.0),
            ("negative grid cell", |c| c.agent.occupancy_grid_cell_size = -1.0),
            ("zero world", |c| c.maze_generation.world_width = 0.0),
            ("zero maze cell", |c| c.maze_generation.cell_size = 0.0),
            ("maze cell larger than the world", |c| {
                c.maze_generation.cell_size = 1000.0
            }),
        ];

        for (name, break_it) in cases {
            let mut config = MazeConfig::default();
            break_it(&mut config);
            assert!(config.validate_settings().is_err(), "{name} was accepted");
        }
    }

    #[test]
    fn disabled_policy_requires_graphical_teleop() {
        let mut config = MazeConfig::default();
        config.agent.policy_hz = 0.0;
        config.teleop = true;
        assert!(config.validate().is_ok());

        config.teleop = false;
        assert!(config.validate().is_err());

        config.teleop = true;
        config.headless = true;
        assert!(config.validate().is_err());
    }
}
