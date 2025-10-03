use avian3d::prelude::*;
use bevy::prelude::*;

use crate::character_controller::MaxLinearSpeed;

use super::components::{CharacterController, Grounded};
use super::messages::MovementMessage;

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
                            if speed > max_speed.0 {
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
