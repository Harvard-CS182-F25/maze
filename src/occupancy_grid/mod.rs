mod components;
mod systems;

use std::sync::{Arc, RwLock};

use bevy::{input::common_conditions::input_just_pressed, prelude::*};
use pyo3::prelude::*;

pub use components::*;

use crate::core::MazeConfig;

pub struct OccupancyGridPlugin {
    pub config: MazeConfig,
}

impl Plugin for OccupancyGridPlugin {
    fn build(&self, app: &mut App) {
        let width = (self.config.maze_generation.width / self.config.agent.occupancy_grid_cell_size)
            .round() as usize;
        let height = (self.config.maze_generation.height
            / self.config.agent.occupancy_grid_cell_size)
            .round() as usize;

        let true_arc = {
            let true_grid = Python::attach(|py| {
                Py::new(
                    py,
                    OccupancyGrid::new(width, height, self.config.agent.occupancy_grid_cell_size),
                )
            })
            .expect("Failed to create OccupancyGrid");
            Arc::new(RwLock::new(true_grid))
        };
        let player_arc = if self.config.use_true_map {
            true_arc.clone()
        } else {
            let player_grid = Python::attach(|py| {
                Py::new(
                    py,
                    OccupancyGrid::new(width, height, self.config.agent.occupancy_grid_cell_size),
                )
            })
            .expect("Failed to create OccupancyGrid");

            Arc::new(RwLock::new(player_grid))
        };

        app.insert_resource(PlayerGrid(player_arc));
        app.insert_resource(TrueGrid(true_arc));
        app.insert_resource(HoverCell {
            cell: None,
            world_hit: None,
        });

        app.add_systems(
            Startup,
            (
                systems::setup_key_instructions,
                systems::setup_hover_box::<PlayerGrid>,
                systems::setup_hover_box::<TrueGrid>,
                systems::spawn_grid_texture::<PlayerGrid>,
                systems::spawn_grid_texture::<TrueGrid>,
            )
                .run_if(|config: Res<MazeConfig>| !config.headless),
        );
        app.add_systems(
            Update,
            (
                systems::update_grid_texture::<PlayerGrid>,
                systems::toggle_grid::<PlayerGrid, TrueGrid>
                    .run_if(input_just_pressed(KeyCode::KeyO)),
                systems::update_grid_texture::<TrueGrid>,
                systems::toggle_grid::<TrueGrid, PlayerGrid>
                    .run_if(input_just_pressed(KeyCode::KeyT)),
                systems::cursor_to_grid_cell::<PlayerGrid>,
                systems::update_hover_box::<PlayerGrid>,
                systems::cursor_to_grid_cell::<TrueGrid>,
                systems::update_hover_box::<TrueGrid>,
            )
                .run_if(|config: Res<MazeConfig>| !config.headless),
        );
    }
}
