use avian3d::prelude::*;
use bevy::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pyclass_enum, gen_stub_pymethods};
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use rand_distr::Distribution;
use rand_distr::Normal;

use crate::core::MazeConfig;

use crate::{
    agent::{Agent, RayCasters},
    character_controller::MaxLinearSpeed,
    flag::{CapturePoint, Flag},
    interaction_range::InteractionRadius,
    scene::Wall,
};

#[derive(Clone, Debug, PartialEq)]
#[gen_stub_pyclass]
#[pyclass(name = "GameState", frozen)]
/// Represents a snapshot of the game state.
pub struct GameState {
    /// Returns the state of the agent.
    #[pyo3(get)]
    pub agent: AgentState,
    /// Returns the total number of flags.
    #[pyo3(get)]
    pub total_flags: u32,
    /// Returns the number of flags delivered to capture points.
    #[pyo3(get)]
    pub captured_flags: u32,
    /// Returns the width of the maze in world units.
    #[pyo3(get)]
    pub world_width: f32,
    /// Returns the height of the maze in world units.
    #[pyo3(get)]
    pub world_height: f32,
}

#[derive(Clone, Debug, PartialEq)]
#[gen_stub_pyclass]
#[pyclass(name = "AgentState", frozen)]
/// Represents the state of the agent, including its observed information.
pub struct AgentState {
    /// Returns the observed position, including position noise.
    #[pyo3(get)]
    pub position: (f32, f32),

    /// Returns the standard deviation of position observations.
    #[pyo3(get)]
    pub position_stddev: f32,

    /// Returns a list of range-sensor readings.
    #[pyo3(get)]
    pub raycasts: Vec<Raycast>,

    /// Returns whether the agent is currently carrying a flag.
    #[pyo3(get)]
    pub carrying_flag: bool,

    /// Returns the maximum linear speed of the agent.
    #[pyo3(get)]
    pub max_speed: f32,
}

#[gen_stub_pyclass_enum]
#[pyclass(name = "EntityType", frozen, eq, hash, str)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
/// Represents the type observed at the endpoint of a range-sensor reading.
pub enum EntityType {
    Free,
    Wall,
    Flag,
    CapturePoint,
    Unknown,
}

#[gen_stub_pymethods]
#[pymethods]
impl EntityType {
    fn __repr__(&self) -> String {
        self.to_string()
    }
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EntityType::Free => "Free",
            EntityType::Wall => "Wall",
            EntityType::Flag => "Flag",
            EntityType::CapturePoint => "CapturePoint",
            EntityType::Unknown => "Unknown",
        };
        write!(f, "{}", s)
    }
}

#[gen_stub_pyclass]
#[pyclass(name = "Raycast", frozen, str)]
#[derive(Clone, Debug, PartialEq)]
/// Represents one range-sensor reading from the agent's current position.
pub struct Raycast {
    /// Returns the ray angle in radians clockwise from +x.
    #[pyo3(get)]
    pub theta: f32,

    /// Returns the type observed at the raycast endpoint. Equals `Free` if
    /// no collision occurs before the endpoint.
    #[pyo3(get)]
    pub endpoint_type: EntityType,

    /// Returns the reported distance, including range noise.
    #[pyo3(get)]
    pub distance: f32,

    /// Returns the maximum distance the raycast can travel.
    #[pyo3(get)]
    pub max_distance: f32,

    /// Returns the per-class confidence at the endpoint.
    #[pyo3(get)]
    pub endpoint_confidence: SensorConfidence,

    /// Returns the per-class confidence for cells before the endpoint.
    #[pyo3(get)]
    pub path_confidence: SensorConfidence,
}

#[gen_stub_pyclass]
#[pyclass(name = "SensorConfidence")]
#[derive(Clone, Debug, PartialEq)]
/// Represents per-class sensor confidence used to update an occupancy grid.
/// Note that these are not normalized probabilities.
pub struct SensorConfidence {
    /// Returns the confidence in a free space.
    #[pyo3(get)]
    pub conf_free: f32,

    /// Returns the confidence in a wall.
    #[pyo3(get)]
    pub conf_wall: f32,

    /// Returns the confidence in a flag.
    #[pyo3(get)]
    pub conf_flag: f32,

    /// Returns the confidence in a capture point.
    #[pyo3(get)]
    pub conf_capture_point: f32,
}

#[gen_stub_pymethods]
#[pymethods]
impl SensorConfidence {
    fn __repr__(&self) -> String {
        format!(
            "SensorConfidence(free={}, wall={}, flag={}, capture_point={})",
            self.conf_free, self.conf_wall, self.conf_flag, self.conf_capture_point
        )
    }

    #[new]
    pub fn new(conf_free: f32, conf_wall: f32, conf_flag: f32, conf_capture_point: f32) -> Self {
        Self {
            conf_free,
            conf_wall,
            conf_flag,
            conf_capture_point,
        }
    }

