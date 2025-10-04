use bevy::prelude::*;

use crate::core::MazeConfig;

use super::components::AgentBundle;
use super::visual::AgentGraphicsAssets;

pub fn spawn_agents(
    mut commands: Commands,
    graphics: Option<Res<AgentGraphicsAssets>>,
    config: Res<MazeConfig>,
) {
    let mut entity = commands.spawn(AgentBundle::new(
        &config.agent.name,
        Vec3::new(config.agent.position.0, 0.0, config.agent.position.1),
        config.agent.speed,
    ));

    info!("Spawning agent at position: {:?}", config.agent.position);

    if let Some(graphics) = graphics {
        entity.insert((
            Mesh3d(graphics.mesh.clone()),
            MeshMaterial3d(graphics.material.clone()),
        ));
    }
}
