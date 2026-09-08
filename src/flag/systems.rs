use bevy::prelude::*;
use pyo3::prelude::*;
use rand::{SeedableRng, seq::SliceRandom};
use rand_chacha::ChaCha20Rng;

use crate::core::MazeConfig;
use crate::flag::{CapturePoint, CapturePointBundle, Flag};
use crate::interaction_range::InteractionRadius;
use crate::occupancy_grid::{OccupancyGrid, OccupancyGridCellData, TrueGrid};
use crate::python::game_state::EntityType;
use crate::scene::WallCells;

use super::components::FlagBundle;
use super::visual::{CapturePointGraphicsAssets, FlagGraphicsAssets};

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

/// Picks positions with `flags.spawn_clearance()` of clearance from:
/// - walls
/// - existing flags/capture points
/// - newly selected same-type items (to avoid clumping)
///
fn pick_positions_for(
    py_grid: &mut OccupancyGrid,
    config: &MazeConfig,
    rng: &mut ChaCha20Rng,
    count: usize,
    place_as: EntityType,
) -> Vec<(f32, f32)> {
    let w = py_grid.columns as i32;
    let h = py_grid.rows as i32;
    let n = (w * h) as usize;
    let cell_size = config.agent.occupancy_grid_cell_size;
    let clearance = config.flags.spawn_clearance();

    let mut blocked = vec![false; n];

    for (i, assignment) in py_grid.assignments().iter().enumerate() {
        match assignment {
            Some(EntityType::Wall) => {
                mark_neighborhood_units(&mut blocked, i, w, h, cell_size, clearance);
            }
            Some(EntityType::Flag) | Some(EntityType::CapturePoint) => {
                mark_neighborhood_units(&mut blocked, i, w, h, cell_size, clearance);
            }
            _ => {}
        }
    }

    let mut candidates: Vec<usize> = py_grid
        .assignments()
        .iter()
        .enumerate()
        .filter(|(i, assignment)| **assignment == Some(EntityType::Free) && !blocked[*i])
        .map(|(i, _)| i)
        .collect();

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
        mark_neighborhood_units(&mut blocked, idx, w, h, cell_size, clearance);
    }

    if picked.len() < count {
        warn!(
            "Could only place {} of {} {:?} with {:.1}u clearance.",
            picked.len(),
            count,
            place_as,
            clearance
        );
    }

    let mut out = Vec::with_capacity(picked.len());
    for &i in &picked {
        py_grid.set_assignment(i, Some(place_as));
        let (column, row) = (i % py_grid.columns, i / py_grid.columns);
        out.extend(py_grid.world_center(column, row));
    }

    out
}

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
            config.flags.flag_count,
            EntityType::Flag,
        )
    });

    for (i, &(x, y)) in positions.iter().enumerate() {
        let flag_name = format!("Flag {}", i + 1);
        info!("Spawning flag at position: ({x:.2}, {y:.2})");

        let mut entity = commands.spawn(FlagBundle::new(
            &flag_name,
            Vec3::new(x, 0.5, y),
            config.flags.flag_radius,
        ));

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

        // Flags are already in the grid by now, so capture points keep their clearance from
        // those too.
        pick_positions_for(
            &mut py_obj,
            &config,
            &mut rng,
            config.flags.capture_point_count,
            EntityType::CapturePoint,
        )
    });

    for (i, &(x, y)) in positions.iter().enumerate() {
        let name = format!("Capture Point {}", i + 1);
        info!("Spawning capture point at position: ({x:.2}, {y:.2})");

        let mut entity = commands.spawn(CapturePointBundle::new(
            &name,
            Vec3::new(x, 0.5, y),
            config.flags.capture_point_radius,
        ));

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
    wall_cells: Res<WallCells>,
    // `InteractionRadius` is dropped the moment a flag or capture point stops being usable: a
    // carried or captured flag, and a capture point that has already taken its one flag. Those
    // must not appear in the map, or an agent will keep routing to a delivery that cannot happen.
    query_flag: Query<&GlobalTransform, (With<Flag>, With<InteractionRadius>)>,
    query_cp: Query<&GlobalTransform, (With<CapturePoint>, With<InteractionRadius>)>,
) {
    let footprint = |transform: &GlobalTransform| {
        let position = transform.translation();
        (
            Vec2::new(position.x - 0.5, position.z - 0.5),
            Vec2::new(position.x + 0.5, position.z + 0.5),
        )
    };
    let flags: Vec<_> = query_flag.iter().map(footprint).collect();
    let capture_points: Vec<_> = query_cp.iter().map(footprint).collect();

    // One attach for the whole rebuild. Taking the GIL once per entity made this the most
    // expensive part of a tick, which is why it used to be skipped in headless runs entirely.
    Python::attach(|py| {
        let grid = true_grid.0.write().unwrap();
        let mut py_obj = grid.borrow_mut(py);
        let columns = py_obj.columns;

        // Rebuilt rather than patched: `use_true_map` hands this very grid to the policy, so a
        // class the policy wrote has to be overwritten rather than left standing.
        py_obj.fill(OccupancyGridCellData::known(EntityType::Free));
        for &index in &wall_cells.0 {
            py_obj.set_cell(index, OccupancyGridCellData::known(EntityType::Wall));
        }

        // Capture points last, so one holding a flag reads as a capture point rather than a flag.
        for (kind, footprints) in [
            (EntityType::Flag, &flags),
            (EntityType::CapturePoint, &capture_points),
        ] {
            let cell = OccupancyGridCellData::known(kind);
            for (aabb_min, aabb_max) in footprints {
                for (column, row) in py_obj.overlapping_cells(*aabb_min, *aabb_max) {
                    py_obj.set_cell(column as usize + row as usize * columns, cell);
                }
            }
        }
    });
}
