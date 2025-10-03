use bevy::prelude::*;

use crate::core::MazeConfig;

pub fn setup_camera(mut commands: Commands, config: Res<MazeConfig>) {
    if config.headless {
        return;
    }

    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::new(0.0, 10.0, 0.0)).looking_at(Vec3::ZERO, Vec3::NEG_Z),
        Projection::from(OrthographicProjection {
            scale: config.camera.scale,
            ..OrthographicProjection::default_3d()
        }),
    ));
}
