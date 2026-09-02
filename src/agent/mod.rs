mod components;
mod systems;
mod visual;

use bevy::prelude::*;
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
use serde::{Deserialize, Serialize};

pub use components::*;

use crate::core::{MazeConfig, StartupSets};

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
    pub speed: f32,

    #[pyo3(get, set)]
    #[derivative(Default(value = "60.0"))]
    pub policy_hz: f32,

    #[pyo3(get, set)]
    pub odometry_stddev: f32,

    #[pyo3(get, set)]
    pub range_stddev: f32,

    #[pyo3(get, set)]
    #[derivative(Default(value = "1.0"))]
    pub occupancy_grid_cell_size: f32,
}

impl AgentConfig {
    const MIN_POLICY_HZ: f32 = 1.0;
    const MAX_POLICY_HZ: f32 = 240.0;

    /// The bounded rate used by both the policy timer and the headless clock.
    pub(crate) fn effective_policy_hz(&self) -> f32 {
        self.policy_hz
            .clamp(Self::MIN_POLICY_HZ, Self::MAX_POLICY_HZ)
    }

    pub(crate) fn policy_interval_secs(&self) -> f32 {
        self.effective_policy_hz().recip()
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
    fn policy_rate_and_interval_share_the_same_bounds() {
        let cases = [(0.5, 1.0), (60.0, 60.0), (500.0, 240.0)];

        for (requested, effective) in cases {
            let config = AgentConfig {
                policy_hz: requested,
                ..Default::default()
            };
            assert_eq!(config.effective_policy_hz(), effective);
            assert!((config.policy_interval_secs() - effective.recip()).abs() < f32::EPSILON);
        }
    }
}
