use avian3d::math::Vector2;
use bevy::prelude::*;

#[derive(Message)]
pub struct MovementMessage(pub Vector2);
