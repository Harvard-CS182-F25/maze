use std::sync::{Arc, RwLock};

use avian3d::prelude::SpatialQuery;
use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender, TrySendError};
use pyo3::prelude::*;

use crate::agent::{GhostAgent, RayCasters};
use crate::character_controller::MaxLinearSpeed;
use crate::flag::{CapturePoint, Flag, FlagCaptureCounts};
use crate::interaction_range::{FlagDropMessage, FlagPickupMessage};
use crate::occupancy_grid::{OccupancyGrid, OccupancyGridView};
use crate::occupancy_grid::{PlayerGrid, TrueGrid};
use crate::python::game_state::collect_agent_state;
use crate::scene::{EstimatedPositionText, Wall};
use crate::{
    agent::{Action, Agent},
    character_controller::MovementMessage,
    core::MazeConfig,
    python::game_state::GameState,
};

#[derive(Resource)]
struct Bridge {
    pub agent_bridge: PolicyBridge,
    pub test_bridge: Option<TestHarnessBridge>,
}

struct PolicyBridge {
    pub tx_state: Sender<(GameState, Arc<RwLock<Py<OccupancyGrid>>>)>,
    pub rx_action: Receiver<Action>,
    pub rx_position: Receiver<(f32, f32)>,
}

#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct TestHarnessBridge {
    pub tx_state: Sender<(
        GameState,
        Arc<RwLock<Py<OccupancyGrid>>>,
        Arc<RwLock<Py<OccupancyGrid>>>,
    )>,
    pub rx_stop: Receiver<()>,
}

#[derive(Resource)]
struct PolicyTimer(Timer);

pub struct PythonPolicyBridgePlugin {
    pub config: MazeConfig,
    pub agent_policy: Py<PyAny>,
    pub test_harness: Option<TestHarnessBridge>,
}

impl Plugin for PythonPolicyBridgePlugin {
    fn build(&self, app: &mut App) {
        let hz = self.config.agent.policy_hz.clamp(1.0, 240.0);
        let interval = 1.0_f32 / hz;

        let agent_bridge = Python::attach(|py| {
            PolicyBridge::start(self.agent_policy.clone_ref(py))
                .expect("Failed to start agent policy")
        });

        app.insert_resource(PolicyTimer(Timer::from_seconds(
            interval,
            TimerMode::Repeating,
        )));

        app.insert_resource(Bridge {
            agent_bridge,
            test_bridge: self.test_harness.clone(),
        });

        app.add_systems(
            Update,
            (
                send_game_states,
                apply_actions,
                update_estimated_position_text,
                on_test_harness_stop,
            ),
        );

        app.add_systems(Last, shutdown_workers_on_exit);
    }
}

