mod systems;

use bevy::input::common_conditions::input_pressed;
use bevy::prelude::*;

use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
use serde::{Deserialize, Serialize};

const FIT_MARGIN: f32 = 1.15;
const ZOOM_OUT_MARGIN: f32 = 3.0;
const MAX_CELL_SIZE_PX: f32 = 500.0;

/// Board-relative bounds for orthographic camera zoom.
///
/// The limits use the initial configured scale as part of their range, so a
/// user-provided `CameraConfig::scale` remains a valid starting point.
#[derive(Resource)]
pub(super) struct CameraZoomLimits {
    min_scale: f32,
    max_scale: f32,
}

impl Default for CameraZoomLimits {
    fn default() -> Self {
        Self {
            min_scale: 0.0,
            max_scale: f32::INFINITY,
        }
    }
}

impl CameraZoomLimits {
    fn new(board_size: Vec2, cell_size: f32, viewport_size: Vec2, initial_scale: f32) -> Self {
        let initial_scale = initial_scale.abs();
        Self {
            min_scale: (cell_size / MAX_CELL_SIZE_PX).min(initial_scale),
            max_scale: (fit_scale(board_size, viewport_size) * ZOOM_OUT_MARGIN).max(initial_scale),
        }
    }
}

fn fit_scale(board_size: Vec2, viewport_size: Vec2) -> f32 {
    let viewport_size = viewport_size.max(Vec2::ONE);
    (board_size.x * FIT_MARGIN / viewport_size.x).max(board_size.y * FIT_MARGIN / viewport_size.y)
}

#[gen_stub_pyclass]
#[pyclass(name = "CameraConfig")]
#[derive(Debug, Clone, Resource, Reflect, Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[serde(default, deny_unknown_fields)]
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
        app.init_resource::<systems::PanState>();
        app.init_resource::<CameraZoomLimits>();
        app.add_systems(Startup, systems::setup_camera);
        app.add_systems(
            Update,
            (
                systems::zoom_in.run_if(input_pressed(KeyCode::Equal)),
                systems::zoom_out.run_if(input_pressed(KeyCode::Minus)),
                systems::pan_camera,
                systems::update_pan_cursor,
            ),
        );
    }
}
