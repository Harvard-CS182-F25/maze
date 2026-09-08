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
use crate::scene::GROUND_SURFACE_Y;

pub const COLLISION_LAYER_AGENT: u32 = 1 << 1;
/// Where the agent's centre sits at rest. Its collider is a unit cube, so half of it is below the
/// centre. Spawning any lower buries the raycast origin in the ground, and the first observation
/// comes back as zero range in every direction.
pub(crate) const AGENT_SPAWN_Y: f32 = GROUND_SURFACE_Y + 0.5;
pub(crate) const NUM_AGENT_RAYS: u32 = 16;
/// How far above the agent's centre its rays are cast from.
pub(crate) const AGENT_RAY_ORIGIN_Y: f32 = 0.5;
/// Half the width of the agent's collider. Rays start at its centre, so no solid the agent
/// collides with can ever be nearer than this.
pub(crate) const AGENT_HALF_EXTENT: f32 = 0.5;
pub(crate) const AGENT_RAYCAST_MAX_DISTANCE: f32 = 20.0;

#[gen_stub_pyclass]
#[pyclass(name = "AgentConfig")]
#[derive(Debug, Clone, Resource, Reflect, Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[reflect(Resource)]
#[serde(default, deny_unknown_fields)]
pub struct AgentConfig {
    #[pyo3(get)]
    #[derivative(Default(value = "\"Agent\".to_string()"))]
    pub name: String,

    #[pyo3(get)]
    #[derivative(Default(value = "10.0"))]
    pub max_speed: f32,

    #[pyo3(get)]
    #[derivative(Default(value = "60.0"))]
    pub policy_hz: f32,

    #[pyo3(get)]
    pub position_stddev: f32,

    #[pyo3(get)]
    pub range_stddev: f32,

    #[pyo3(get)]
    #[derivative(Default(value = "NUM_AGENT_RAYS"))]
    pub raycast_count: u32,

    #[pyo3(get)]
    #[derivative(Default(value = "AGENT_RAYCAST_MAX_DISTANCE"))]
    pub raycast_max_distance: f32,

    #[pyo3(get)]
    #[derivative(Default(value = "1.0"))]
    pub occupancy_grid_cell_size: f32,

    #[pyo3(get)]
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
    use super::{AGENT_RAY_ORIGIN_Y, AGENT_SPAWN_Y, AgentConfig};
    use crate::scene::GROUND_SURFACE_Y;

    #[test]
    fn the_agent_casts_its_rays_from_above_the_ground() {
        // A ray starting inside the ground plane reports zero range in every direction, and the
        // policy is queried before physics has had a chance to lift the agent clear.
        assert!(AGENT_SPAWN_Y + AGENT_RAY_ORIGIN_Y > GROUND_SURFACE_Y);
    }

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