impl PolicyBridge {
    pub fn start(policy: Py<PyAny>) -> anyhow::Result<Self> {
        let (tx_state, rx_state) =
            crossbeam_channel::bounded::<(GameState, Arc<RwLock<Py<OccupancyGrid>>>)>(60);
        let (tx_action, rx_action) = crossbeam_channel::bounded::<Action>(60);
        let (tx_position, rx_position) = crossbeam_channel::bounded::<(f32, f32)>(60);

        std::thread::spawn(move || {
            while let Ok((state, grid)) = rx_state.recv() {
                let action_and_position = Python::attach(|py| -> PyResult<(Action, (f32, f32))> {
                    let state = Py::new(py, state)?;
                    let grid = Py::new(py, OccupancyGridView { inner: grid })?;
                    let action: Action = policy
                        .call_method(py, "get_action", (state, grid), None)?
                        .extract(py)?;

                    let position: (f32, f32) =
                        policy.call_method(py, "position", (), None)?.extract(py)?;

                    Ok((action, position))
                });

                match action_and_position {
                    Ok((action, position)) => {
                        if let Err(TrySendError::Disconnected(_)) = tx_action.try_send(action) {
                            break; // main thread has exited
                        }
                        if let Err(TrySendError::Disconnected(_)) = tx_position.try_send(position) {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Error calling policy: {:#?}", e);
                        break; // exit thread on error
                    }
                }
            }
        });

        Ok(PolicyBridge {
            tx_state,
            rx_action,
            rx_position,
        })
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn send_game_states(
    time: Res<Time>,
    mut t: ResMut<PolicyTimer>,
    scores: Res<FlagCaptureCounts>,
    config: Res<MazeConfig>,
    player_grid: Res<PlayerGrid>,
    true_grid: Res<TrueGrid>,
    bridge: Option<Res<Bridge>>,
    spatial_query: SpatialQuery,
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
    kinds: Query<(Option<&Wall>, Option<&Flag>, Option<&CapturePoint>)>,
    flags: Query<&Flag>,
) {
    if !t.0.tick(time.delta()).just_finished() {
        return;
    }

    let Some(bridge) = bridge else {
        return;
    };

    let (noisy_agent_state, true_agent_state) =
        collect_agent_state(&config, &spatial_query, agent, &kinds);

    let noisy_state = GameState {
        agent: noisy_agent_state,
        total_flags: flags.iter().count() as u32,
        collected_flags: scores.0,
        world_width: 100.0,
        world_height: 100.0,
    };

    let true_state = GameState {
        agent: true_agent_state,
        total_flags: flags.iter().count() as u32,
        collected_flags: scores.0,
        world_width: 100.0,
        world_height: 100.0,
    };

    match bridge
        .agent_bridge
        .tx_state
        .try_send((noisy_state, player_grid.0.clone()))
    {
        Ok(_) => {}
        Err(TrySendError::Full(_)) => { /* worker still busy; skip this one */ }
        Err(TrySendError::Disconnected(_)) => {
            /* Agent worker has died */
            return;
        }
    }

    if let Some(test) = &bridge.test_bridge {
        match test
            .tx_state
            .try_send((true_state, true_grid.0.clone(), player_grid.0.clone()))
        {
            Ok(_) => {}
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => { /* test harness died */ }
        }
    }
}

fn apply_actions(
    bridge: Option<Res<Bridge>>,
    agents: Query<(Entity, &Agent)>,
    mut movement_event_writer: MessageWriter<MovementMessage>,
    mut pickup_event_writer: MessageWriter<FlagPickupMessage>,
    mut drop_event_writer: MessageWriter<FlagDropMessage>,
) {
    let Some(bridge) = bridge else {
        return;
    };

    let mut latest: Option<Action> = None;
    while let Ok(action) = bridge.agent_bridge.rx_action.try_recv() {
        latest = Some(action);
    }
    let Some(action) = latest else {
        return;
    };

    match action {
        Action::Move { id, velocity } => {
            if !check_agent_exists(id, agents) {
                return;
            }
            movement_event_writer.write(MovementMessage::TranslateById(id, velocity.into()));
        }
        Action::PickupFlag { id } => {
            if !check_agent_exists(id, agents) {
                return;
            }
            pickup_event_writer.write(FlagPickupMessage { agent_id: id });
        }
        Action::DropFlag { id } => {
            if !check_agent_exists(id, agents) {
                return;
            }
            drop_event_writer.write(FlagDropMessage { agent_id: id });
        }
    }
}

fn update_estimated_position_text(
    bridge: Option<Res<Bridge>>,
    agent_transform: Query<&Transform, (With<Agent>, Without<GhostAgent>)>,
    mut ghost_agent_transform: Query<&mut Transform, (With<GhostAgent>, Without<Agent>)>,
    mut query: Query<&mut Text, With<EstimatedPositionText>>,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(agent_transform) = agent_transform.single().ok() else {
        return;
    };
    let Some(mut ghost_transform) = ghost_agent_transform.single_mut().ok() else {
        return;
    };
    let mut query = query.iter_mut();
    let Some(mut text) = query.next() else {
        return;
    };

    let mut latest: Option<(f32, f32)> = None;
    while let Ok(position) = bridge.agent_bridge.rx_position.try_recv() {
        latest = Some(position);
    }
    let Some((x, y)) = latest else {
        return;
    };

    let error = ((agent_transform.translation.x - x).powi(2)
        + (agent_transform.translation.z - y).powi(2))
    .sqrt();

    text.0 = format!("Estimated Agent Position: ({x:.2}, {y:.2}) [{error:.2}]");
    ghost_transform.translation = Vec3::new(x, 0.0, y);
}

fn on_test_harness_stop(bridge: Option<Res<Bridge>>, mut exit: MessageWriter<AppExit>) {
    let Some(bridge) = bridge else {
        return;
    };
    if let Some(test) = &bridge.test_bridge
        && test.rx_stop.try_recv().is_ok()
    {
        println!("Test harness requested stop; exiting");

        exit.write(AppExit::Success);
    }
}

fn shutdown_workers_on_exit(
    mut exit_ev: MessageReader<AppExit>,
    mut bridge: Option<ResMut<Bridge>>,
) {
    if exit_ev.read().next().is_none() {
        return;
    }

    bridge.take();
}

fn check_agent_exists(id: u32, agents: Query<(Entity, &Agent)>) -> bool {
    agents.iter().any(|(e, _a)| e.index() == id)
}
