mod systems;

use bevy::prelude::*;
use derivative::Derivative;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Resource, Reflect, Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[serde(default)]
#[reflect(Resource)]
pub struct CameraConfig {
    #[derivative(Default(value = "-0.15"))]
    pub scale: f32,
}

pub struct CameraPlugin;
impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, systems::setup_camera);
    }
}
