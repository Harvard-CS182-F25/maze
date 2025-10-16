use avian3d::prelude::*;
use bevy::prelude::*;
use maze_generator::prelude::*;
use maze_generator::recursive_backtracking::RbGenerator;
use pyo3::prelude::*;

use crate::{
    agent::{Agent, COLLISION_LAYER_AGENT},
    core::MazeConfig,
    occupancy_grid::{LOGIT_CLAMP, PlayerGrid, TrueGrid},
    python::game_state::EntityType,
    scene::{
        COLLISION_LAYER_WALL, EstimatedPositionText, MappingErrorText, TimeText, TruePositionText,
        WALL_HEIGHT, WALL_THICKNESS, WallBundle, WallGraphicsAssets, WallSegments,
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
        let material = materials.add(Color::srgb(0.0, 1.0, 0.0));
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
    // keep within outer bounds
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

    // outer bounds for clamping
    let xmin = x0;
    let xmax = x0 + (w as f32) * cell;
    let zmin = z0;
    let zmax = z0 + (h as f32) * cell;

    let mut segments = Vec::new();

    // top (north) border and left (west) border
    for c in 0..w {
        push_horizontal(&mut segments, x0, z0, cell, 0, c, pad, xmin, xmax);
    }
    for r in 0..h {
        push_vertical(&mut segments, x0, z0, cell, 0, r, pad, zmin, zmax);
    }

    // interior: add East/South walls where there is NO passage
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

fn overlapping_indexes(
    aabb_min: Vec2,    // bottom-left  (x,y)
    aabb_max: Vec2,    // top-right    (x,y)
    cell_size: f32,    // occupancy grid cell size (world units)
    world_width: f32,  // world extent in X
    world_height: f32, // world extent in Y (your Z)
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

pub fn spawn_seed_and_time(
    mut commands: Commands,
    mut config: ResMut<MazeConfig>,
    time: Res<Time>,
) {
    let seed = if let Some(seed) = config.maze_generation.seed {
        seed
    } else {
        let seed = rand::random::<u16>().into();
        config.maze_generation.seed = Some(seed);
        seed
    };

    info!("Using maze generation seed: {}", seed);

    if config.headless {
        return;
    }

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::Grid,
                top: Val::Px(5.0),
                left: Val::Px(5.0),
                padding: Val::Px(2.5).into(),
                justify_items: JustifyItems::Start,
                align_items: AlignItems::Start,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(format!("Time: {:.2}s", time.elapsed_secs())),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Right),
                TimeText,
            ));

            parent.spawn((
                Text::new(format! {"Seed: {}", seed}),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Right),
            ));

            parent.spawn((
                Text::new("True Agent Position:"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Right),
                TruePositionText,
            ));

            parent.spawn((
                Text::new("Estimated Agent Position: ()"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Right),
                EstimatedPositionText,
            ));

            parent.spawn((
                Text::new("Mapping Error:"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Right),
                MappingErrorText,
            ));
        });
}

pub fn update_time(mut query: Query<&mut Text, With<TimeText>>, time: Res<Time>) {
    for mut text in query.iter_mut() {
        text.0 = format!("Time: {:.2}s", time.elapsed_secs());
    }
}

pub fn update_mapping_error(
    player_grid: Res<PlayerGrid>,
    true_grid: Res<TrueGrid>,
    mut query: Query<&mut Text, With<MappingErrorText>>,
) {
    Python::attach(|py| {
        let player_grid = player_grid.0.read().unwrap();
        let true_grid = true_grid.0.read().unwrap();
        let player_grid = player_grid.borrow(py);
        let true_grid = true_grid.borrow(py);

        let mut error = 0;
        let mut total = 0;
        for (player_entry, true_entry) in player_grid.grid.iter().zip(true_grid.grid.iter()) {
            if let Some(true_entity_type) = true_entry.assignment
                && true_entity_type != EntityType::Flag
                && true_entity_type != EntityType::CapturePoint
            {
                total += 1;
                if player_entry.assignment != true_entry.assignment {
                    error += 1;
                }
            }
        }

        let error_rate = (error as f32) / total.max(1) as f32 * 100.0;

        for mut text in query.iter_mut() {
            text.0 = format!("Mapping Error: {error:.0}/{total} [{error_rate:.1}%]");
        }
    })
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
            "True Agent Position: ({:.2}, {:.2})",
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

    let maze = generator
        .generate(
            (config.maze_generation.width / config.maze_generation.cell_size).round() as i32,
            (config.maze_generation.height / config.maze_generation.cell_size).round() as i32,
        )
        .expect("Maze generation failed");

    let segments = segments_from_maze(&maze, &config, WALL_THICKNESS * 0.5);

    Python::attach(|py| {
        let grid = true_grid.0.write().unwrap();
        let mut py_obj = grid.borrow_mut(py);
        let width = py_obj.width as u32;
        let height = py_obj.height as u32;
        for index in 0..(width * height) {
            py_obj.grid[index as usize].assignment = Some(EntityType::Empty);
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
            config.maze_generation.width,
            config.maze_generation.height,
        );

        Python::attach(|py| {
            let grid = true_grid.0.write().unwrap();
            let mut py_obj = grid.borrow_mut(py);
            let width = py_obj.width as u32;

            for (ix, iy) in wall_indexes.iter().copied() {
                py_obj.grid[(ix + iy * width) as usize].assignment = Some(EntityType::Wall);
                py_obj.grid[(ix + iy * width) as usize].logit_free = -LOGIT_CLAMP;
                py_obj.grid[(ix + iy * width) as usize].logit_wall = LOGIT_CLAMP;
                py_obj.grid[(ix + iy * width) as usize].logit_flag = -LOGIT_CLAMP;
                py_obj.grid[(ix + iy * width) as usize].logit_capture_point = -LOGIT_CLAMP;
            }
        });

        if let (Some(meshes), Some(graphics)) = (&mut meshes, &graphics) {
            let len = p0.distance(p1);
            let mesh = meshes.add(Cuboid::new(len, WALL_HEIGHT, WALL_THICKNESS));
            entity.insert((Mesh3d(mesh), MeshMaterial3d(graphics.material.clone())));
        }
    }
}
