use avian3d::prelude::*;
use bevy::prelude::*;
use maze_generator::prelude::*;
use maze_generator::recursive_backtracking::RbGenerator;
use pyo3::prelude::*;

use crate::python::policy::PolicyCost;
use crate::{
    agent::{Agent, COLLISION_LAYER_AGENT},
    core::MazeConfig,
    flag::{Flag, FlagCaptureCounts},
    occupancy_grid::{LOGIT_CLAMP, PlayerGrid, TrueGrid},
    python::game_state::{EntityType, SensorRng},
    scene::{
        COLLISION_LAYER_WALL, EstimatedPositionText, FlagProgressText, MappingMetricsText,
        SimulationSpeedText, TimeText, TruePositionText, WALL_HEIGHT, WALL_THICKNESS, WallBundle,
        WallGraphicsAssets, WallSegments,
    },
};

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let mut entity = commands.spawn((
        Name::new("Ground Plane"),
        Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(100.0, 1.0, 100.0)),
        RigidBody::Static,
        Collider::cuboid(1.0, 1.0, 1.0),
        CollisionLayers::new(
            LayerMask(COLLISION_LAYER_WALL),
            LayerMask(COLLISION_LAYER_AGENT),
        ),
    ));

    if let (Some(meshes), Some(materials)) = (&mut meshes, &mut materials) {
        let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
        let material = materials.add(Color::srgb(1.0, 1.0, 1.0));
        entity.insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}

#[allow(clippy::too_many_arguments)]
fn push_horizontal(
    segs: &mut Vec<(Vec2, Vec2)>,
    x0: f32,
    z0: f32,
    cell: f32,
    row: i32,
    col: i32,
    pad: f32,
    xmin: f32,
    xmax: f32,
) {
    let z = z0 + (row as f32) * cell;
    let mut ax = x0 + (col as f32) * cell - pad;
    let mut bx = x0 + ((col + 1) as f32) * cell + pad;
    // Padding may extend an interior segment, but never beyond the maze border.
    ax = ax.max(xmin);
    bx = bx.min(xmax);
    segs.push((Vec2::new(ax, z), Vec2::new(bx, z)));
}

#[allow(clippy::too_many_arguments)]
fn push_vertical(
    segs: &mut Vec<(Vec2, Vec2)>,
    x0: f32,
    z0: f32,
    cell: f32,
    col: i32,
    row: i32,
    pad: f32,
    zmin: f32,
    zmax: f32,
) {
    let x = x0 + (col as f32) * cell;
    let mut az = z0 + (row as f32) * cell - pad;
    let mut bz = z0 + ((row + 1) as f32) * cell + pad;
    az = az.max(zmin);
    bz = bz.min(zmax);
    segs.push((Vec2::new(x, az), Vec2::new(x, bz)));
}

pub fn segments_from_maze(maze: &Maze, config: &MazeConfig, pad: f32) -> Vec<(Vec2, Vec2)> {
    let cell = config.maze_generation.cell_size;
    let (w, h) = maze.size;
    let x0 = -(w as f32) * cell * 0.5;
    let z0 = -(h as f32) * cell * 0.5;

    // Maze bounds cap the padded wall segments.
    let xmin = x0;
    let xmax = x0 + (w as f32) * cell;
    let zmin = z0;
    let zmax = z0 + (h as f32) * cell;

    let mut segments = Vec::new();

    // Add each outer border once; East and South borders come from the final row and column.
    for c in 0..w {
        push_horizontal(&mut segments, x0, z0, cell, 0, c, pad, xmin, xmax);
    }
    for r in 0..h {
        push_vertical(&mut segments, x0, z0, cell, 0, r, pad, zmin, zmax);
    }

    // Emit East and South walls only, so shared walls are not duplicated.
    for y in 0..h {
        for x in 0..w {
            let field = maze.get_field(&Coordinates::new(x, y)).expect("in-bounds");
            if !field.has_passage(&Direction::East) {
                push_vertical(&mut segments, x0, z0, cell, x + 1, y, pad, zmin, zmax);
            }
            if !field.has_passage(&Direction::South) {
                push_horizontal(&mut segments, x0, z0, cell, y + 1, x, pad, xmin, xmax);
            }
        }
    }

    segments
}

