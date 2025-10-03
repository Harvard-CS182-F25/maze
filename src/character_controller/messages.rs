use avian3d::math::*;
use bevy::prelude::*;

#[derive(Message)]
pub enum MovementMessage {
    TranslateById(u32, Vector2),
    RotateById(u32, Scalar),
}
