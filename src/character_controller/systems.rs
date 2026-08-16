use avian3d::prelude::*;
use bevy::prelude::*;

use crate::character_controller::MaxLinearSpeed;

use super::components::{CharacterController, Grounded};
use super::messages::MovementMessage;

/// How far over `MaxLinearSpeed` a request may go before it counts as a real overspeed rather than
/// float noise.
const SPEED_TOLERANCE: f32 = 1.0 + 1e-4;

pub fn update_grounded(
    mut commands: Commands,
    mut query: Query<(Entity, &ShapeHits), With<CharacterController>>,
) {
    for (entity, hits) in &mut query {
        let is_grounded = !hits.is_empty();
        if is_grounded {
            commands.entity(entity).insert(Grounded);
        } else {
            commands.entity(entity).remove::<Grounded>();
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn movement(
    mut movement_event_reader: MessageReader<MovementMessage>,
    mut controllers: Query<(
        Entity,
        Option<&MaxLinearSpeed>,
        &mut LinearVelocity,
        &mut AngularVelocity,
        Has<Grounded>,
    )>,
) {
    for event in movement_event_reader.read() {
        for (entity, max_speed, mut linear_velocity, mut angular_velocity, is_grounded) in
            &mut controllers
        {
            match *event {
                MovementMessage::TranslateById(id, velocity) => {
                    if is_grounded && entity.index() == id {
                        if let Some(max_speed) = max_speed {
                            let speed = velocity.length();
                            // An agent that asks for exactly `max_speed` lands a fraction above it
                            // through ordinary float error. Clamp silently within tolerance
                            // instead of warning on every single tick.
                            if speed > max_speed.0 * SPEED_TOLERANCE {
                                let scale = max_speed.0 / speed;
                                linear_velocity.x = velocity.x * scale;
                                linear_velocity.z = velocity.y * scale;

                                eprintln!(
                                    "Agent {} attemped to move too quickly. Capping speed {} to max {} (scale {})",
                                    entity.index(),
                                    speed,
                                    max_speed.0,
                                    scale
                                );

                                continue;
                            }
                        }

                        linear_velocity.x = velocity.x;
                        linear_velocity.z = velocity.y;
                    }
                }
                MovementMessage::RotateById(id, omega) => {
                    if is_grounded && entity.index() == id {
                        angular_velocity.y = omega;
                    }
                }
            }
        }
    }
}
