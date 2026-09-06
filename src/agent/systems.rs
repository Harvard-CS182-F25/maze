use bevy::prelude::*;
use pyo3::prelude::*;
use rand::SeedableRng;
use rand::seq::IndexedRandom;
use rand_chacha::ChaCha20Rng;

use crate::agent::{AGENT_RAYCAST_MAX_DISTANCE, GhostAgentBundle};
use crate::core::MazeConfig;
use crate::occupancy_grid::TrueGrid;
use crate::python::game_state::EntityType;

use super::components::AgentBundle;
use super::visual::AgentGraphicsAssets;

pub fn spawn_agents(
    mut commands: Commands,
    graphics: Option<Res<AgentGraphicsAssets>>,
    config: Res<MazeConfig>,
    true_grid: ResMut<TrueGrid>,
) {
    let position = Python::attach(|py| {
        let grid = true_grid.0.read().unwrap();
        let py_obj = grid.borrow(py);

        let free_positions: Vec<(f32, f32)> = py_obj
            .grid
            .iter()
            .enumerate()
            .filter_map(|(i, cell)| {
                if cell.assignment != Some(EntityType::Free) {
                    return None;
                }
                py_obj.world_center(i % py_obj.columns, i / py_obj.columns)
            })
            .collect();

        if free_positions.is_empty() {
            panic!("No free positions available to spawn the agent");
        }

        let mut rng = ChaCha20Rng::from_seed({
            let mut arr = [0u8; 32];
            let seed = config
                .maze_generation
                .seed
                .expect("Should have generated a seed before the map generation");
            arr[..4].copy_from_slice(&seed.to_le_bytes());
            arr
        });
        free_positions.choose(&mut rng).copied().unwrap()
    });

    info!("Spawning agent at position: {:?}", position);

    let entity = commands
        .spawn(AgentBundle::new(
            &config.agent.name,
            Vec3::new(position.0, 0.0, position.1),
            config.agent.max_speed,
            AGENT_RAYCAST_MAX_DISTANCE,
        ))
        .id();

    let ghost_entity = commands
        .spawn((
            GhostAgentBundle::new(
                &format!("{}-ghost", config.agent.name),
                Vec3::new(position.0, 0.0, position.1),
            ),
            Visibility::Hidden,
        ))
        .id();

    if let Some(graphics) = graphics {
        commands.entity(entity).insert((
            Mesh3d(graphics.mesh.clone()),
            MeshMaterial3d(graphics.material.clone()),
        ));

        commands.entity(ghost_entity).insert((
            Mesh3d(graphics.mesh.clone()),
            MeshMaterial3d(graphics.ghost_material.clone()),
        ));
    }
}
