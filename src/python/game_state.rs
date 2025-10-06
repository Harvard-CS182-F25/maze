use avian3d::prelude::*;
use bevy::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyclass_complex_enum};
use rand::rng;
use rand_distr::Distribution;
use rand_distr::Normal;

use crate::core::MazeConfig;
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
#[pyclass(name = "EntityType", frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum EntityType {
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
    pub hit: Option<EntityType>,
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

fn classify(
    e: Entity,
    kinds: &Query<(Option<&Wall>, Option<&Flag>, Option<&CapturePoint>)>,
) -> Option<EntityType> {
    if let Ok((is_wall, is_flag, is_cp)) = kinds.get(e) {
        if is_wall.is_some() {
            return Some(EntityType::Wall());
        }
        if let Some(_f) = is_flag {
            return Some(EntityType::Flag(e.index()));
        }
        if let Some(_cp) = is_cp {
            return Some(EntityType::CapturePoint(e.index()));
        }
    }
    Some(EntityType::Unknown(e.index()))
}

#[allow(clippy::type_complexity)]
pub fn collect_agent_state(
    config: &MazeConfig,
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
    kinds: &Query<(Option<&Wall>, Option<&Flag>, Option<&CapturePoint>)>,
) -> (AgentState, AgentState) {
    let (entity, max_speed, agent_transform, raycasters, children) =
        agent.single().expect("There should be exactly one agent");

    let flag = children.and_then(|kids| {
        kids.iter().find_map(|child| {
            let (_, f, _) = kinds.get(child).ok()?;
            f.as_ref()?;
            Some(child.index())
        })
    });

    let mut raycasts = raycasters
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
                    hit: classify(hit.entity, kinds),
                    distance: Some(hit.distance),
                })
                .unwrap_or(HitInfo {
                    theta: raycaster.direction.x.atan2(raycaster.direction.z),
                    hit: None,
                    distance: None,
                })
        })
        .collect::<Vec<_>>();
    raycasts.sort_by(|a, b| a.theta.partial_cmp(&b.theta).unwrap());

    let odometry_noise_distribution = Normal::new(0.0, config.agent.odometry_stddev)
        .expect("Normal distribution should be valid");
    let range_noise_distribution =
        Normal::new(0.0, config.agent.range_stddev).expect("Normal distribution should be valid");

    let true_agent_state = AgentState {
        id: entity.index(),
        position: agent_transform.translation.xz().into(),
        raycasts,
        flag,
        max_speed: max_speed.0,
    };

    let noisy_agent_state = AgentState {
        position: (
            agent_transform.translation.x + odometry_noise_distribution.sample(&mut rng()),
            agent_transform.translation.z + odometry_noise_distribution.sample(&mut rng()),
        ),
        raycasts: true_agent_state
            .raycasts
            .clone()
            .into_iter()
            .map(|hit_info| HitInfo {
                distance: hit_info.distance.map(|d| {
                    let noise = range_noise_distribution.sample(&mut rng());
                    (d + noise).clamp(0.0, config.maze_generation.cell_size)
                }),
                ..hit_info
            })
            .collect::<Vec<_>>(),
        ..true_agent_state
    };

    (noisy_agent_state, true_agent_state)
}
