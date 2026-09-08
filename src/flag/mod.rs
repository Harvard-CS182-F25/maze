mod components;
mod systems;
mod visual;

use bevy::{prelude::*, transform::TransformSystems};
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
use serde::{Deserialize, Serialize};

pub use components::*;

use crate::core::{MazeConfig, SimulationSets, StartupSets};

pub(crate) const FLAG_INTERACTION_RADIUS: f32 = 3.0;
pub(crate) const CAPTURE_POINT_INTERACTION_RADIUS: f32 = 3.0;
pub const COLLISION_LAYER_FLAG: u32 = 1 << 2;
pub const COLLISION_LAYER_CAPTURE_POINT: u32 = 1 << 3;

#[gen_stub_pyclass]
#[pyclass(name = "FlagConfig")]
#[derive(Debug, Clone, Resource, Reflect, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
#[serde(default, deny_unknown_fields)]
#[reflect(Resource)]
pub struct FlagConfig {
    #[pyo3(get, set)]
    #[derivative(Default(value = "1"))]
    pub flag_count: usize,
    #[pyo3(get, set)]
    #[derivative(Default(value = "1"))]
    pub capture_point_count: usize,

    /// How close the agent must be to a dropped flag to pick it up.
    #[pyo3(get, set)]
    #[derivative(Default(value = "FLAG_INTERACTION_RADIUS"))]
    pub pickup_radius: f32,

    /// How close a dropped flag must be to a capture point to be captured.
    #[pyo3(get, set)]
    #[derivative(Default(value = "CAPTURE_POINT_INTERACTION_RADIUS"))]
    pub capture_radius: f32,
}

impl FlagConfig {
    /// Flags and capture points are spawned this far from walls and from each other, so that a
    /// reachable flag never sits inside a wall's clearance or on top of another one.
    pub(crate) fn spawn_clearance(&self) -> f32 {
        self.pickup_radius.max(self.capture_radius)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        // Zero is degenerate but coherent: the agent has to stand on the flag exactly.
        for (name, value) in [
            ("flags.pickup_radius", self.pickup_radius),
            ("flags.capture_radius", self.capture_radius),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(format!("{name} must be zero or positive; got {value}"));
            }
        }

        Ok(())
    }
}

#[pymethods]
impl FlagConfig {
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("FlagConfig({})", self.__str__()?))
    }

    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to serialize FlagConfig: {}",
                e
            ))
        })
    }
}

pub struct FlagPlugin;
impl Plugin for FlagPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<components::FlagCaptureCounts>();
        app.add_systems(PreStartup, init_flag_and_capture_point_assets);
        app.add_systems(
            Startup,
            (systems::spawn_flags, systems::spawn_capture_points)
                .in_set(StartupSets::FlagsAndCapturePoints),
        );

        app.add_systems(
            FixedPostUpdate,
            // Flag and capture-point positions are read from their propagated global transforms.
            // This runs headless too: skipping it there froze the true map at its spawn state, so
            // a headless score disagreed with what the same run showed in a window.
            systems::update_true_grid
                .after(TransformSystems::Propagate)
                .in_set(SimulationSets::TrueGrid),
        );
    }
}

fn init_flag_and_capture_point_assets(mut commands: Commands, config: Res<MazeConfig>) {
    if !config.headless {
        commands.init_resource::<visual::FlagGraphicsAssets>();
        commands.init_resource::<visual::CapturePointGraphicsAssets>();
    }
}
