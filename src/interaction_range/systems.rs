use bevy::prelude::*;

use crate::agent::Agent;
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
    agents: Query<(Entity, &Transform, Option<&Children>), With<Agent>>,
    mut flags: Query<(Entity, &mut Flag, &mut Transform, &InteractionRadius), Without<Agent>>,
) {
    for FlagPickupMessage { agent_id } in reader.read() {
        let agent = agents.iter().find(|(e, _, _)| e.index() == *agent_id);
        let Some((agent_entity, agent_transform, agent_children)) = agent else {
            eprintln!("Agent with id {} either does not exist", agent_id);
            continue;
        };

        // check to see if the agent is already carrying a flag
        let carrying_flag = agent_children
            .is_some_and(|children| children.iter().any(|child| flags.get(child).is_ok()));
        if carrying_flag {
            eprintln!(
                "Agent with id {} is already carrying a flag and can not pick up another",
                agent_id
            );
            continue;
        }

        let agent_position = agent_transform.translation.xz();
        for (flag_entity, mut flag, mut flag_transform, InteractionRadius(radius)) in &mut flags {
            let flag_position = flag_transform.translation.xz();
            let distance = agent_position.distance(flag_position);

            if distance < *radius && flag.status == FlagStatus::Dropped {
                commands.entity(agent_entity).add_child(flag_entity);
                flag.status = FlagStatus::PickedUp;
                flag_transform.translation = Vec3::new(0.0, 0.5, 0.0); // lift flag above agent
                break;
            }
        }
    }
}

pub fn handle_flag_drop(
    mut commands: Commands,
    mut reader: MessageReader<FlagDropMessage>,
    agents: Query<(Entity, &Transform, Option<&Children>), With<Agent>>,
    mut flags: Query<(Entity, &mut Flag, &mut Transform), Without<Agent>>,
) {
    for FlagDropMessage { agent_id } in reader.read() {
        let agent = agents.iter().find(|(e, _, _)| e.index() == *agent_id);
        let Some((agent_entity, agent_transform, agent_children)) = agent else {
            eprintln!("Agent with id {} either does not exist", agent_id);
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
            eprintln!(
                "Agent with id {} is not carrying a flag and can not drop one",
                agent_id
            );
            continue;
        };

        if let Ok((flag_entity, mut flag, mut flag_transform)) = flags.get_mut(flag_entity) {
            commands.entity(agent_entity).remove_child(flag_entity);
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
                capture_counts.0 += 1;
                flag.status = FlagStatus::Captured;
                flag_transform.translation = Vec3::ZERO;
            }
        }
    }
}
