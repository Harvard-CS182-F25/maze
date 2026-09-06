mod components;
mod systems;
mod visual;

use bevy::prelude::*;
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
use serde::{Deserialize, Serialize};

pub use components::*;

use crate::core::{MazeConfig, SIMULATION_HZ, StartupSets};

pub const COLLISION_LAYER_AGENT: u32 = 1 << 1;
pub(crate) const NUM_AGENT_RAYS: u32 = 16;
pub(crate) const AGENT_RAYCAST_MAX_DISTANCE: f32 = 20.0;

#[gen_stub_pyclass]
#[pyclass(name = "AgentConfig")]
#[derive(Debug, Clone, Resource, Reflect, Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[reflect(Resource)]
#[serde(default, deny_unknown_fields)]
pub struct AgentConfig {
    #[pyo3(get, set)]
    #[derivative(Default(value = "\"Agent\".to_string()"))]
    pub name: String,

    #[pyo3(get, set)]
    #[derivative(Default(value = "10.0"))]
    pub max_speed: f32,

    /// How often `get_action` is called, in Hz. Zero disables the policy entirely, which is only
    /// useful for a keyboard-only teleop demonstration.
    ///
    /// The policy runs on simulation ticks, which are fixed at 60 Hz, so a rate that does not
    /// divide 60 cannot be hit exactly: it is correct on average, but the `dt` handed to
    /// `get_action` alternates between neighbouring tick counts. At 7 Hz, for instance, `dt`
    /// alternates between 0.133 and 0.150 rather than sitting at 1/7. A rate that divides 60
    /// gives a constant `dt`.
    #[pyo3(get, set)]
    #[derivative(Default(value = "60.0"))]
    pub policy_hz: f32,

    #[pyo3(get, set)]
    pub position_stddev: f32,

    #[pyo3(get, set)]
    pub range_stddev: f32,

    /// How many rays the agent casts, spread evenly over a full turn. All of them are cast on the
    /// tick the policy is queried, so the cost of a tick grows with this. Zero leaves the agent
    /// with no range sensor at all.
    #[pyo3(get, set)]
    #[derivative(Default(value = "NUM_AGENT_RAYS"))]
    pub raycast_count: u32,

    /// How far each ray reaches. A ray that hits nothing within this distance reports the distance
    /// itself, so a reading equal to it means "nothing found", not "a wall exactly here".
    #[pyo3(get, set)]
    #[derivative(Default(value = "AGENT_RAYCAST_MAX_DISTANCE"))]
    pub raycast_max_distance: f32,

    #[pyo3(get, set)]
    #[derivative(Default(value = "1.0"))]
    pub occupancy_grid_cell_size: f32,

    /// How long the simulation waits for one `get_action` call before giving up on the policy, in
    /// seconds. Zero waits forever, which is what a debugger session needs.
    #[pyo3(get, set)]
    #[derivative(Default(value = "60.0"))]
    pub policy_timeout_seconds: f32,
}

impl AgentConfig {
    /// Any slower and a run would look silently broken: the policy would be queried so rarely
    /// that nothing visibly happens, which is what `policy_hz = 0` is for instead.
    const MIN_POLICY_HZ: f32 = 1.0;
    const MAX_POLICY_HZ: f32 = SIMULATION_HZ;

    /// Far past the point where more rays sample the grid any finer, and low enough that a stray
    /// digit cannot make a tick take seconds.
    const MAX_RAYCAST_COUNT: u32 = 512;

    /// The rate to query the policy at, or `None` when `policy_hz` turns the policy off.
    pub(crate) fn active_policy_hz(&self) -> Option<f32> {
        (self.policy_hz > 0.0).then_some(self.policy_hz)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        // NaN and the infinities fail the range check, so they need no separate arm.
        if self.policy_hz != 0.0
            && !(Self::MIN_POLICY_HZ..=Self::MAX_POLICY_HZ).contains(&self.policy_hz)
        {
            return Err(format!(
                "agent.policy_hz must be 0, which disables the policy, or between {} and {}; got {}",
                Self::MIN_POLICY_HZ,
                Self::MAX_POLICY_HZ,
                self.policy_hz
            ));
        }

        // Zero is allowed where it is merely degenerate: a motionless agent and a noiseless
        // sensor both describe a real setup.
        for (name, value) in [
            ("agent.max_speed", self.max_speed),
            ("agent.position_stddev", self.position_stddev),
            ("agent.range_stddev", self.range_stddev),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!("{name} must be zero or positive; got {value}"));
            }
        }

        if self.raycast_count > Self::MAX_RAYCAST_COUNT {
            return Err(format!(
                "agent.raycast_count must be at most {}; got {}",
                Self::MAX_RAYCAST_COUNT,
                self.raycast_count
            ));
        }

        if !self.raycast_max_distance.is_finite() || self.raycast_max_distance <= 0.0 {
            return Err(format!(
                "agent.raycast_max_distance must be positive; got {}. Use agent.raycast_count 0 \
                 for an agent with no range sensor.",
                self.raycast_max_distance
            ));
        }

        if !self.occupancy_grid_cell_size.is_finite() || self.occupancy_grid_cell_size <= 0.0 {
            return Err(format!(
                "agent.occupancy_grid_cell_size must be positive; got {}",
                self.occupancy_grid_cell_size
            ));
        }

        if !self.policy_timeout_seconds.is_finite() || self.policy_timeout_seconds < 0.0 {
            return Err(format!(
                "agent.policy_timeout_seconds must be zero, which waits forever, or positive; got {}",
                self.policy_timeout_seconds
            ));
        }

        Ok(())
    }
}

#[pymethods]
impl AgentConfig {
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("AgentConfig({})", self.__str__()?))
    }

    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to serialize AgentConfig: {}",
                e
            ))
        })
    }
}

pub struct AgentPlugin;
impl Plugin for AgentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, spawn_agent_assets);
        app.add_systems(Startup, systems::spawn_agents.in_set(StartupSets::Agents));
    }
}

fn spawn_agent_assets(mut commands: Commands, config: Res<MazeConfig>) {
    if config.headless {
        return;
    }

    commands.init_resource::<visual::AgentGraphicsAssets>();
}

#[cfg(test)]
mod tests {
    use super::AgentConfig;

    #[test]
    fn policy_rate_accepts_zero_and_is_bounded_by_the_simulation_rate() {
        for requested in [0.0, 1.0, 30.0, 60.0] {
            let config = AgentConfig {
                policy_hz: requested,
                ..Default::default()
            };
            assert!(config.validate().is_ok());
        }

        // A rate just above zero would query the policy so rarely that a run looks broken while
        // reporting no error at all, so it is rejected rather than treated as "disabled".
        for requested in [-1.0, 1e-4, 0.5, 60.1, f32::NAN, f32::INFINITY] {
            let config = AgentConfig {
                policy_hz: requested,
                ..Default::default()
            };
            assert!(config.validate().is_err());
        }

        let disabled = AgentConfig {
            policy_hz: 0.0,
            ..Default::default()
        };
        assert_eq!(disabled.active_policy_hz(), None);
    }
}
