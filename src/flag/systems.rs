use bevy::prelude::*;
use pyo3::prelude::*;
use rand::{SeedableRng, seq::SliceRandom};
use rand_chacha::ChaCha20Rng;

use crate::core::MazeConfig;
use crate::flag::{CapturePoint, CapturePointBundle, FLAG_INTERACTION_RADIUS, Flag};
use crate::occupancy_grid::{LOGIT_CLAMP, OccupancyGrid, TrueGrid};
use crate::python::game_state::EntityType;
use crate::scene::{WALL_THICKNESS, WallSegments};

use super::components::FlagBundle;
use super::visual::{CapturePointGraphicsAssets, FlagGraphicsAssets};

const MIN_CLEARANCE_UNITS: f32 = FLAG_INTERACTION_RADIUS;

fn idx_to_rc(i: usize, width: usize) -> (i32, i32) {
    let r = (i / width) as i32;
    let c = (i % width) as i32;
    (r, c)
}

fn rc_to_idx(r: i32, c: i32, width: i32, height: i32) -> Option<usize> {
    if r < 0 || c < 0 || r >= height || c >= width {
        return None;
    }
    Some((r as usize) * (width as usize) + (c as usize))
}

fn mark_neighborhood_units(
    blocked: &mut [bool],
    idx: usize,
    w: i32,
    h: i32,
    cell_size: f32,
    min_clear_units: f32,
) {
    let (r0, c0) = idx_to_rc(idx, w as usize);
    let rad_cells: i32 = (min_clear_units / cell_size).ceil() as i32;
    let min_sq = min_clear_units * min_clear_units;

    for dr in -rad_cells..=rad_cells {
        let rr = r0 + dr;
        if rr < 0 || rr >= h {
            continue;
        }
        for dc in -rad_cells..=rad_cells {
            let cc = c0 + dc;
            if cc < 0 || cc >= w {
                continue;
            }
            let dx = (dc as f32) * cell_size;
            let dy = (dr as f32) * cell_size;
            if dx * dx + dy * dy <= min_sq
                && let Some(nidx) = rc_to_idx(rr, cc, w, h)
            {
                blocked[nidx] = true;
            }
        }
    }
}

fn grid_to_world_xy(col: u32, row: u32, cell_size: f32, world_w: f32, world_h: f32) -> (f32, f32) {
    let x = (col as f32) * cell_size + cell_size * 0.5 - world_w * 0.5;
    let y = (row as f32) * cell_size + cell_size * 0.5 - world_h * 0.5;
    (x, y)
}

