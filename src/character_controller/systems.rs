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
    mut announced_overspeed: Local<bool>,
    mut controllers: Query<
        (Option<&MaxLinearSpeed>, &mut LinearVelocity, Has<Grounded>),
        With<CharacterController>,
    >,
) {
    for MovementMessage(velocity) in movement_event_reader.read() {
        for (max_speed, mut linear_velocity, is_grounded) in &mut controllers {
            if !is_grounded {
                continue;
            }

            let mut velocity = *velocity;
            if let Some(max_speed) = max_speed {
                let speed = velocity.length();
                // An agent that asks for exactly `max_speed` lands a fraction above it through
                // ordinary float error. Clamp silently within tolerance instead of warning.
                if speed > max_speed.0 * SPEED_TOLERANCE {
                    velocity *= max_speed.0 / speed;

                    if !*announced_overspeed {
                        *announced_overspeed = true;
                        warn!(
                            "The agent asked to move at {speed}, above its max speed of {}. \
                             Capping, and not reporting this again.",
                            max_speed.0
                        );
                    }
                }
            }

            linear_velocity.x = velocity.x;
            linear_velocity.z = velocity.y;
        }
    }
}
