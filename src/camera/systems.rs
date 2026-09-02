use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

use crate::{
    camera::{CameraZoomLimits, fit_scale},
    core::MazeConfig,
};

pub fn setup_camera(
    mut commands: Commands,
    config: Res<MazeConfig>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if config.headless {
        return;
    }

    let board_size = Vec2::new(
        config.maze_generation.world_width,
        config.maze_generation.world_height,
    );
    let scale = windows
        .single()
        .map(|window| -fit_scale(board_size, Vec2::new(window.width(), window.height())))
        .unwrap_or(-0.15);
    if let Ok(window) = windows.single() {
        commands.insert_resource(CameraZoomLimits::new(
            board_size,
            config.maze_generation.cell_size,
            Vec2::new(window.width(), window.height()),
            scale,
        ));
    }

    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(Vec3::new(0.0, 10.0, 0.0)).looking_at(Vec3::ZERO, Vec3::NEG_Z),
        Projection::from(OrthographicProjection {
            scale,
            ..OrthographicProjection::default_3d()
        }),
    ));
}

/// Zooms in by 3%, bounded so a maze cell cannot grow arbitrarily large.
pub fn zoom_in(limits: Res<CameraZoomLimits>, mut query: Query<&mut Projection, With<Camera3d>>) {
    for mut proj in query.iter_mut() {
        if let Projection::Orthographic(ortho) = &mut *proj {
            let scale = ortho.scale.abs();
            if scale > limits.min_scale {
                ortho.scale = ortho.scale.signum() * (scale * 0.97).max(limits.min_scale);
            }
        }
    }
}

/// Zooms out by 3%, bounded relative to the maze and current viewport.
pub fn zoom_out(limits: Res<CameraZoomLimits>, mut query: Query<&mut Projection, With<Camera3d>>) {
    for mut proj in query.iter_mut() {
        if let Projection::Orthographic(ortho) = &mut *proj {
            let scale = ortho.scale.abs();
            if scale < limits.max_scale {
                ortho.scale = ortho.scale.signum() * (scale / 0.97).min(limits.max_scale);
            }
        }
    }
}

/// Tracks the last cursor position while a Shift+Left-click drag is active.
#[derive(Resource, Default)]
pub struct PanState {
    last_cursor: Option<Vec2>,
}

fn shift_held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

fn drag_delta(pan_state: &mut PanState, dragging: bool, cursor: Option<Vec2>) -> Vec2 {
    let delta = if dragging {
        match (pan_state.last_cursor, cursor) {
            (Some(last), Some(current)) => current - last,
            _ => Vec2::ZERO,
        }
    } else {
        Vec2::ZERO
    };
    pan_state.last_cursor = if dragging { cursor } else { None };
    delta
}

/// Uses an open/closed-hand cursor while Shift enables camera panning.
pub fn update_pan_cursor(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut current: Local<Option<SystemCursorIcon>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let desired = shift_held(&keys).then(|| {
        if mouse_buttons.pressed(MouseButton::Left) {
            SystemCursorIcon::Grabbing
        } else {
            SystemCursorIcon::Grab
        }
    });
    if desired == *current {
        return;
    }
    *current = desired;

    match desired {
        Some(icon) => commands.entity(window).insert(CursorIcon::System(icon)),
        None => commands.entity(window).remove::<CursorIcon>(),
    };
}

/// Shift+Left-click drag pans the top-down camera. Arrow keys remain available
/// for teleop movement.
pub fn pan_camera(
    mut query: Query<(&mut Transform, &Projection), With<Camera3d>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    config: Res<MazeConfig>,
    mut pan_state: ResMut<PanState>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let dragging = shift_held(&keys) && mouse_buttons.pressed(MouseButton::Left);
    let delta = drag_delta(&mut pan_state, dragging, window.cursor_position());
    let board_size = Vec2::new(
        config.maze_generation.world_width,
        config.maze_generation.world_height,
    );
    let window_size = Vec2::new(window.width(), window.height());
    let padding = config.maze_generation.cell_size * 2.0;

    for (mut transform, projection) in &mut query {
        let Projection::Orthographic(ortho) = projection else {
            continue;
        };
        transform.translation += Vec3::new(delta.x * ortho.scale, 0.0, delta.y * ortho.scale);

        // Keep a small, cell-scaled margin beyond the board edge instead of
        // letting the world scroll away completely.
        let half_view = window_size * ortho.scale.abs() / 2.0;
        let max = (board_size / 2.0 - half_view + Vec2::splat(padding)).max(Vec2::ZERO);
        transform.translation.x = transform.translation.x.clamp(-max.x, max.x);
        transform.translation.z = transform.translation.z.clamp(-max.y, max.y);
    }
}