// Returns cells overlapped by a world-space XZ AABB.
fn overlapping_indexes(
    aabb_min: Vec2,
    aabb_max: Vec2,
    cell_size: f32,
    world_width: f32,
    world_height: f32,
) -> Vec<(u32, u32)> {
    let grid_w = (world_width / cell_size).round() as i32;
    let grid_h = (world_height / cell_size).round() as i32;

    let half_w = world_width * 0.5;
    let half_h = world_height * 0.5;

    let min_c = (((aabb_min.x + half_w) / cell_size).floor() as i32).clamp(0, grid_w - 1);
    let min_r = (((aabb_min.y + half_h) / cell_size).floor() as i32).clamp(0, grid_h - 1);

    let max_c = ((((aabb_max.x + half_w) / cell_size).ceil() as i32) - 1).clamp(0, grid_w - 1);
    let max_r = ((((aabb_max.y + half_h) / cell_size).ceil() as i32) - 1).clamp(0, grid_h - 1);

    if max_c < min_c || max_r < min_r {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(((max_c - min_c + 1) * (max_r - min_r + 1)) as usize);
    for r in min_r..=max_r {
        for c in min_c..=max_c {
            out.push((c as u32, r as u32));
        }
    }
    out
}

pub fn initialize_sensor_rng(mut commands: Commands, mut config: ResMut<MazeConfig>) {
    let seed = if let Some(seed) = config.maze_generation.seed {
        seed
    } else {
        let seed = rand::random::<u16>().into();
        config.maze_generation.seed = Some(seed);
        seed
    };

    info!("Using maze generation seed: {}", seed);

    commands.insert_resource(SensorRng::from_seed(seed));
}

pub fn setup_hud(mut commands: Commands, config: Res<MazeConfig>, time: Res<Time>) {
    if config.headless {
        return;
    }

    let seed = config
        .maze_generation
        .seed
        .expect("Sensor RNG initialization should establish a maze seed before the HUD");

    let line_font = TextFont {
        font_size: 14.0,
        ..default()
    };
    let line_layout = TextLayout::new_with_justify(Justify::Right);

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::Grid,
                top: Val::Px(5.0),
                right: Val::Px(5.0),
                padding: Val::Px(2.5).into(),
                justify_items: JustifyItems::End,
                align_items: AlignItems::Start,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(format!("Maze Seed: {seed}")),
                line_font.clone(),
                line_layout,
            ));

            parent.spawn((
                Text::new(format!("Time: {:.2}s", time.elapsed_secs())),
                line_font.clone(),
                line_layout,
                TimeText,
            ));

            parent.spawn((
                Text::new("Speed: n/a"),
                line_font.clone(),
                line_layout,
                SimulationSpeedText,
            ));

            parent.spawn((
                Text::new("True Position: n/a"),
                line_font.clone(),
                line_layout,
                TruePositionText,
            ));

            parent.spawn((
                Text::new("Estimated Position: n/a"),
                line_font.clone(),
                line_layout,
                EstimatedPositionText,
            ));

            parent.spawn(Node {
                height: Val::Px(14.0),
                ..default()
            });

            parent.spawn((
                Text::new("Flags: n/a"),
                line_font.clone(),
                line_layout,
                FlagProgressText,
            ));

            parent.spawn((
                Text::new("Mapping Accuracy: n/a\nFree Space Recall: n/a\nWall Recall: n/a"),
                line_font.clone(),
                line_layout,
                MappingMetricsText,
            ));

            parent.spawn(Node {
                height: Val::Px(14.0),
                ..default()
            });

            parent.spawn((
                Text::new("+/-: Zoom In/Out"),
                line_font.clone(),
                line_layout,
            ));
            parent.spawn((
                Text::new("Shift+Drag: Pan Camera"),
                line_font.clone(),
                line_layout,
            ));

            parent.spawn(Node {
                height: Val::Px(14.0),
                ..default()
            });

            if config.teleop {
                parent.spawn((
                    Text::new("Arrows/WASD: Drive Agent"),
                    line_font.clone(),
                    line_layout,
                ));
                parent.spawn((
                    Text::new("Space: Pickup/Drop Flag"),
                    line_font.clone(),
                    line_layout,
                ));
            } else {
                parent.spawn((
                    Text::new("Space: Pause/Play"),
                    line_font.clone(),
                    line_layout,
                ));
                parent.spawn((
                    Text::new("[/]: Change Speed"),
                    line_font.clone(),
                    line_layout,
                ));
            }

            parent.spawn(Node {
                height: Val::Px(14.0),
                ..default()
            });

            parent.spawn((
                Text::new("C: Toggle Computed Occupancy Grid"),
                line_font.clone(),
                line_layout,
            ));
            parent.spawn((
                Text::new("T: Toggle True Occupancy Grid"),
                line_font,
                line_layout,
            ));
        });
}

