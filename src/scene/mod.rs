mod components;
mod systems;
mod visual;

use bevy::prelude::*;

pub use components::*;
pub use visual::*;

use crate::core::MazeConfig;

pub const COLLISION_LAYER_WALL: u32 = 1 << 0;
pub const WALL_HEIGHT: f32 = 5.0;
const WALL_THICKNESS: f32 = 1.0;

pub struct ScenePlugin;
impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 1_500.0,
            ..Default::default()
        });

        app.add_systems(PreStartup, init_wall_assets);
        app.add_systems(Startup, systems::setup_scene);
        app.add_systems(Startup, systems::spawn_walls);
    }
}

fn init_wall_assets(mut commands: Commands, config: Res<MazeConfig>) {
    if !config.headless {
        commands.init_resource::<WallGraphicsAssets>();
    }
}
