mod components;
mod systems;
mod visual;

use bevy::prelude::*;
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;
use serde::{Deserialize, Serialize};

pub use components::*;
pub use systems::mapping_metrics;
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

impl MazeGenerationConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        for (name, value) in [
            ("maze_generation.world_width", self.world_width),
            ("maze_generation.world_height", self.world_height),
            ("maze_generation.cell_size", self.cell_size),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(format!("{name} must be positive; got {value}"));
            }
        }

        // The maze generator panics rather than erroring on a zero-sized maze.
        let (columns, rows) = self.maze_dimensions();
        if columns < 1 || rows < 1 {
            return Err(format!(
                "maze_generation.cell_size {} is too large for a {}x{} world: it leaves a {}x{} maze",
                self.cell_size, self.world_width, self.world_height, columns, rows
            ));
        }

        Ok(())
    }

    pub(crate) fn maze_dimensions(&self) -> (i32, i32) {
        (
            (self.world_width / self.cell_size).round() as i32,
            (self.world_height / self.cell_size).round() as i32,
        )
    }

    /// How much ground the maze covers. Rounding to whole cells means this can differ from the
    /// nominal world size in either direction.
    pub(crate) fn maze_extent(&self) -> Vec2 {
        let (columns, rows) = self.maze_dimensions();
        Vec2::new(columns as f32, rows as f32) * self.cell_size
    }
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
        // `update_mapping_metrics` takes the GIL every frame, so running it there is pure waste.
        app.add_systems(
            Update,
            (
                systems::update_time,
                systems::update_policy_duration,
                systems::update_true_position,
                systems::update_mapping_metrics,
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
