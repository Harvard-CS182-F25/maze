use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender, TrySendError};
use pyo3::prelude::*;

use crate::character_controller::MaxLinearSpeed;
use crate::flag::{CapturePoint, Flag, FlagCaptureCounts};
use crate::interaction_range::{FlagDropMessage, FlagPickupMessage};
use crate::python::game_state::{AgentState, collect_agent_state};
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
    pub tx_state: Sender<GameState>,
    pub rx_action: Receiver<Action>,
}

#[derive(Clone)]
pub struct TestHarnessBridge {
    pub tx_state: Sender<GameState>,
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

        let bridge = Python::attach(|py| {
            PolicyBridge::start(self.agent_policy.clone_ref(py))
                .expect("Failed to start agent policy")
        });

        app.insert_resource(PolicyTimer(Timer::from_seconds(
            interval,
            TimerMode::Repeating,
        )));

        let test = if self.config.headless {
            unimplemented!()
        } else {
            None
        };

        app.insert_resource(Bridge {
            agent_bridge: bridge,
            test_bridge: test,
        });

        app.add_systems(
            Update,
            (send_game_states, apply_actions, on_test_harness_stop),
        );

        app.add_systems(Last, shutdown_workers_on_exit);
    }
}

impl PolicyBridge {
    pub fn start(policy: Py<PyAny>) -> anyhow::Result<Self> {
        let (tx_state, rx_state) = crossbeam_channel::bounded::<GameState>(60);
        let (tx_action, rx_action) = crossbeam_channel::bounded::<Action>(60);

        std::thread::spawn(move || {
            while let Ok(state) = rx_state.recv() {
                let action = Python::attach(|py| -> PyResult<Action> {
                    let state = Py::new(py, state)?;
                    let action: Action = policy
                        .call_method(py, "get_action", (state,), None)?
                        .extract(py)?;
                    Ok(action)
                });

                match action {
                    Ok(action) => {
                        if let Err(TrySendError::Disconnected(_)) = tx_action.try_send(action) {
                            break; // main thread has exited
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
        })
    }
}

fn send_game_states(
    time: Res<Time>,
    mut t: ResMut<PolicyTimer>,
    scores: Res<FlagCaptureCounts>,
    bridge: Option<Res<Bridge>>,
    agent: Query<(Entity, &Agent, &MaxLinearSpeed, &Transform, Option<&Flag>)>,
    flags: Query<&Flag>,
) {
    if !t.0.tick(time.delta()).just_finished() {
        return;
    }

    let Some(bridge) = bridge else {
        return;
    };

    let game_state = GameState {
        agent: collect_agent_state(agent, &[]),
        total_flags: flags.iter().count() as u32,
        collected_flags: scores.0,
        world_width: 100.0,
        world_height: 100.0,
    };

    match bridge.agent_bridge.tx_state.try_send(game_state.clone()) {
        Ok(_) => {}
        Err(TrySendError::Full(_)) => { /* worker still busy; skip this one */ }
        Err(TrySendError::Disconnected(_)) => {
            /* Agent worker has died */
            return;
        }
    }

    if let Some(test) = &bridge.test_bridge {
        match test.tx_state.try_send(game_state) {
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

fn on_test_harness_stop(bridge: Option<Res<Bridge>>, mut exit: MessageWriter<AppExit>) {
    let Some(bridge) = bridge else {
        return;
    };
    if let Some(test) = &bridge.test_bridge {
        if test.rx_stop.try_recv().is_ok() {
            println!("Test harness requested stop; exiting");

            exit.write(AppExit::Success);
        }
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
    agents.iter().find(|(e, _a)| e.index() == id).is_some()
}
