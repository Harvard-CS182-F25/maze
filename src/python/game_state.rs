use bevy::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::gen_stub_pyclass;

use crate::{agent::Agent, character_controller::MaxLinearSpeed, flag::Flag};

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

#[derive(Clone, Debug, PartialEq)]
#[gen_stub_pyclass]
#[pyclass(name = "HitInfo", frozen)]
pub struct HitInfo {}

pub fn collect_agent_state(
    agent: Query<(Entity, &Agent, &MaxLinearSpeed, &Transform, Option<&Flag>)>,
    raycasts: &[(Vec3, Option<Entity>)],
) -> AgentState {
    // TODO: handle the whole child entity thing with flag, etc.
    let (entity, agent, max_speed, transform, flag) =
        agent.single().expect("There should be exactly one agent");

    AgentState {
        id: entity.index(),
        position: transform.translation.xz().into(),
        raycasts: raycasts.iter().map(|(pos, hit)| HitInfo {}).collect(),
        flag: None,
        max_speed: max_speed.0,
    }
}
