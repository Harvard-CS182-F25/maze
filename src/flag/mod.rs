mod components;
mod systems;
mod visual;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub use components::*;

use crate::core::MazeConfig;

pub const FLAG_INTERACTION_RADIUS: f32 = 3.0;
pub const CAPTURE_POINT_INTERACTION_RADIUS: f32 = 3.0;

#[derive(Debug, Clone, Default, Resource, Reflect, Serialize, Deserialize)]
#[serde(default)]
#[reflect(Resource)]
pub struct FlagConfig {
    pub positions: Vec<(f32, f32)>,
}

#[derive(Debug, Clone, Default, Resource, Reflect, Serialize, Deserialize)]
#[serde(default)]
#[reflect(Resource)]
pub struct CapturePointConfig {
    pub positions: Vec<(f32, f32)>,
}

pub struct FlagPlugin;
impl Plugin for FlagPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<components::FlagCaptureCounts>();
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
