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

/// Everything `parse_config` read out of a YAML file.
#[gen_stub_pyclass]
#[pyclass(name = "MazeConfig")]
#[derive(Debug, Clone, Resource, Reflect, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
// Reject unknown config fields so typos fail fast.
#[serde(default, deny_unknown_fields)]
#[reflect(Resource)]
pub struct MazeConfig {
    // Read-only from Python. PyO3 hands back a clone of a nested `#[pyclass]` field rather than a
    // reference into the parent, so a setter here would accept writes that never arrive. Every
    // leaf of the three is reachable directly on `MazeConfig` instead.
    #[pyo3(get)]
    pub agent: agent::AgentConfig,
    #[pyo3(get)]
    pub flags: flag::FlagConfig,
    #[pyo3(get)]
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
        self.flags.validate()?;
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
            (self.maze_generation.world_height / self.agent.occupancy_grid_cell_size).round()
                as i32,
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

    #[getter(name)]
    fn get_name(&self) -> String {
        self.agent.name.clone()
    }
    #[setter(name)]
    fn set_name(&mut self, value: String) {
        self.agent.name = value;
    }

    #[getter(max_speed)]
    fn get_max_speed(&self) -> f32 {
        self.agent.max_speed
    }
    #[setter(max_speed)]
    fn set_max_speed(&mut self, value: f32) {
        self.agent.max_speed = value;
    }

    /// How often `get_action` is called, in Hz. Zero disables the policy entirely, which is only
    /// useful for a keyboard-only teleop demonstration.
    ///
    /// The policy runs on simulation ticks, which are fixed at 60 Hz, so a rate that does not
    /// divide 60 cannot be hit exactly: it is correct on average, but the `dt` handed to
    /// `get_action` alternates between neighbouring tick counts. At 7 Hz, for instance, `dt`
    /// alternates between 0.133 and 0.150 rather than sitting at 1/7. A rate that divides 60
    /// gives a constant `dt`.
    #[getter(policy_hz)]
    fn get_policy_hz(&self) -> f32 {
        self.agent.policy_hz
    }
    #[setter(policy_hz)]
    fn set_policy_hz(&mut self, value: f32) {
        self.agent.policy_hz = value;
    }

    #[getter(position_stddev)]
    fn get_position_stddev(&self) -> f32 {
        self.agent.position_stddev
    }
    #[setter(position_stddev)]
    fn set_position_stddev(&mut self, value: f32) {
        self.agent.position_stddev = value;
    }

    #[getter(range_stddev)]
    fn get_range_stddev(&self) -> f32 {
        self.agent.range_stddev
    }
    #[setter(range_stddev)]
    fn set_range_stddev(&mut self, value: f32) {
        self.agent.range_stddev = value;
    }

    /// How many rays the agent casts, spread evenly over a full turn. All of them are cast on the
    /// tick the policy is queried, so the cost of a tick grows with this. Zero leaves the agent
    /// with no range sensor at all.
    #[getter(raycast_count)]
    fn get_raycast_count(&self) -> u32 {
        self.agent.raycast_count
    }
    #[setter(raycast_count)]
    fn set_raycast_count(&mut self, value: u32) {
        self.agent.raycast_count = value;
    }

    /// How far each ray reaches. A ray that hits nothing within this distance reports the distance
    /// itself, so a reading equal to it means "nothing found", not "a wall exactly here".
    #[getter(raycast_max_distance)]
    fn get_raycast_max_distance(&self) -> f32 {
        self.agent.raycast_max_distance
    }
    #[setter(raycast_max_distance)]
    fn set_raycast_max_distance(&mut self, value: f32) {
        self.agent.raycast_max_distance = value;
    }

    #[getter(occupancy_grid_cell_size)]
    fn get_occupancy_grid_cell_size(&self) -> f32 {
        self.agent.occupancy_grid_cell_size
    }
    #[setter(occupancy_grid_cell_size)]
    fn set_occupancy_grid_cell_size(&mut self, value: f32) {
        self.agent.occupancy_grid_cell_size = value;
    }

