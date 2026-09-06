use avian3d::prelude::*;
use bevy::prelude::*;

use crate::agent::Agent;
use crate::core::MazeConfig;
use crate::flag::{CapturePoint, Flag, FlagCaptureCounts, FlagStatus};
use crate::interaction_range::messages::{FlagDropMessage, FlagPickupMessage};

use super::components::{InteractionRadius, InteractionRange, VisibleRange};
use super::visual::RingAssets;

const RING_Y_OFFSET: f32 = 0.02; // lift above ground

#[allow(clippy::type_complexity)]
pub fn attach_interaction_range(
    mut commands: Commands,
    ring: Res<RingAssets>,
    interactables: Query<
        (Entity, &InteractionRadius),
        (Added<InteractionRadius>, With<VisibleRange>),
    >,
) {
    for (entity, InteractionRadius(radius)) in &interactables {
        let child = commands
            .spawn((
                Name::new("Interaction Range"),
                InteractionRange,
                Mesh3d(ring.mesh.clone()),
                MeshMaterial3d(ring.material.clone()),
                Transform::from_xyz(0.0, RING_Y_OFFSET, 0.0)
                    .with_scale(Vec3::splat(radius.max(1e-4))),
                Visibility::Inherited,
            ))
            .id();

        commands.entity(entity).add_child(child);
    }
}

#[allow(clippy::type_complexity)]
pub fn update_ring_scale_on_radius_change(
    flags: Query<(&InteractionRadius, &Children), Changed<InteractionRadius>>,
    mut ring_transforms: Query<&mut Transform, With<InteractionRange>>,
) {
    for (InteractionRadius(r), children) in &flags {
        for child in children.iter() {
            if let Ok(mut t) = ring_transforms.get_mut(child) {
                t.scale = Vec3::splat(r.max(0.0001));
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn remove_ring_on_radius_removal(
    mut commands: Commands,
    mut removed: RemovedComponents<InteractionRadius>,
    children: Query<&Children>,
    range_marker: Query<(), With<InteractionRange>>,
) {
    for parent in removed.read() {
        if let Ok(children) = children.get(parent) {
            for child in children.iter() {
                if range_marker.get(child).is_ok() {
                    commands.entity(child).despawn();
                }
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn handle_flag_pickups(
    mut commands: Commands,
    mut reader: MessageReader<FlagPickupMessage>,
    mut announced_out_of_range: Local<bool>,
    agents: Query<(Entity, &Transform, Option<&Children>), With<Agent>>,
    mut flags: Query<(Entity, &mut Flag, &mut Transform, &InteractionRadius), Without<Agent>>,
) {
    for _ in reader.read() {
        let Ok((agent_entity, agent_transform, agent_children)) = agents.single() else {
            continue;
        };

        // An agent can carry at most one flag.
        let carrying_flag = agent_children
            .is_some_and(|children| children.iter().any(|child| flags.get(child).is_ok()));
        if carrying_flag {
            warn!("The agent is already carrying a flag and cannot pick up another");
            continue;
        }

        let agent_position = agent_transform.translation.xz();
        let mut picked_up = false;
        let mut nearest: Option<(f32, f32)> = None;

        for (flag_entity, mut flag, mut flag_transform, InteractionRadius(radius)) in &mut flags {
            let flag_position = flag_transform.translation.xz();
            let distance = agent_position.distance(flag_position);

            if flag.status == FlagStatus::Dropped
                && nearest.is_none_or(|(nearest, _)| distance < nearest)
            {
                nearest = Some((distance, *radius));
            }

            if distance < *radius && flag.status == FlagStatus::Dropped {
                commands.entity(agent_entity).add_child(flag_entity);
                commands
                    .entity(flag_entity)
                    .remove::<RigidBody>()
                    .remove::<Collider>()
                    .remove::<InteractionRadius>();
                flag.status = FlagStatus::PickedUp;
                flag_transform.translation = Vec3::new(0.0, 0.5, 0.0); // lift flag above agent
                picked_up = true;
                break;
            }
        }

        // Reaching for a flag that is not there is the one failure the agent got no word about,
        // and it is the one students hit while debugging a navigation bug.
        if !picked_up && !*announced_out_of_range {
            *announced_out_of_range = true;
            match nearest {
                Some((distance, radius)) => warn!(
                    "The agent tried to pick up a flag, but the nearest dropped one is \
                     {distance:.1} units away and must be within {radius:.1}. Not reporting this again."
                ),
                None => warn!(
                    "The agent tried to pick up a flag, but none are on the ground to pick up. \
                     Not reporting this again."
                ),
            }
        }
    }
}

pub fn handle_flag_drop(
    mut commands: Commands,
    config: Res<MazeConfig>,
    mut reader: MessageReader<FlagDropMessage>,
    agents: Query<(Entity, &Transform, Option<&Children>), With<Agent>>,
    mut flags: Query<(Entity, &mut Flag, &mut Transform), Without<Agent>>,
) {
    for _ in reader.read() {
        let Ok((agent_entity, agent_transform, agent_children)) = agents.single() else {
            continue;
        };

        let flag_entity = agent_children.and_then(|children| {
            children.iter().find_map(|child| {
                if let Ok((flag_entity, _, _)) = flags.get(child) {
                    Some(flag_entity)
                } else {
                    None
                }
            })
        });
        let Some(flag_entity) = flag_entity else {
            warn!("The agent is not carrying a flag and cannot drop one");
            continue;
        };

        if let Ok((flag_entity, mut flag, mut flag_transform)) = flags.get_mut(flag_entity) {
            commands.entity(agent_entity).remove_child(flag_entity);
            commands.entity(flag_entity).insert((
                RigidBody::Kinematic,
                Collider::cylinder(0.5, 3.0),
                InteractionRadius(config.flags.flag_radius),
            ));
            flag.status = FlagStatus::Dropped;
            flag_transform.translation = agent_transform.translation
        }
    }
}

pub fn handle_flag_capture(
    mut commands: Commands,
    mut flags: Query<(Entity, &mut Flag, &mut Transform), Without<CapturePoint>>,
    mut capture_points: Query<
        (Entity, &Transform, Option<&Children>, &InteractionRadius),
        With<CapturePoint>,
    >,
    mut capture_counts: ResMut<FlagCaptureCounts>,
) {
    for (
        capture_point_entity,
        capture_point_transform,
        capture_point_children,
        &InteractionRadius(radius),
    ) in &mut capture_points
    {
        let has_flag = capture_point_children
            .is_some_and(|children| children.iter().any(|child| flags.get(child).is_ok()));
        if has_flag {
            continue;
        }
        let capture_point_position = capture_point_transform.translation.xz();
        for (flag_entity, mut flag, mut flag_transform) in &mut flags {
            let flag_position = flag_transform.translation.xz();
            let distance = capture_point_position.distance(flag_position);

            if distance < radius && flag.status == FlagStatus::Dropped {
                commands.entity(capture_point_entity).add_child(flag_entity);
                commands.entity(flag_entity).remove::<InteractionRadius>();
                commands
                    .entity(capture_point_entity)
                    .remove::<InteractionRadius>();
                capture_counts.0 += 1;
                flag.status = FlagStatus::Captured;
                flag_transform.translation = Vec3::ZERO;
                // A capture point holds one flag. `has_flag` cannot see the child added just above
                // because commands are deferred, so without this a second flag in range on the
                // same tick would be captured too.
                break;
            }
        }
    }
}
