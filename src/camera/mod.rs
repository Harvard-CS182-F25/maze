mod systems;

use bevy::input::common_conditions::input_pressed;
use bevy::prelude::*;

use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
use serde::{Deserialize, Serialize};

#[gen_stub_pyclass]
#[pyclass(name = "CameraConfig")]
#[derive(Debug, Clone, Resource, Reflect, Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[serde(default)]
#[reflect(Resource)]
pub struct CameraConfig {
    #[pyo3(get, set)]
    #[derivative(Default(value = "-0.15"))]
    pub scale: f32,
}

#[pymethods]
impl CameraConfig {
    fn __repr__(&self) -> PyResult<String> {
        Ok(format!("CameraConfig({})", self.__str__()?))
    }

    fn __str__(&self) -> PyResult<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "Failed to serialize CameraConfig: {}",
                e
            ))
        })
    }
}

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, systems::setup_camera);
        app.add_systems(
            Update,
            (
                systems::zoom_in.run_if(input_pressed(KeyCode::Equal)),
                systems::zoom_out.run_if(input_pressed(KeyCode::Minus)),
                systems::pan_camera,
            ),
        );
    }
}
