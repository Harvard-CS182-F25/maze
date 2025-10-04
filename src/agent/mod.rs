mod components;
mod systems;
mod visual;

use bevy::prelude::*;
use derivative::Derivative;
use serde::{Deserialize, Serialize};

pub use components::*;
pub use visual::*;

use crate::core::MazeConfig;

pub const COLLISION_LAYER_AGENT: u32 = 1 << 1;

#[derive(Debug, Clone, Resource, Reflect, Derivative, Serialize, Deserialize)]
#[derivative(Default)]
#[reflect(Resource)]
#[serde(default)]
pub struct AgentConfig {
    #[derivative(Default(value = "\"Agent\".to_string()"))]
    pub name: String,
    #[derivative(Default(value = "10.0"))]
    pub speed: f32,
    pub position: (f32, f32),
    #[derivative(Default(value = "60.0"))]
    pub policy_hz: f32,
}

pub struct AgentPlugin;
impl Plugin for AgentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, spawn_agent_assets);
        app.add_systems(Startup, systems::spawn_agents);
    }
}

fn spawn_agent_assets(mut commands: Commands, config: Res<MazeConfig>) {
    if config.headless {
        return;
    }

    commands.init_resource::<visual::AgentGraphicsAssets>();
}