pub fn update_time(mut query: Query<&mut Text, With<TimeText>>, time: Res<Time>) {
    for mut text in query.iter_mut() {
        text.0 = format!("Time: {:.2}s", time.elapsed_secs());
    }
}

/// Reports how fast simulated time is advancing against the wall clock, and what the policy costs.
///
/// The simulation waits for `get_action` rather than skipping ahead, so a slow policy shows up as
/// the whole game running in slow motion. Without this the only symptom is an agent that seems to
/// crawl, which reads as a bug in the agent.
pub fn update_simulation_speed(
    real_time: Res<Time<Real>>,
    simulated_time: Res<Time<Fixed>>,
    policy_cost: Option<Res<PolicyCost>>,
    mut previous_simulated: Local<Option<f32>>,
    mut window: Local<(f32, f32)>,
    mut query: Query<&mut Text, With<SimulationSpeedText>>,
) {
    /// Long enough for the ratio to settle, and to keep the number readable rather than flickering.
    const WINDOW_SECONDS: f32 = 0.5;

    let now = simulated_time.elapsed_secs();
    let Some(previous) = previous_simulated.replace(now) else {
        return;
    };

    // Totals over a window rather than an average of per-frame ratios: a frame that runs no fixed
    // tick and one that runs two are not equally weighted samples, so averaging their ratios reads
    // low even when the simulation is keeping up.
    let (simulated_elapsed, real_elapsed) = &mut *window;
    *simulated_elapsed += now - previous;
    *real_elapsed += real_time.delta_secs();
    if *real_elapsed < WINDOW_SECONDS {
        return;
    }

    let speed = format!("Speed: {:.2}x", *simulated_elapsed / *real_elapsed);
    *window = (0.0, 0.0);

    let line = match policy_cost.map(|cost| cost.smoothed_seconds) {
        Some(seconds) => format!("{speed} (policy {:.1}ms)", seconds * 1000.0),
        None => speed,
    };

    for mut text in query.iter_mut() {
        text.0 = line.clone();
    }
}

