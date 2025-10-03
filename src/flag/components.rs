use bevy::prelude::*;

use crate::flag::{CAPTURE_POINT_INTERACTION_RADIUS, FLAG_INTERACTION_RADIUS};
use crate::interaction_range::{InteractionRadius, VisibleRange};

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
}

impl CapturePointBundle {
    pub fn new(name: &str, position: Vec3) -> Self {
        Self {
            name: Name::new(name.to_string()),
            capture_point: CapturePoint,
            interaction_radius: InteractionRadius(CAPTURE_POINT_INTERACTION_RADIUS),
            visible_range: VisibleRange,
            transform: Transform::from_translation(position),
        }
    }
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource, Default)]
pub struct FlagCaptureCounts(pub u32);
