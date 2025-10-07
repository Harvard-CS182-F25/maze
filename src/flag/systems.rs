use bevy::prelude::*;

use crate::core::MazeConfig;
use crate::flag::CapturePointBundle;

use super::components::FlagBundle;
use super::visual::{CapturePointGraphicsAssets, FlagGraphicsAssets};

pub fn spawn_flags(
    mut commands: Commands,
    flag_graphics: Option<Res<FlagGraphicsAssets>>,
    config: Res<MazeConfig>,
) {
    for (i, &position) in config.flags.positions.iter().enumerate() {
        info!("Spawning flag at position: {:?}", position);
        let flag_name = format!("Flag {}", i + 1);

        let mut entity = commands.spawn(FlagBundle::new(
            &flag_name,
            Vec3::new(position.0, 0.5, position.1),
        ));

        if let Some(flag_graphics) = &flag_graphics {
            entity.insert((
                Mesh3d(flag_graphics.mesh.clone()),
                MeshMaterial3d(flag_graphics.blue_material.clone()),
            ));
        }
    }
}

pub fn spawn_capture_points(
    mut commands: Commands,
    capture_point_graphics: Option<Res<CapturePointGraphicsAssets>>,
    config: Res<MazeConfig>,
) {
    for (i, &position) in config.capture_points.positions.iter().enumerate() {
        info!("Spawning capture point at position: {:?}", position);
        let name = format!("Capture Point {}", i + 1);

        let mut entity = commands.spawn(CapturePointBundle::new(
            &name,
            Vec3::new(position.0, 0.5, position.1),
        ));

        if let Some(capture_point_graphics) = &capture_point_graphics {
            entity.insert((
                Mesh3d(capture_point_graphics.mesh.clone()),
                MeshMaterial3d(capture_point_graphics.blue_material.clone()),
            ));
        }
    }
}
