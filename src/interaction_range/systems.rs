use avian3d::prelude::*;
use bevy::prelude::*;

use crate::agent::{Agent, AgentGraphicsAssets};
use crate::flag::{CapturePoint, Flag, FlagCaptureCounts, FlagStatus};
use crate::interaction_range::messages::{FlagCaptureMessage, FlagDropMessage, FlagPickupMessage};

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

pub fn handle_flag_pickups(
    mut commands: Commands,
    mut reader: MessageReader<FlagPickupMessage>,
    mut flags: Query<(&mut Flag, &mut Visibility, &mut Transform)>,
    agent_graphics: Option<Res<AgentGraphicsAssets>>,
) {
    unimplemented!()
}

pub fn detect_flag_capture(
    mut writer: MessageWriter<FlagCaptureMessage>,
    agents: Query<(Entity, &Transform), With<Agent>>,
    capture_points: Query<(Entity, &InteractionRadius, &Transform, &CapturePoint)>,
) {
    unimplemented!()
}

pub fn handle_flag_capture(
    mut commands: Commands,
    mut reader: MessageReader<FlagCaptureMessage>,
    mut agents: Query<&mut Agent>,
    mut flags: Query<(&mut Flag, &mut Visibility, &mut Transform)>,
    mut capture_points: Query<&mut CapturePoint>,
    mut capture_counts: ResMut<FlagCaptureCounts>,
    agent_graphics: Option<Res<AgentGraphicsAssets>>,
) {
    unimplemented!()
}

pub fn handle_flag_drop(
    mut commands: Commands,
    mut reader: MessageReader<FlagDropMessage>,
    mut agents: Query<&mut Agent>,
    mut flags: Query<(&mut Flag, &mut Visibility, &mut Transform)>,
    agent_graphics: Option<Res<AgentGraphicsAssets>>,
) {
    unimplemented!()
}
