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

pub fn zoom_in(mut query: Query<&mut Projection, With<Camera3d>>) {
    for mut proj in query.iter_mut() {
        if let Projection::Orthographic(ortho) = &mut *proj {
            ortho.scale += 0.001;
        }
    }
}

pub fn zoom_out(mut query: Query<&mut Projection, With<Camera3d>>) {
    for mut proj in query.iter_mut() {
        if let Projection::Orthographic(ortho) = &mut *proj {
            ortho.scale -= 0.001;
        }
    }
}

pub fn pan_camera(
    mut query: Query<&mut Transform, With<Camera3d>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let mut direction = Vec3::ZERO;
    if keyboard_input.pressed(KeyCode::ArrowUp) {
        direction.z -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::ArrowDown) {
        direction.z += 1.0;
    }
    if keyboard_input.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keyboard_input.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }
    for mut transform in query.iter_mut() {
        transform.translation += direction;
    }
}
