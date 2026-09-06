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
pub const NUM_AGENT_RAYS: u32 = 16;
pub const AGENT_RAYCAST_MAX_DISTANCE: f32 = 20.0;

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
    #[pyo3(get, set)]
    #[derivative(Default(value = "60.0"))]
    pub policy_hz: f32,

    #[pyo3(get, set)]
    pub position_stddev: f32,

    #[pyo3(get, set)]
    pub range_stddev: f32,

    #[pyo3(get, set)]
    #[derivative(Default(value = "1.0"))]
    pub occupancy_grid_cell_size: f32,
}

impl AgentConfig {
    /// Any slower and a run would look silently broken: the policy would be queried so rarely
    /// that nothing visibly happens, which is what `policy_hz = 0` is for instead.
    const MIN_POLICY_HZ: f32 = 1.0;
    const MAX_POLICY_HZ: f32 = SIMULATION_HZ;

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
