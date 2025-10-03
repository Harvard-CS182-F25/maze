use bevy::prelude::*;

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct InteractionRadius(pub f32);

#[derive(Component)]
pub struct InteractionRange;

#[derive(Component)]
pub struct VisibleRange;
