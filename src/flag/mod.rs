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

pub const FLAG_INTERACTION_RADIUS: f32 = 3.0;
pub const CAPTURE_POINT_INTERACTION_RADIUS: f32 = 3.0;
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
            systems::update_true_grid
                .after(TransformSystems::Propagate)
                .in_set(SimulationSets::TrueGrid)
                .run_if(|config: Res<MazeConfig>| !config.headless || config.use_true_map),
        );
    }
}

fn init_flag_and_capture_point_assets(mut commands: Commands, config: Res<MazeConfig>) {
    if !config.headless {
        commands.init_resource::<visual::FlagGraphicsAssets>();
        commands.init_resource::<visual::CapturePointGraphicsAssets>();
    }
}
