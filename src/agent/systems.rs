use bevy::prelude::*;
use pyo3::prelude::*;
use rand::SeedableRng;
use rand::seq::IndexedRandom;
use rand_chacha::ChaCha20Rng;

use crate::agent::AGENT_RAYCAST_MAX_DISTANCE;
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
                if cell.assignment == Some(EntityType::Empty) {
                    let grid_col = i as u32 % py_obj.width as u32;
                    let grid_row = i as u32 / py_obj.width as u32;
                    let x = (grid_col as f32) * config.agent.occupancy_grid_cell_size
                        + config.agent.occupancy_grid_cell_size / 2.0
                        - config.maze_generation.width / 2.0;
                    let y = (grid_row as f32) * config.agent.occupancy_grid_cell_size
                        + config.agent.occupancy_grid_cell_size / 2.0
                        - config.maze_generation.height / 2.0;
                    Some((x, y))
                } else {
                    None
                }
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

    let mut entity = commands.spawn(AgentBundle::new(
        &config.agent.name,
        Vec3::new(position.0, 0.0, position.1),
        config.agent.speed,
        AGENT_RAYCAST_MAX_DISTANCE,
    ));

    info!("Spawning agent at position: {:?}", position);

    if let Some(graphics) = graphics {
        entity.insert((
            Mesh3d(graphics.mesh.clone()),
            MeshMaterial3d(graphics.material.clone()),
        ));
    }
}
