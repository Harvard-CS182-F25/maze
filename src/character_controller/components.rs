use avian3d::{math::*, prelude::*};
use bevy::prelude::*;
use derivative::Derivative;

#[derive(Debug, Clone, Component)]
pub struct CharacterController;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Grounded;

#[derive(Debug, Clone, Component, Reflect, Derivative)]
#[derivative(Default)]
pub struct MaxLinearSpeed(#[derivative(Default(value = "10.0"))] pub f32);

#[derive(Debug, Clone, Bundle)]
pub struct CharacterControllerBundle {
    character_controller: CharacterController,
    body: RigidBody,
    collider: Collider,
    ground_caster: ShapeCaster,
    locked_axes: LockedAxes,
}

impl CharacterControllerBundle {
    pub fn new(collider: Collider) -> Self {
        let mut caster_shape = collider.clone();
        caster_shape.set_scale(Vector::ONE * 0.99, 10);

        Self {
            character_controller: CharacterController,
            body: RigidBody::Dynamic,
            collider,
            ground_caster: ShapeCaster::new(
                caster_shape,
                Vector::ZERO,
                Quaternion::default(),
                Dir3::NEG_Y,
            )
            .with_max_distance(0.2),
            locked_axes: LockedAxes::ROTATION_LOCKED,
        }
    }

    pub fn _with_locked_axes(mut self, locked_axes: LockedAxes) -> Self {
        self.locked_axes = locked_axes;
        self
    }
}
