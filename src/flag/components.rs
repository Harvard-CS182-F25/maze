use avian3d::prelude::*;
use bevy::prelude::*;

use crate::flag::{
    CAPTURE_POINT_INTERACTION_RADIUS, COLLISION_LAYER_CAPTURE_POINT, COLLISION_LAYER_FLAG,
    FLAG_INTERACTION_RADIUS,
};
use crate::interaction_range::{InteractionRadius, VisibleRange};
use crate::scene::COLLISION_LAYER_WALL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum FlagStatus {
    Dropped,
    PickedUp,
    Captured,
}

#[derive(Debug, Clone, Copy, PartialEq, Component, Reflect)]
#[reflect(Component)]
pub struct Flag {
    pub status: FlagStatus,
}

#[derive(Bundle)]
pub struct FlagBundle {
    pub name: Name,
    pub flag: Flag,
    pub interaction_radius: InteractionRadius,
    pub visible_range: VisibleRange,
    pub transform: Transform,
    pub rigid_body: RigidBody,
    pub collider: Collider,
    pub collision_layer: CollisionLayers,
}

impl FlagBundle {
    pub fn new(name: &str, position: Vec3) -> Self {
        Self {
            name: Name::new(name.to_string()),
            flag: Flag {
                status: FlagStatus::Dropped,
            },
            interaction_radius: InteractionRadius(FLAG_INTERACTION_RADIUS),
            transform: Transform::from_translation(position),
            visible_range: VisibleRange,
            rigid_body: RigidBody::Static,
            collider: Collider::cylinder(0.5, 3.0),
            collision_layer: CollisionLayers::new(
                COLLISION_LAYER_FLAG,
                COLLISION_LAYER_FLAG | COLLISION_LAYER_WALL | COLLISION_LAYER_CAPTURE_POINT,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component, Reflect)]
#[reflect(Component)]
pub struct CapturePoint;

#[derive(Bundle)]
pub struct CapturePointBundle {
    pub name: Name,
    pub capture_point: CapturePoint,
    pub visible_range: VisibleRange,
    pub interaction_radius: InteractionRadius,
    pub transform: Transform,
    pub rigid_body: RigidBody,
    pub collider: Collider,
    pub collision_layer: CollisionLayers,
}

impl CapturePointBundle {
    pub fn new(name: &str, position: Vec3) -> Self {
        Self {
            name: Name::new(name.to_string()),
            capture_point: CapturePoint,
            interaction_radius: InteractionRadius(CAPTURE_POINT_INTERACTION_RADIUS),
            visible_range: VisibleRange,
            transform: Transform::from_translation(position),
            rigid_body: RigidBody::Static,
            collider: Collider::cylinder(0.5, 3.0),
            collision_layer: CollisionLayers::new(
                COLLISION_LAYER_CAPTURE_POINT,
                COLLISION_LAYER_FLAG | COLLISION_LAYER_WALL | COLLISION_LAYER_CAPTURE_POINT,
            ),
        }
    }
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource, Default)]
pub struct FlagCaptureCounts(pub u32);
