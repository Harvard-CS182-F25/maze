use avian3d::math::*;
use bevy::prelude::*;

#[derive(Message)]
#[allow(dead_code)]
pub enum MovementMessage {
    TranslateById(u32, Vector2),
    RotateById(u32, Scalar),
}