    /// Returns `(conf_free, conf_wall, conf_flag, conf_capture_point)`.
    pub fn as_tuple(&self) -> (f32, f32, f32, f32) {
        (
            self.conf_free,
            self.conf_wall,
            self.conf_flag,
            self.conf_capture_point,
        )
    }
}

impl From<[f32; 4]> for SensorConfidence {
    fn from(conf: [f32; 4]) -> Self {
        Self {
            conf_free: conf[0],
            conf_wall: conf[1],
            conf_flag: conf[2],
            conf_capture_point: conf[3],
        }
    }
}

#[gen_stub_pymethods]
#[pymethods]
impl Raycast {
    fn __repr__(&self) -> String {
        self.to_string()
    }
}

impl std::fmt::Display for Raycast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Raycast(endpoint_type={:?}, distance={}, theta={})",
            self.endpoint_type, self.distance, self.theta
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
        EntityType::Free => [0.85, 0.15, 0.20, 0.20].into(),
        EntityType::Flag => [0.05, 0.10, 0.85, 0.10].into(),
        EntityType::CapturePoint => [0.05, 0.10, 0.10, 0.85].into(),
        EntityType::Unknown => [0.25, 0.25, 0.25, 0.25].into(),
    }
}

/// The RNG behind the odometry and range noise. Seeded from the maze seed so that a headless run
/// with a fixed seed is reproducible; `rand::rng()` would make every run differ.
#[derive(Resource)]
pub struct SensorRng(pub ChaCha20Rng);

impl SensorRng {
    pub fn from_seed(seed: u32) -> Self {
        let mut bytes = [0u8; 32];
        bytes[..4].copy_from_slice(&seed.to_le_bytes());
        SensorRng(ChaCha20Rng::from_seed(bytes))
    }
}

#[allow(clippy::type_complexity)]
pub fn collect_agent_state(
    config: &MazeConfig,
    sensor_rng: &mut SensorRng,
    spatial_query: &SpatialQuery,
    agent: Query<(&MaxLinearSpeed, &Transform, &RayCasters, Option<&Children>), With<Agent>>,
    kinds: &Query<(Option<&Wall>, Option<&Flag>, Option<&CapturePoint>)>,
    spent: &Query<Entity, (Or<(With<Flag>, With<CapturePoint>)>, Without<InteractionRadius>)>,
) -> (AgentState, AgentState) {
    let (max_speed, agent_transform, raycasters, children) =
        agent.single().expect("There should be exactly one agent");

    let flag = children.and_then(|kids| {
        kids.iter().find_map(|child| {
            let (_, f, _) = kinds.get(child).ok()?;
            f.as_ref()?;
            Some(child)
        })
    });

    // Anything that has lost its `InteractionRadius` is hidden from the map, so the rays have to
    // agree: otherwise the agent senses a capture point the map says is not there. This covers the
    // carried flag too, which would otherwise sit on top of the ray origin and return zero range.
    let hidden: Vec<Entity> = spent.iter().collect();

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
                    .with_excluded_entities(hidden.iter().copied()),
            );

            let entity_type = hit
                .map(|hit| classify(hit.entity, kinds))
                .unwrap_or(EntityType::Free);

            let distance = hit
                .map(|hit| hit.distance)
                .unwrap_or(raycaster.max_distance);

            Raycast {
                theta: raycaster.direction.z.atan2(raycaster.direction.x),
                endpoint_type: entity_type,
                distance,
                max_distance: raycaster.max_distance,
                endpoint_confidence: confidence_by_entity_type(entity_type),
                path_confidence: [0.9, 0.01, 0.045, 0.045].into(),
            }
        })
        .collect::<Vec<_>>();
    raycasts.sort_by(|a, b| a.theta.partial_cmp(&b.theta).unwrap());

    let position_noise_distribution = Normal::new(0.0, config.agent.position_stddev)
        .expect("Normal distribution should be valid");
    let range_noise_distribution =
        Normal::new(0.0, config.agent.range_stddev).expect("Normal distribution should be valid");

    let true_agent_state = AgentState {
        position: agent_transform.translation.xz().into(),
        position_stddev: config.agent.position_stddev,
        raycasts,
        carrying_flag: flag.is_some(),
        max_speed: max_speed.0,
    };

    let rng = &mut sensor_rng.0;
    let noisy_agent_state = AgentState {
        position: (
            agent_transform.translation.x + position_noise_distribution.sample(rng),
            agent_transform.translation.z + position_noise_distribution.sample(rng),
        ),
        raycasts: true_agent_state
            .raycasts
            .clone()
            .into_iter()
            .map(|hit_info| Raycast {
                distance: {
                    let noise = range_noise_distribution.sample(rng);
                    (hit_info.distance + noise).clamp(0.0, hit_info.max_distance)
                },
                ..hit_info
            })
            .collect::<Vec<_>>(),
        ..true_agent_state
    };

    (noisy_agent_state, true_agent_state)
}