/// Picks up to `count` indices for `place_as`, respecting 3.0-unit clearance from:
/// - walls
/// - existing flags/capture points
/// - newly selected same-type items (to avoid clumping)
///
/// Returns (index, (world_x, world_y)) for each picked cell and commits assignments.
fn pick_positions_for(
    py_grid: &mut OccupancyGrid,
    config: &MazeConfig,
    rng: &mut ChaCha20Rng,
    count: usize,
    place_as: EntityType, // Flag or CapturePoint
) -> Vec<(f32, f32)> {
    let w = py_grid.width as i32;
    let h = py_grid.height as i32;
    let n = (w * h) as usize;
    let cell_size = config.agent.occupancy_grid_cell_size;

    // 1) Start with everything unblocked.
    let mut blocked = vec![false; n];

    // 2) Block around walls and existing specials.
    for (i, cell) in py_grid.grid.iter().enumerate() {
        match cell.assignment {
            Some(EntityType::Wall) => {
                mark_neighborhood_units(&mut blocked, i, w, h, cell_size, MIN_CLEARANCE_UNITS);
            }
            Some(EntityType::Flag) | Some(EntityType::CapturePoint) => {
                mark_neighborhood_units(&mut blocked, i, w, h, cell_size, MIN_CLEARANCE_UNITS);
            }
            _ => {}
        }
    }

    // 3) Collect empty + not blocked candidates.
    let mut candidates: Vec<usize> = py_grid
        .grid
        .iter()
        .enumerate()
        .filter(|(i, cell)| cell.assignment == Some(EntityType::Empty) && !blocked[*i])
        .map(|(i, _)| i)
        .collect();

    // 4) Shuffle for randomness, then greedily pick; after each pick, block a 3.0-unit radius
    //    around the new item so the next picks stay spaced out.
    candidates.shuffle(rng);

    let mut picked: Vec<usize> = Vec::with_capacity(count);
    for &idx in &candidates {
        if picked.len() >= count {
            break;
        }
        if blocked[idx] {
            continue;
        }
        picked.push(idx);
        mark_neighborhood_units(&mut blocked, idx, w, h, cell_size, MIN_CLEARANCE_UNITS);
    }

    if picked.len() < count {
        warn!(
            "Could only place {} of {} {:?} with {:.1}u clearance.",
            picked.len(),
            count,
            place_as,
            MIN_CLEARANCE_UNITS
        );
    }

    // 5) Commit to grid and prepare world positions.
    let world_w = config.maze_generation.width;
    let world_h = config.maze_generation.height;

    let mut out = Vec::with_capacity(picked.len());
    for &i in &picked {
        py_grid.grid[i].assignment = Some(place_as);
        let col = (i as u32) % (py_grid.width as u32);
        let row = (i as u32) / (py_grid.width as u32);
        out.push(grid_to_world_xy(col, row, cell_size, world_w, world_h));
    }

    out
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

// --------------------- spawners ---------------------

pub fn spawn_flags(
    mut commands: Commands,
    flag_graphics: Option<Res<FlagGraphicsAssets>>,
    config: Res<MazeConfig>,
    true_grid: ResMut<TrueGrid>,
) {
    let positions = Python::attach(|py| {
        let grid = true_grid.0.write().unwrap();
        let mut py_obj = grid.borrow_mut(py);

        let mut rng = ChaCha20Rng::from_seed({
            let mut arr = [0u8; 32];
            let seed = config
                .maze_generation
                .seed
                .expect("Seed must be set before map generation");
            arr[..4].copy_from_slice(&seed.to_le_bytes());
            arr
        });

        pick_positions_for(
            &mut py_obj,
            &config,
            &mut rng,
            config.flags.number,
            EntityType::Flag,
        )
    });

    for (i, &(x, y)) in positions.iter().enumerate() {
        let flag_name = format!("Flag {}", i + 1);
        info!("Spawning flag at position: ({x:.2}, {y:.2})");

        let mut entity = commands.spawn(FlagBundle::new(&flag_name, Vec3::new(x, 0.5, y)));

        if let Some(flag_graphics) = &flag_graphics {
            entity.insert((
                Mesh3d(flag_graphics.mesh.clone()),
                MeshMaterial3d(flag_graphics.material.clone()),
            ));
        }
    }
}

pub fn spawn_capture_points(
    mut commands: Commands,
    capture_point_graphics: Option<Res<CapturePointGraphicsAssets>>,
    config: Res<MazeConfig>,
    true_grid: ResMut<TrueGrid>,
) {
    let positions = Python::attach(|py| {
        let grid = true_grid.0.write().unwrap();
        let mut py_obj = grid.borrow_mut(py);

        let mut rng = ChaCha20Rng::from_seed({
            let mut arr = [0u8; 32];
            let seed = config
                .maze_generation
                .seed
                .expect("Seed must be set before map generation");
            arr[..4].copy_from_slice(&seed.to_le_bytes());
            arr
        });

        // Note: existing Flags are already in the grid now, so capture points will
        // also keep 3u away from them.
        pick_positions_for(
            &mut py_obj,
            &config,
            &mut rng,
            config.capture_points.number,
            EntityType::CapturePoint,
        )
    });

    for (i, &(x, y)) in positions.iter().enumerate() {
        let name = format!("Capture Point {}", i + 1);
        info!("Spawning capture point at position: ({x:.2}, {y:.2})");

        let mut entity = commands.spawn(CapturePointBundle::new(&name, Vec3::new(x, 0.5, y)));

        if let Some(capture_point_graphics) = &capture_point_graphics {
            entity.insert((
                Mesh3d(capture_point_graphics.mesh.clone()),
                MeshMaterial3d(capture_point_graphics.material.clone()),
            ));
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn update_true_grid(
    true_grid: ResMut<TrueGrid>,
    config: Res<MazeConfig>,
    segments: Res<WallSegments>,
    query_flag: Query<&GlobalTransform, With<Flag>>,
    query_cp: Query<&GlobalTransform, With<CapturePoint>>,
) {
    Python::attach(|py| {
        let grid = true_grid.0.write().unwrap();
        let mut py_obj = grid.borrow_mut(py);
        let width = py_obj.width as u32;

        for entry in &mut py_obj.grid {
            if entry.assignment != Some(EntityType::Wall) {
                entry.assignment = Some(EntityType::Empty);
                entry.logit_free = LOGIT_CLAMP;
                entry.logit_wall = -LOGIT_CLAMP;
                entry.logit_flag = -LOGIT_CLAMP;
                entry.logit_capture_point = -LOGIT_CLAMP;
            }
        }

        for (p0, p1) in &segments.0 {
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

            for (ix, iy) in wall_indexes.iter().copied() {
                py_obj.grid[(ix + iy * width) as usize].assignment = Some(EntityType::Wall);
                py_obj.grid[(ix + iy * width) as usize].logit_free = -LOGIT_CLAMP;
                py_obj.grid[(ix + iy * width) as usize].logit_wall = LOGIT_CLAMP;
                py_obj.grid[(ix + iy * width) as usize].logit_flag = -LOGIT_CLAMP;
                py_obj.grid[(ix + iy * width) as usize].logit_capture_point = -LOGIT_CLAMP;
            }
        }
    });

    for transform in &query_flag {
        let aabb_min = Vec2::new(
            transform.translation().x - 0.5,
            transform.translation().z - 0.5,
        );
        let aabb_max = Vec2::new(
            transform.translation().x + 0.5,
            transform.translation().z + 0.5,
        );
        let overlapping = overlapping_indexes(
            aabb_min,
            aabb_max,
            config.agent.occupancy_grid_cell_size,
            config.maze_generation.width,
            config.maze_generation.height,
        );

        Python::attach(|py| {
            let grid = true_grid.0.write().unwrap();
            let mut py_obj = grid.borrow_mut(py);

            for (col, row) in overlapping {
                let idx = (row * (py_obj.width as u32) + col) as usize;
                py_obj.grid[idx].assignment = Some(EntityType::Flag);
                py_obj.grid[idx].logit_free = -LOGIT_CLAMP;
                py_obj.grid[idx].logit_wall = -LOGIT_CLAMP;
                py_obj.grid[idx].logit_flag = LOGIT_CLAMP;
                py_obj.grid[idx].logit_capture_point = -LOGIT_CLAMP;
            }
        });
    }

    for transform in &query_cp {
        let aabb_min = Vec2::new(
            transform.translation().x - 0.5,
            transform.translation().z - 0.5,
        );
        let aabb_max = Vec2::new(
            transform.translation().x + 0.5,
            transform.translation().z + 0.5,
        );
        let overlapping = overlapping_indexes(
            aabb_min,
            aabb_max,
            config.agent.occupancy_grid_cell_size,
            config.maze_generation.width,
            config.maze_generation.height,
        );

        Python::attach(|py| {
            let grid = true_grid.0.write().unwrap();
            let mut py_obj = grid.borrow_mut(py);

            for (col, row) in overlapping {
                let idx = (row * (py_obj.width as u32) + col) as usize;
                py_obj.grid[idx].assignment = Some(EntityType::CapturePoint);
                py_obj.grid[idx].logit_free = -LOGIT_CLAMP;
                py_obj.grid[idx].logit_wall = -LOGIT_CLAMP;
                py_obj.grid[idx].logit_flag = -LOGIT_CLAMP;
                py_obj.grid[idx].logit_capture_point = LOGIT_CLAMP;
            }
        });
    }
}
