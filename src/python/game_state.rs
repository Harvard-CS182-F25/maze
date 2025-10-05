use avian3d::prelude::*;
use bevy::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyclass_complex_enum};

use crate::{
    agent::{Agent, RayCasters},
    character_controller::MaxLinearSpeed,
    flag::{CapturePoint, Flag},
    scene::Wall,
};

#[derive(Clone, Debug, PartialEq)]
#[gen_stub_pyclass]
#[pyclass(name = "GameState", frozen)]
pub struct GameState {
    #[pyo3(get)]
    pub agent: AgentState,
    #[pyo3(get)]
    pub total_flags: u32,
    #[pyo3(get)]
    pub collected_flags: u32,
    #[pyo3(get)]
    pub world_width: f32,
    #[pyo3(get)]
    pub world_height: f32,
}

#[derive(Clone, Debug, PartialEq)]
#[gen_stub_pyclass]
#[pyclass(name = "AgentState", frozen)]
pub struct AgentState {
    #[pyo3(get)]
    pub id: u32,
    #[pyo3(get)]
    pub position: (f32, f32),
    #[pyo3(get)]
    pub raycasts: Vec<HitInfo>,
    #[pyo3(get)]
    pub flag: Option<u32>,
    #[pyo3(get)]
    pub max_speed: f32,
}

#[gen_stub_pyclass_complex_enum]
#[pyclass]
#[derive(Clone, Debug, PartialEq)]
pub enum CollidedEntity {
    Wall(),
    Flag(u32),
    CapturePoint(u32),
    Unknown(u32),
}

#[gen_stub_pyclass]
#[pyclass(name = "HitInfo", frozen, str)]
#[derive(Clone, Debug, PartialEq)]
pub struct HitInfo {
    #[pyo3(get)]
    pub theta: f32,
    #[pyo3(get)]
    pub hit: Option<CollidedEntity>,
    #[pyo3(get)]
    pub distance: Option<f32>,
}

impl std::fmt::Display for HitInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.hit {
            Some(collided_entity) => write!(
                f,
                "HitInfo(hit={:?}, distance={}, theta={})",
                collided_entity,
                self.distance.unwrap_or(-1.0),
                self.theta
            ),
            None => write!(f, "HitInfo(hit=None, distance=None, theta={})", self.theta),
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn collect_agent_state(
    spatial_query: &SpatialQuery,
    agent: Query<
        (
            Entity,
            &MaxLinearSpeed,
            &Transform,
            &RayCasters,
            Option<&Children>,
        ),
        With<Agent>,
    >,
    walls: Query<Entity, With<Wall>>,
    flags: Query<Entity, With<Flag>>,
    capture_points: Query<Entity, With<CapturePoint>>,
) -> AgentState {
    let (entity, max_speed, agent_transform, raycasters, children) =
        agent.single().expect("There should be exactly one agent");

    let flag = children.and_then(|children| {
        children.iter().find_map(|child| {
            if let Ok(flag_entity) = flags.get(child) {
                Some(flag_entity.index())
            } else {
                None
            }
        })
    });

    let mut hits = raycasters
        .0
        .iter()
        .map(|raycaster| {
            spatial_query
                .cast_ray(
                    agent_transform.translation + raycaster.origin,
                    raycaster.direction,
                    raycaster.max_distance,
                    raycaster.solid,
                    &raycaster.query_filter,
                )
                .map(|hit| HitInfo {
                    theta: raycaster.direction.z.atan2(raycaster.direction.x),
                    hit: if walls.get(hit.entity).is_ok() {
                        Some(CollidedEntity::Wall())
                    } else if let Ok(flag_entity) = flags.get(hit.entity) {
                        Some(CollidedEntity::Flag(flag_entity.index()))
                    } else if let Ok(capture_point_entity) = capture_points.get(hit.entity) {
                        Some(CollidedEntity::CapturePoint(capture_point_entity.index()))
                    } else {
                        Some(CollidedEntity::Unknown(hit.entity.index()))
                    },
                    distance: Some(hit.distance),
                })
                .unwrap_or(HitInfo {
                    theta: raycaster.direction.x.atan2(raycaster.direction.z),
                    hit: None,
                    distance: None,
                })
        })
        .collect::<Vec<_>>();
    hits.sort_by(|a, b| a.theta.partial_cmp(&b.theta).unwrap());

    AgentState {
        id: entity.index(),
        position: agent_transform.translation.xz().into(),
        raycasts: hits,
        flag,
        max_speed: max_speed.0,
    }
}