pub fn update_flag_progress(
    mut query: Query<&mut Text, With<FlagProgressText>>,
    captures: Res<FlagCaptureCounts>,
    flags: Query<&Flag>,
) {
    let total = flags.iter().count();
    for mut text in &mut query {
        text.0 = format!("Flags: {}/{total}", captures.0);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MappingMetrics {
    pub correct_cells: u32,
    pub total_cells: u32,
    pub free_correct_cells: u32,
    pub free_cells: u32,
    pub wall_correct_cells: u32,
    pub wall_cells: u32,
}

impl MappingMetrics {
    pub fn accuracy(self) -> f32 {
        ratio(self.correct_cells, self.total_cells)
    }

    pub fn free_recall(self) -> f32 {
        ratio(self.free_correct_cells, self.free_cells)
    }

    pub fn wall_recall(self) -> f32 {
        ratio(self.wall_correct_cells, self.wall_cells)
    }
}

fn ratio(numerator: u32, denominator: u32) -> f32 {
    numerator as f32 / denominator.max(1) as f32
}

fn mapping_metrics_from_assignments(
    assignments: impl IntoIterator<Item = (Option<EntityType>, Option<EntityType>)>,
) -> MappingMetrics {
    let mut metrics = MappingMetrics::default();

    for (prediction, truth) in assignments {
        let Some(truth) = truth else {
            continue;
        };
        if matches!(truth, EntityType::Flag | EntityType::CapturePoint) {
            continue;
        }

        metrics.total_cells += 1;
        if prediction == Some(truth) {
            metrics.correct_cells += 1;
        }

        match truth {
            EntityType::Free => {
                metrics.free_cells += 1;
                if prediction == Some(EntityType::Free) {
                    metrics.free_correct_cells += 1;
                }
            }
            EntityType::Wall => {
                metrics.wall_cells += 1;
                if prediction == Some(EntityType::Wall) {
                    metrics.wall_correct_cells += 1;
                }
            }
            EntityType::Flag | EntityType::CapturePoint | EntityType::Unknown => {}
        }
    }

    metrics
}

pub fn mapping_metrics(player_grid: &PlayerGrid, true_grid: &TrueGrid) -> MappingMetrics {
    Python::attach(|py| {
        let player_grid = player_grid.0.read().unwrap();
        let true_grid = true_grid.0.read().unwrap();
        let player_grid = player_grid.borrow(py);
        let true_grid = true_grid.borrow(py);
        mapping_metrics_from_assignments(
            player_grid
                .grid
                .iter()
                .zip(true_grid.grid.iter())
                .map(|(prediction, truth)| (prediction.assignment, truth.assignment)),
        )
    })
}

pub fn update_mapping_metrics(
    player_grid: Res<PlayerGrid>,
    true_grid: Res<TrueGrid>,
    mut query: Query<&mut Text, With<MappingMetricsText>>,
) {
    let metrics = mapping_metrics(&player_grid, &true_grid);

    for mut text in query.iter_mut() {
        text.0 = format!(
            "Mapping Accuracy: {:.1}%\nFree Space Recall: {:.1}%\nWall Recall: {:.1}%",
            metrics.accuracy() * 100.0,
            metrics.free_recall() * 100.0,
            metrics.wall_recall() * 100.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::mapping_metrics_from_assignments;
    use crate::python::game_state::EntityType;

    #[test]
    fn mapping_metrics_track_overall_accuracy_and_per_class_recall() {
        let metrics = mapping_metrics_from_assignments([
            (Some(EntityType::Free), Some(EntityType::Free)),
            (Some(EntityType::Wall), Some(EntityType::Free)),
            (Some(EntityType::Wall), Some(EntityType::Wall)),
            (None, Some(EntityType::Wall)),
            (Some(EntityType::Free), Some(EntityType::Flag)),
            (Some(EntityType::Wall), Some(EntityType::CapturePoint)),
        ]);

        assert_eq!(metrics.correct_cells, 2);
        assert_eq!(metrics.total_cells, 4);
        assert_eq!(metrics.free_correct_cells, 1);
        assert_eq!(metrics.free_cells, 2);
        assert_eq!(metrics.wall_correct_cells, 1);
        assert_eq!(metrics.wall_cells, 2);
        assert_eq!(metrics.accuracy(), 0.5);
        assert_eq!(metrics.free_recall(), 0.5);
        assert_eq!(metrics.wall_recall(), 0.5);
    }

    #[test]
    fn all_free_prediction_has_no_wall_recall() {
        let metrics = mapping_metrics_from_assignments([
            (Some(EntityType::Free), Some(EntityType::Free)),
            (Some(EntityType::Free), Some(EntityType::Wall)),
        ]);

        assert_eq!(metrics.free_recall(), 1.0);
        assert_eq!(metrics.wall_recall(), 0.0);
    }
}

pub fn update_true_position(
    mut query: Query<&mut Text, With<TruePositionText>>,
    agent_transform: Query<&Transform, With<Agent>>,
) {
    let Ok(agent_transform) = agent_transform.single() else {
        return;
    };

    for mut text in query.iter_mut() {
        text.0 = format!(
            "True Position: ({:.2}, {:.2})",
            agent_transform.translation.x, agent_transform.translation.z
        );
    }
}

pub fn spawn_walls(
    mut commands: Commands,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    true_grid: ResMut<TrueGrid>,
    graphics: Option<Res<WallGraphicsAssets>>,
    config: Res<MazeConfig>,
) {
    let seed = config
        .maze_generation
        .seed
        .expect("Should have generated a seed before the map generation");
    let mut generator = RbGenerator::new({
        let mut arr = [0u8; 32];
        arr[..4].copy_from_slice(&seed.to_le_bytes());
        Some(arr)
    });

    let (maze_columns, maze_rows) = config.maze_generation.maze_dimensions();
    let maze = generator
        .generate(maze_columns, maze_rows)
        .expect("Maze generation failed");

    let segments = segments_from_maze(&maze, &config, WALL_THICKNESS * 0.5);

    Python::attach(|py| {
        let grid = true_grid.0.write().unwrap();
        let mut py_obj = grid.borrow_mut(py);
        let columns = py_obj.columns as u32;
        let rows = py_obj.rows as u32;
        for index in 0..(columns * rows) {
            py_obj.grid[index as usize].assignment = Some(EntityType::Free);
            py_obj.grid[index as usize].logit_free = LOGIT_CLAMP;
            py_obj.grid[index as usize].logit_wall = -LOGIT_CLAMP;
            py_obj.grid[index as usize].logit_flag = -LOGIT_CLAMP;
            py_obj.grid[index as usize].logit_capture_point = -LOGIT_CLAMP;
        }
    });

    commands.insert_resource(WallSegments(segments.clone()));
    for (p0, p1) in segments {
        let mut entity = commands.spawn(WallBundle::new(p0, p1, WALL_THICKNESS));

        let aabb_bottom_left = Vec2::new(
            p0.x.min(p1.x) - WALL_THICKNESS * 0.5,
            p0.y.min(p1.y) - WALL_THICKNESS * 0.5,
        );

        let aabb_top_right = Vec2::new(
            p0.x.max(p1.x) + WALL_THICKNESS * 0.5,
            p0.y.max(p1.y) + WALL_THICKNESS * 0.5,
        );

        let wall_indexes = overlapping_indexes(
            aabb_bottom_left,
            aabb_top_right,
            config.agent.occupancy_grid_cell_size,
            config.maze_generation.world_width,
            config.maze_generation.world_height,
        );

        Python::attach(|py| {
            let grid = true_grid.0.write().unwrap();
            let mut py_obj = grid.borrow_mut(py);
            let columns = py_obj.columns as u32;

            for (ix, iy) in wall_indexes.iter().copied() {
                py_obj.grid[(ix + iy * columns) as usize].assignment = Some(EntityType::Wall);
                py_obj.grid[(ix + iy * columns) as usize].logit_free = -LOGIT_CLAMP;
                py_obj.grid[(ix + iy * columns) as usize].logit_wall = LOGIT_CLAMP;
                py_obj.grid[(ix + iy * columns) as usize].logit_flag = -LOGIT_CLAMP;
                py_obj.grid[(ix + iy * columns) as usize].logit_capture_point = -LOGIT_CLAMP;
            }
        });

        if let (Some(meshes), Some(graphics)) = (&mut meshes, &graphics) {
            let len = p0.distance(p1);
            let mesh = meshes.add(Cuboid::new(len, WALL_HEIGHT, WALL_THICKNESS));
            entity.insert((Mesh3d(mesh), MeshMaterial3d(graphics.material.clone())));
        }
    }
}
