mod components;
mod systems;
mod visual;

use bevy::prelude::*;
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
use serde::{Deserialize, Serialize};

pub use components::*;
pub use systems::mapping_error;
pub use visual::*;

use crate::core::{MazeConfig, StartupSets};

pub const COLLISION_LAYER_WALL: u32 = 1 << 0;
pub const WALL_HEIGHT: f32 = 5.0;
pub const WALL_THICKNESS: f32 = 1.0;

#[gen_stub_pyclass]
#[pyclass(name = "MazeGenerationConfig")]
#[derive(Debug, Clone, Resource, Reflect, Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[reflect(Resource)]
#[serde(default, deny_unknown_fields)]
pub struct MazeGenerationConfig {
    #[pyo3(get, set)]
    pub seed: Option<u32>,
    #[pyo3(get, set)]
    #[derivative(Default(value = "100.0"))]
    pub world_width: f32,
    #[pyo3(get, set)]
    #[derivative(Default(value = "100.0"))]
    pub world_height: f32,
    #[pyo3(get, set)]
    #[derivative(Default(value = "5.0"))]
    pub cell_size: f32,
}

pub struct ScenePlugin;
impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.6, 0.8, 0.8)));
        app.insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 3_000.0,
            ..Default::default()
        });

        app.add_systems(
            PreStartup,
            (
                init_wall_assets,
                systems::initialize_sensor_rng,
                systems::setup_hud,
            )
                .chain(),
        );
        app.add_systems(
            Startup,
            (systems::setup_scene, systems::spawn_walls).in_set(StartupSets::Walls),
        );
        // These only write to HUD text entities, which do not exist in headless mode — and
        // `update_mapping_error` takes the GIL every frame, so running it there is pure waste.
        app.add_systems(
            Update,
            (
                systems::update_time,
                systems::update_true_position,
                systems::update_mapping_error,
                systems::update_flag_progress,
            )
                .run_if(|config: Res<MazeConfig>| !config.headless),
        );
    }
}

fn init_wall_assets(mut commands: Commands, config: Res<MazeConfig>) {
    if !config.headless {
        commands.init_resource::<WallGraphicsAssets>();
    }
}
