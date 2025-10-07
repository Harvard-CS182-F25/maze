use avian3d::prelude::*;
use bevy::prelude::*;
use maze_generator::prelude::*;
use maze_generator::recursive_backtracking::RbGenerator;
use pyo3::prelude::*;

use crate::{
    agent::COLLISION_LAYER_AGENT,
    core::MazeConfig,
    occupancy_grid::TrueGrid,
    python::game_state::EntityType,
    scene::{COLLISION_LAYER_WALL, WALL_HEIGHT, WALL_THICKNESS, WallBundle, WallGraphicsAssets},
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

pub fn segments_from_maze(maze: &Maze, cell: f32, pad: f32) -> Vec<(Vec2, Vec2)> {
    let (w, h) = maze.size; // i32
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
    aabb_bottom_left: Vec2,
    aabb_top_right: Vec2,
    cell_size: f32,
    width: usize,
    height: usize,
) -> Vec<(u32, u32)> {
    let min_x = (aabb_bottom_left.x / cell_size).floor().max(0.0) as u32;
    let max_x = (aabb_top_right.x / cell_size).ceil().min(width as f32) as u32;
    let min_y = (aabb_bottom_left.y / cell_size).floor().max(0.0) as u32;
    let max_y = (aabb_top_right.y / cell_size).ceil().min(height as f32) as u32;

    let mut indexes = Vec::new();
    for x in min_x..max_x {
        for y in min_y..max_y {
            indexes.push((x, y));
        }
    }

    indexes
}

pub fn spawn_walls(
    mut commands: Commands,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    true_grid: ResMut<TrueGrid>,
    graphics: Option<Res<WallGraphicsAssets>>,
    config: Res<MazeConfig>,
) {
    let mut generator = RbGenerator::new(config.maze_generation.seed.map(|s| {
        let mut arr = [0u8; 32];
        arr[..4].copy_from_slice(&s.to_le_bytes());
        arr
    }));

    let maze = generator
        .generate(
            (config.maze_generation.width / config.maze_generation.cell_size).round() as i32,
            (config.maze_generation.height / config.maze_generation.cell_size).round() as i32,
        )
        .expect("Maze generation failed");

    let segments = segments_from_maze(
        &maze,
        config.maze_generation.cell_size,
        WALL_THICKNESS * 0.5,
    );

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

        let indexes = overlapping_indexes(
            aabb_bottom_left + Vec2::splat(config.maze_generation.width * 0.5),
            aabb_top_right + Vec2::splat(config.maze_generation.height * 0.5),
            config.agent.occupancy_grid_cell_size,
            (config.maze_generation.width / config.agent.occupancy_grid_cell_size).round() as usize,
            (config.maze_generation.height / config.agent.occupancy_grid_cell_size).round()
                as usize,
        );

        Python::attach(|py| {
            let grid = true_grid.0.write().unwrap();
            let mut py_obj = grid.borrow_mut(py);
            let width = py_obj.width as u32;
            let height = py_obj.height as u32;

            for index in 0..(width * height) {
                py_obj.grid[index as usize].assignment = Some(EntityType::Empty());
                py_obj.grid[index as usize].p_free = 1.0;
            }

            for (ix, iy) in indexes.iter().copied() {
                py_obj.grid[(ix + iy * width) as usize].assignment = Some(EntityType::Wall());
                py_obj.grid[(ix + iy * width) as usize].p_free = 0.0;
                py_obj.grid[(ix + iy * width) as usize].p_wall = 1.0;
            }
        });

        if let (Some(meshes), Some(graphics)) = (&mut meshes, &graphics) {
            let len = p0.distance(p1);
            let mesh = meshes.add(Cuboid::new(len, WALL_HEIGHT, WALL_THICKNESS));
            entity.insert((Mesh3d(mesh), MeshMaterial3d(graphics.material.clone())));
        }
    }
}
