mod components;
mod systems;
mod visual;

use bevy::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
use serde::{Deserialize, Serialize};

pub use components::*;

use crate::core::MazeConfig;

pub const FLAG_INTERACTION_RADIUS: f32 = 3.0;
pub const CAPTURE_POINT_INTERACTION_RADIUS: f32 = 3.0;

#[gen_stub_pyclass]
#[pyclass(name = "FlagConfig")]
#[derive(Debug, Clone, Default, Resource, Reflect, Serialize, Deserialize)]
#[serde(default)]
#[reflect(Resource)]
pub struct FlagConfig {
    #[pyo3(get, set)]
    pub positions: Vec<(f32, f32)>,
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

#[gen_stub_pyclass]
#[pyclass(name = "CapturePointConfig")]
#[derive(Debug, Clone, Default, Resource, Reflect, Serialize, Deserialize)]
#[serde(default)]
#[reflect(Resource)]
pub struct CapturePointConfig {
    #[pyo3(get, set)]
    pub positions: Vec<(f32, f32)>,
}

#[pymethods]
impl CapturePointConfig {
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("CapturePointConfig({})", self.__str__()?))
    }

    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to serialize CapturePointConfig: {}",
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
        app.add_systems(Startup, systems::spawn_flags);
        app.add_systems(Startup, systems::spawn_capture_points);
    }
}

fn init_flag_and_capture_point_assets(mut commands: Commands, config: Res<MazeConfig>) {
    if !config.headless {
        commands.init_resource::<visual::FlagGraphicsAssets>();
        commands.init_resource::<visual::CapturePointGraphicsAssets>();
    }
}
