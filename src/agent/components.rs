use avian3d::prelude::*;
use bevy::prelude::*;
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass_complex_enum;

use crate::{
    agent::COLLISION_LAYER_AGENT,
    character_controller::{CharacterControllerBundle, MaxLinearSpeed},
    scene::{COLLISION_LAYER_WALL, NUM_AGENT_RAYS},
};

#[derive(Debug, Clone, Copy, PartialEq, Component, Reflect, Derivative)]
#[derivative(Default)]
#[reflect(Component)]
pub struct Agent;

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
pub struct RayCasters(pub Vec<RayCaster>);

impl RayCasters {
    pub fn new(num_rays: u32, max_distance: f32) -> Self {
        let thetas = (0..num_rays).map(|i| i as f32 * (std::f32::consts::TAU / num_rays as f32));

        RayCasters(
            thetas
                .map(|theta| {
                    let direction = Vec3::new(theta.cos(), 0.0, theta.sin());
                    RayCaster::new(Vec3::ZERO, Dir3::new(direction).unwrap())
                        .with_max_hits(1)
                        .with_max_distance(max_distance)
                        .with_query_filter(SpatialQueryFilter::from_mask(COLLISION_LAYER_WALL))
                })
                .collect::<Vec<_>>(),
        )
    }
}

#[derive(Debug, Clone, Bundle)]
pub struct AgentBundle {
    pub name: Name,
    pub agent: Agent,
    pub position: Transform,
    pub friction: Friction,
    pub restitution: Restitution,
    pub character_controller: CharacterControllerBundle,
    pub collision_layer: CollisionLayers,
    pub max_speed: MaxLinearSpeed,
    pub raycasters: RayCasters,
}

#[derive(Debug, Clone, PartialEq, Reflect)]
#[gen_stub_pyclass_complex_enum]
#[pyclass(name = "Action")]
pub enum Action {
    Move { id: u32, velocity: (f32, f32) },
    PickupFlag { id: u32 },
    DropFlag { id: u32 },
}

impl AgentBundle {
    pub fn new(name: &str, position: Vec3, max_speed: f32, max_distance: f32) -> Self {
        Self {
            name: Name::new(name.to_string()),
            agent: Agent,
            position: Transform::from_translation(position),
            max_speed: MaxLinearSpeed(max_speed),
            raycasters: RayCasters::new(NUM_AGENT_RAYS, max_distance),
            ..Default::default()
        }
    }
}

impl Default for AgentBundle {
    fn default() -> Self {
        let collision_layer = CollisionLayers::new(
            LayerMask(COLLISION_LAYER_AGENT),
            LayerMask(COLLISION_LAYER_AGENT | COLLISION_LAYER_WALL),
        );

        Self {
            name: Name::new("Agent"),
            agent: Agent,
            position: Transform::default(),
            max_speed: MaxLinearSpeed::default(),
            friction: Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
            restitution: Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
            character_controller: CharacterControllerBundle::new(Collider::cuboid(1.0, 1.0, 1.0)),
            collision_layer,
            raycasters: RayCasters::new(NUM_AGENT_RAYS, 20.0),
        }
    }
}