    /// How long the simulation waits for one `get_action` call before giving up on the policy, in
    /// seconds. Zero waits forever, which is what a debugger session needs.
    #[getter(policy_timeout_seconds)]
    fn get_policy_timeout_seconds(&self) -> f32 {
        self.agent.policy_timeout_seconds
    }
    #[setter(policy_timeout_seconds)]
    fn set_policy_timeout_seconds(&mut self, value: f32) {
        self.agent.policy_timeout_seconds = value;
    }

    #[getter(flag_count)]
    fn get_flag_count(&self) -> usize {
        self.flags.flag_count
    }
    #[setter(flag_count)]
    fn set_flag_count(&mut self, value: usize) {
        self.flags.flag_count = value;
    }

    #[getter(capture_point_count)]
    fn get_capture_point_count(&self) -> usize {
        self.flags.capture_point_count
    }
    #[setter(capture_point_count)]
    fn set_capture_point_count(&mut self, value: usize) {
        self.flags.capture_point_count = value;
    }

    /// How close the agent must be to a dropped flag to pick it up.
    #[getter(pickup_radius)]
    fn get_pickup_radius(&self) -> f32 {
        self.flags.pickup_radius
    }
    #[setter(pickup_radius)]
    fn set_pickup_radius(&mut self, value: f32) {
        self.flags.pickup_radius = value;
    }

    /// How close a dropped flag must be to a capture point to be captured.
    #[getter(capture_radius)]
    fn get_capture_radius(&self) -> f32 {
        self.flags.capture_radius
    }
    #[setter(capture_radius)]
    fn set_capture_radius(&mut self, value: f32) {
        self.flags.capture_radius = value;
    }

    #[getter(seed)]
    fn get_seed(&self) -> Option<u32> {
        self.maze_generation.seed
    }
    #[setter(seed)]
    fn set_seed(&mut self, value: Option<u32>) {
        self.maze_generation.seed = value;
    }

    #[getter(world_width)]
    fn get_world_width(&self) -> f32 {
        self.maze_generation.world_width
    }
    #[setter(world_width)]
    fn set_world_width(&mut self, value: f32) {
        self.maze_generation.world_width = value;
    }

    #[getter(world_height)]
    fn get_world_height(&self) -> f32 {
        self.maze_generation.world_height
    }
    #[setter(world_height)]
    fn set_world_height(&mut self, value: f32) {
        self.maze_generation.world_height = value;
    }

    #[getter(cell_size)]
    fn get_cell_size(&self) -> f32 {
        self.maze_generation.cell_size
    }
    #[setter(cell_size)]
    fn set_cell_size(&mut self, value: f32) {
        self.maze_generation.cell_size = value;
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
        // A blind agent and flags that must be stood on exactly are degenerate the same way.
        degenerate.agent.raycast_count = 0;
        degenerate.flags.pickup_radius = 0.0;
        degenerate.flags.capture_radius = 0.0;
        assert!(degenerate.validate_settings().is_ok());

        let rejects = |break_it: fn(&mut MazeConfig)| {
            let mut config = MazeConfig::default();
            break_it(&mut config);
            config.validate_settings().is_err()
        };

        assert!(rejects(|c| c.agent.max_speed = -1.0), "negative max_speed");
        assert!(
            rejects(|c| c.agent.position_stddev = -1.0),
            "negative stddev"
        );
        assert!(
            rejects(|c| c.agent.occupancy_grid_cell_size = 0.0),
            "zero grid cell"
        );
        assert!(
            rejects(|c| c.agent.occupancy_grid_cell_size = -1.0),
            "negative grid cell"
        );
        assert!(
            rejects(|c| c.maze_generation.world_width = 0.0),
            "zero world"
        );
        assert!(
            rejects(|c| c.maze_generation.cell_size = 0.0),
            "zero maze cell"
        );
        assert!(
            rejects(|c| c.maze_generation.cell_size = 1000.0),
            "maze cell larger than the world"
        );
        assert!(
            rejects(|c| c.agent.raycast_count = 100_000),
            "absurd ray count"
        );
        assert!(
            rejects(|c| c.agent.raycast_max_distance = 0.0),
            "zero ray reach"
        );
        assert!(
            rejects(|c| c.flags.pickup_radius = -1.0),
            "negative flag radius"
        );
        assert!(
            rejects(|c| c.flags.capture_radius = f32::NAN),
            "non-finite capture point radius"
        );
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
