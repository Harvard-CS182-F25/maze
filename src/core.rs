use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::agent;
use crate::camera;
use crate::character_controller;
use crate::flag;
use crate::interaction_range;
use crate::scene;

#[derive(Debug, Clone, Default, Resource, Reflect, Serialize, Deserialize)]
#[serde(default)]
#[reflect(Resource)]
pub struct MazeConfig {
    pub agent: agent::AgentConfig,
    pub flags: flag::FlagConfig,
    pub capture_points: flag::CapturePointConfig,
    pub camera: camera::CameraConfig,
    pub debug: bool,
    pub headless: bool,
}

pub struct MazePlugin {
    pub config: MazeConfig,
}

impl Plugin for MazePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());

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
