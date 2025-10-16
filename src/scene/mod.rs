mod components;
mod systems;
mod visual;

use bevy::prelude::*;
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
use serde::{Deserialize, Serialize};

pub use components::*;
pub use visual::*;

use crate::core::MazeConfig;

pub const COLLISION_LAYER_WALL: u32 = 1 << 0;
pub const WALL_HEIGHT: f32 = 5.0;
const WALL_THICKNESS: f32 = 1.0;

#[gen_stub_pyclass]
#[pyclass(name = "MazeGenerationConfig")]
#[derive(Debug, Clone, Resource, Reflect, Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[reflect(Resource)]
#[serde(default)]
pub struct MazeGenerationConfig {
    #[pyo3(get, set)]
    pub seed: Option<u32>,
    #[pyo3(get, set)]
    #[derivative(Default(value = "100.0"))]
    pub width: f32,
    #[pyo3(get, set)]
    #[derivative(Default(value = "100.0"))]
    pub height: f32,
    #[pyo3(get, set)]
    #[derivative(Default(value = "5.0"))]
    pub cell_size: f32,
}

pub struct ScenePlugin;
impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1_500.0,
            ..Default::default()
        });

        app.add_systems(PreStartup, init_wall_assets);
        app.add_systems(Startup, (systems::setup_scene, systems::spawn_walls));
    }
}

fn init_wall_assets(mut commands: Commands, config: Res<MazeConfig>) {
    if !config.headless {
        commands.init_resource::<WallGraphicsAssets>();
    }
}
