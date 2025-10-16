use avian3d::prelude::*;
use bevy::prelude::*;
use derivative::Derivative;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass_complex_enum;

use crate::{
    agent::{AGENT_RAYCAST_MAX_DISTANCE, COLLISION_LAYER_AGENT, NUM_AGENT_RAYS},
    character_controller::{CharacterControllerBundle, MaxLinearSpeed},
    flag::{COLLISION_LAYER_CAPTURE_POINT, COLLISION_LAYER_FLAG},
    scene::COLLISION_LAYER_WALL,
};

#[derive(Debug, Clone, Copy, PartialEq, Component, Reflect, Derivative)]
#[derivative(Default)]
#[reflect(Component)]
pub struct Agent;

#[derive(Debug, Clone, Copy, PartialEq, Component, Reflect, Derivative)]
#[derivative(Default)]
#[reflect(Component)]
pub struct GhostAgent;

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
pub struct RayCasters(pub Vec<RayCaster>);

impl RayCasters {
    pub fn new(num_rays: u32, max_distance: f32) -> Self {
        let thetas = (0..num_rays)
            .map(|i| (2 * i + 1) as f32 * (std::f32::consts::TAU / (2 * num_rays) as f32));

        RayCasters(
            thetas
                .map(|theta| {
                    let direction = Vec3::new(theta.cos(), 0.0, theta.sin());
                    RayCaster::new(Vec3::ZERO.with_y(0.5), Dir3::new(direction).unwrap())
                        .with_max_hits(1)
                        .with_max_distance(max_distance)
                        .with_query_filter(SpatialQueryFilter::from_mask(
                            COLLISION_LAYER_WALL
                                | COLLISION_LAYER_FLAG
                                | COLLISION_LAYER_CAPTURE_POINT,
                        ))
                })
                .collect::<Vec<_>>(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Reflect)]
#[gen_stub_pyclass_complex_enum]
#[pyclass(name = "Action")]
pub enum Action {
    Move { id: u32, velocity: (f32, f32) },
    PickupFlag { id: u32 },
    DropFlag { id: u32 },
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
            raycasters: RayCasters::new(NUM_AGENT_RAYS, AGENT_RAYCAST_MAX_DISTANCE),
        }
    }
}

#[derive(Debug, Clone, Bundle)]
pub struct GhostAgentBundle {
    pub name: Name,
    pub agent: GhostAgent,
    pub position: Transform,
}

impl GhostAgentBundle {
    pub fn new(name: &str, position: Vec3) -> Self {
        Self {
            name: Name::new(name.to_string()),
            agent: GhostAgent,
            position: Transform::from_translation(position),
        }
    }
}

impl Default for GhostAgentBundle {
    fn default() -> Self {
        Self {
            name: Name::new("GhostAgent"),
            agent: GhostAgent,
            position: Transform::default(),
        }
    }
}
