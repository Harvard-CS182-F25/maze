use std::sync::Arc;
use std::sync::RwLock;

use bevy::prelude::*;
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
use serde::{Deserialize, Serialize};

use crate::agent;
use crate::camera;
use crate::character_controller;
use crate::flag;
use crate::interaction_range;
use crate::python::occupancy_grid::OccupancyGrid;
use crate::scene;

#[gen_stub_pyclass]
#[pyclass(name = "MazeConfig")]
#[derive(Debug, Clone, Resource, Reflect, Serialize, Deserialize, Derivative)]
#[derivative(Default)]
#[serde(default)]
#[reflect(Resource)]
pub struct MazeConfig {
    #[pyo3(get, set)]
    pub agent: agent::AgentConfig,
    #[pyo3(get, set)]
    pub flags: flag::FlagConfig,
    #[pyo3(get, set)]
    pub capture_points: flag::CapturePointConfig,
    #[pyo3(get, set)]
    pub camera: camera::CameraConfig,
    #[pyo3(get, set)]
    pub maze_generation: scene::MazeGenerationConfig,
    #[pyo3(get, set)]
    pub debug: bool,
    #[pyo3(get, set)]
    pub headless: bool,
}

#[pymethods]
impl MazeConfig {
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

#[derive(Resource, Clone)]
pub struct PlayerGrid(pub Arc<RwLock<Py<OccupancyGrid>>>);

#[derive(Resource, Clone)]
pub struct TrueGrid(pub Arc<RwLock<Py<OccupancyGrid>>>);

pub trait PyGridProvider: Resource + Clone + Send + Sync + 'static {
    fn arc(&self) -> &Arc<RwLock<Py<OccupancyGrid>>>;
    fn name() -> &'static str;
}
impl PyGridProvider for PlayerGrid {
    fn arc(&self) -> &Arc<RwLock<Py<OccupancyGrid>>> {
        &self.0
    }
    fn name() -> &'static str {
        "player"
    }
}
impl PyGridProvider for TrueGrid {
    fn arc(&self) -> &Arc<RwLock<Py<OccupancyGrid>>> {
        &self.0
    }
    fn name() -> &'static str {
        "truth"
    }
}

impl Plugin for MazePlugin {
    fn build(&self, app: &mut App) {
        let width = (self.config.maze_generation.width / self.config.agent.occupancy_grid_cell_size)
            .round() as usize;
        let height = (self.config.maze_generation.height
            / self.config.agent.occupancy_grid_cell_size)
            .round() as usize;

        app.insert_resource(self.config.clone());

        let player_grid = Python::attach(|py| Py::new(py, OccupancyGrid::new(width, height)))
            .expect("Failed to create OccupancyGrid");
        let true_grid = Python::attach(|py| Py::new(py, OccupancyGrid::new(width, height)))
            .expect("Failed to create OccupancyGrid");

        app.insert_resource(PlayerGrid(Arc::new(RwLock::new(player_grid))));
        app.insert_resource(TrueGrid(Arc::new(RwLock::new(true_grid))));

        app.add_plugins((
            camera::CameraPlugin,
            character_controller::CharacterControllerPlugin,
            agent::AgentPlugin,
            flag::FlagPlugin,
            interaction_range::InteractionRangePlugin,
            scene::ScenePlugin,
        ));
    }
}
