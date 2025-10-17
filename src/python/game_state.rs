use avian3d::prelude::*;
use bevy::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyclass_enum, gen_stub_pymethods};
use rand::rng;
use rand_distr::Distribution;
use rand_distr::Normal;

use crate::agent::AGENT_RAYCAST_MAX_DISTANCE;
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
    /// The unique ID of the agent entity.
    #[pyo3(get)]
    pub id: u32,

    /// The (noisy!) position of the agent in world coordinates
    #[pyo3(get)]
    pub position: (f32, f32),

    /// The standard deviation of the position noise. This noise is Gaussian with mean 0 and stddev `position_stddev`.
    #[pyo3(get)]
    pub position_stddev: f32,

    /// The results of the agent's raycasts.
    #[pyo3(get)]
    pub raycasts: Vec<HitInfo>,

    /// The entity ID of the flag the agent is currently carrying, if any.
    #[pyo3(get)]
    pub flag: Option<u32>,

    /// The maximum linear speed of the agent.
    #[pyo3(get)]
    pub max_speed: f32,
}

#[gen_stub_pyclass_enum]
#[pyclass(name = "EntityType", frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
/// The type of entity that was hit by a raycast. Note, that "Unknown" should not occur.
pub enum EntityType {
    Wall,
    Empty,
    Flag,
    CapturePoint,
    Unknown,
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EntityType::Wall => "Wall",
            EntityType::Empty => "Empty",
            EntityType::Flag => "Flag",
            EntityType::CapturePoint => "CapturePoint",
            EntityType::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

#[gen_stub_pyclass]
#[pyclass(name = "HitInfo", frozen, str)]
#[derive(Clone, Debug, PartialEq)]
pub struct HitInfo {
    /// The angle of the raycast in radians, relative to the +x axis (right on the screen). Remember, +y points down on the screen!
    #[pyo3(get)]
    pub theta: f32,

    /// The type of entity that was hit by the raycast.
    #[pyo3(get)]
    pub hit: EntityType,

    /// How far the ray traveled before hitting something, or the max distance if nothing was hit.
    #[pyo3(get)]
    pub distance: f32,

    /// The maximum distance the raycast could travel.
    #[pyo3(get)]
    pub max_distance: f32,

    /// The confidence (probability of each class) of the thing that the ray hit.
    /// If nothing was hit, this will be the confidence of an empty space.
    #[pyo3(get)]
    pub hit_confidence: SensorConfidence,

    /// The confidence (probability of each class) of the cells that the ray passed through of being free space.
    #[pyo3(get)]
    pub free_confidence: SensorConfidence,
}

#[gen_stub_pyclass]
#[pyclass(name = "SensorConfidence")]
#[derive(Clone, Debug, PartialEq)]
pub struct SensorConfidence {
    /// Probability of being free space
    #[pyo3(get)]
    pub p_free: f32,

    /// Probability of being a wall
    #[pyo3(get)]
    pub p_wall: f32,

    /// Probability of being a flag
    #[pyo3(get)]
    pub p_flag: f32,

    /// Probability of being a capture point
    #[pyo3(get)]
    pub p_capture_point: f32,
}

#[gen_stub_pymethods]
#[pymethods]
impl SensorConfidence {
    #[new]
    pub fn new(p_free: f32, p_wall: f32, p_flag: f32, p_capture_point: f32) -> Self {
        Self {
            p_free,
            p_wall,
            p_flag,
            p_capture_point,
        }
    }

    pub fn as_tuple(&self) -> (f32, f32, f32, f32) {
        (self.p_free, self.p_wall, self.p_flag, self.p_capture_point)
    }
}

impl From<[f32; 4]> for SensorConfidence {
    fn from(conf: [f32; 4]) -> Self {
        Self {
            p_free: conf[0],
            p_wall: conf[1],
            p_flag: conf[2],
            p_capture_point: conf[3],
        }
    }
}

impl std::fmt::Display for HitInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "HitInfo(hit={:?}, distance={}, theta={})",
            self.hit, self.distance, self.theta
        )
    }
}

fn classify(
    e: Entity,
    kinds: &Query<(Option<&Wall>, Option<&Flag>, Option<&CapturePoint>)>,
) -> EntityType {
    match kinds.get(e) {
        Ok((Some(_), None, None)) => EntityType::Wall,
        Ok((None, Some(_), None)) => EntityType::Flag,
        Ok((None, None, Some(_))) => EntityType::CapturePoint,
        Ok(kinds) => {
            warn!(
                "Entity {:?} has multiple kinds {:?}, classifying as Unknown",
                e, kinds
            );
            EntityType::Unknown
        }
        Err(err) => {
            warn!(
                "{:?} Entity {:?} has no kind, classifying as Unknown",
                err, e
            );
            EntityType::Unknown
        }
    }
}

#[inline]
fn confidence_by_entity_type(entity_type: EntityType) -> SensorConfidence {
    match entity_type {
        EntityType::Wall => [0.05, 0.90, 0.05, 0.05].into(),
        EntityType::Empty => [0.85, 0.15, 0.20, 0.20].into(),
        EntityType::Flag => [0.05, 0.10, 0.85, 0.10].into(),
        EntityType::CapturePoint => [0.05, 0.10, 0.10, 0.85].into(),
        EntityType::Unknown => [0.25, 0.25, 0.25, 0.25].into(),
    }
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
            Some(child)
        })
    });

    let mut raycasts = raycasters
        .0
        .iter()
        .map(|raycaster| {
            let hit = spatial_query.cast_ray(
                agent_transform.translation + raycaster.origin,
                raycaster.direction,
                raycaster.max_distance,
                raycaster.solid,
                &raycaster
                    .query_filter
                    .clone()
                    .with_excluded_entities(flag.map(|e| vec![e]).unwrap_or(vec![])),
            );

            let entity_type = hit
                .map(|hit| classify(hit.entity, kinds))
                .unwrap_or(EntityType::Empty);

            let distance = hit
                .map(|hit| hit.distance)
                .unwrap_or(raycaster.max_distance);

            HitInfo {
                theta: raycaster.direction.z.atan2(raycaster.direction.x),
                hit: entity_type,
                distance,
                max_distance: raycaster.max_distance,
                hit_confidence: confidence_by_entity_type(entity_type),
                free_confidence: [0.9, 0.01, 0.045, 0.045].into(),
            }
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
        position_stddev: config.agent.odometry_stddev,
        raycasts,
        flag: flag.map(|f| f.index()),
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
                distance: {
                    let noise = range_noise_distribution.sample(&mut rng());
                    (hit_info.distance + noise).clamp(0.0, AGENT_RAYCAST_MAX_DISTANCE)
                },
                ..hit_info
            })
            .collect::<Vec<_>>(),
        ..true_agent_state
    };

    (noisy_agent_state, true_agent_state)
}
