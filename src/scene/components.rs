use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    agent::COLLISION_LAYER_AGENT,
    scene::{COLLISION_LAYER_WALL, WALL_HEIGHT},
};

#[derive(Debug, Clone, Copy, Default, Component, Reflect)]
#[reflect(Component)]
pub struct Wall;

#[derive(Debug, Clone, Default, Resource)]
pub struct WallSegments(pub Vec<(Vec2, Vec2)>);

#[derive(Debug, Clone, Copy, Default, Component, Reflect)]
#[reflect(Component)]
pub struct TimeText;

#[derive(Debug, Clone, Copy, Default, Component, Reflect)]
#[reflect(Component)]
pub struct TruePositionText;

#[derive(Debug, Clone, Copy, Default, Component, Reflect)]
#[reflect(Component)]
pub struct EstimatedPositionText;

#[derive(Debug, Clone, Copy, Default, Component, Reflect)]
#[reflect(Component)]
pub struct MappingErrorText;

#[derive(Debug, Clone, Bundle, Default)]
pub struct WallBundle {
    pub wall: Wall,
    pub position: Transform,
    pub rigid_body: RigidBody,
    pub collider: Collider,
    pub collision_layer: CollisionLayers,
}

impl WallBundle {
    pub fn new(endpoint1: Vec2, endpoint2: Vec2, width: f32) -> Self {
        let diff = endpoint2 - endpoint1;
        let len = diff.length().max(1e-4);
        let center = (endpoint1 + endpoint2) * 0.5;
        let yaw = diff.y.atan2(diff.x);

        let position =
            Transform::from_translation(Vec3::new(center.x, WALL_HEIGHT / 2.0, center.y))
                .with_rotation(Quat::from_rotation_y(yaw));
        let collider = Collider::cuboid(len, WALL_HEIGHT, width);

        Self {
            position,
            collider,
            wall: Wall,
            rigid_body: RigidBody::Static,
            collision_layer: CollisionLayers::new(
                LayerMask(COLLISION_LAYER_WALL),
                LayerMask(COLLISION_LAYER_AGENT),
            ),
        }
    }
}
