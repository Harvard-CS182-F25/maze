use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use avian3d::prelude::SpatialQuery;
use bevy::prelude::*;
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TrySendError};
use pyo3::prelude::*;

use crate::agent::{GhostAgent, RayCasters};
use crate::character_controller::MaxLinearSpeed;
use crate::flag::{CapturePoint, Flag, FlagCaptureCounts};
use crate::interaction_range::{FlagDropMessage, FlagPickupMessage};
use crate::occupancy_grid::{OccupancyGrid, OccupancyGridView};
use crate::occupancy_grid::{PlayerGrid, TrueGrid};
use crate::python::game_state::{SensorRng, collect_agent_state};
use crate::scene::{EstimatedPositionText, Wall};
use crate::{
    agent::{Action, Agent},
    character_controller::MovementMessage,
    core::MazeConfig,
    python::game_state::GameState,
};

/// How long a lockstep tick waits for the Python policy before giving up on it. Generous, because
/// a student policy doing heavy numpy work on a big occupancy grid can legitimately be slow.
const LOCKSTEP_POLICY_TIMEOUT: Duration = Duration::from_secs(60);

/// Set when the sim has handed a state to the policy and is waiting for the matching action.
/// Only meaningful in lockstep (headless) mode.
#[derive(Resource, Default)]
struct PendingPolicyRequest(bool);

/// The first error the Python policy raised, if it raised one. Held separately from `Bridge` so it
/// outlives `shutdown_workers_on_exit`, which drops the bridge as soon as `AppExit` is written.
#[derive(Resource, Clone, Default)]
pub struct PolicyErrorSlot(pub Arc<Mutex<Option<String>>>);

impl PolicyErrorSlot {
    pub fn get(&self) -> Option<String> {
        self.0.lock().unwrap().clone()
    }

    fn set(&self, message: String) {
        let mut slot = self.0.lock().unwrap();
        if slot.is_none() {
            *slot = Some(message);
        }
    }
}

#[derive(Resource)]
struct Bridge {
    pub agent_bridge: PolicyBridge,
    pub test_bridge: Option<TestHarnessBridge>,
    /// When true the sim blocks until the policy answers, so every tick gets exactly one
    /// `get_action` call. Used by the headless evaluation runner.
    pub lockstep: bool,
}

struct PolicyBridge {
    pub tx_state: Sender<PolicyRequest>,
    pub rx_action: Receiver<Action>,
    pub rx_estimated_position: Receiver<(f32, f32)>,
    /// Shares the `PolicyErrorSlot` resource, so the worker thread can report why it stopped.
    pub error: PolicyErrorSlot,
}

/// One policy tick: the observation, the grid the agent writes into, and the simulated time
/// elapsed since the previous tick.
type PolicyRequest = (GameState, Arc<RwLock<Py<OccupancyGrid>>>, f32);

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
struct PolicyTimer {
    timer: Timer,
    /// Simulated seconds accumulated since the last state we successfully handed to the policy.
    /// This is what gets passed to `get_action` as `dt`, so it tracks pausing and speed scaling.
    elapsed_since_dispatch: f32,
}

pub struct PythonPolicyBridgePlugin {
    pub config: MazeConfig,
    pub agent_policy: Py<PyAny>,
    pub test_harness: Option<TestHarnessBridge>,
}

impl Plugin for PythonPolicyBridgePlugin {
    fn build(&self, app: &mut App) {
        let interval = self.config.agent.policy_interval_secs();
        let lockstep = self.config.headless;

        let error_slot = PolicyErrorSlot::default();
        let agent_bridge = Python::attach(|py| {
            PolicyBridge::start(
                self.agent_policy.clone_ref(py),
                lockstep,
                error_slot.clone(),
            )
            .expect("Failed to start agent policy")
        });
        app.insert_resource(error_slot);

        app.insert_resource(PolicyTimer {
            timer: Timer::from_seconds(interval, TimerMode::Repeating),
            elapsed_since_dispatch: 0.0,
        });

        app.init_resource::<PendingPolicyRequest>();

        app.insert_resource(Bridge {
            agent_bridge,
            test_bridge: self.test_harness.clone(),
            lockstep,
        });

        // `send_game_states` must precede `apply_actions`: in lockstep mode the former hands over a
        // state and the latter blocks waiting for the matching action within the same tick.
        app.add_systems(Update, (send_game_states, apply_actions).chain());
        app.add_systems(Update, on_test_harness_stop);
        app.add_systems(
            Update,
            update_estimated_position_text.run_if(|c: Res<MazeConfig>| !c.headless),
        );

        app.add_systems(Last, shutdown_workers_on_exit);
    }
}

impl PolicyBridge {
    pub fn start(
        policy: Py<PyAny>,
        lockstep: bool,
        error: PolicyErrorSlot,
    ) -> anyhow::Result<Self> {
        // In lockstep there is never more than one outstanding request, and a deeper queue would
        // just let the sim run ahead of the policy.
        let capacity = if lockstep { 1 } else { 60 };

        let (tx_state, rx_state) = crossbeam_channel::bounded::<PolicyRequest>(capacity);
        let (tx_action, rx_action) = crossbeam_channel::bounded::<Action>(capacity);
        let (tx_estimated_position, rx_estimated_position) =
            crossbeam_channel::bounded::<(f32, f32)>(capacity);

        let worker_error = error.clone();

        std::thread::spawn(move || {
            // The `estimated_position` attribute is optional: an agent that does not estimate its own
            // position (a teleop shim, say) simply gets no ghost marker. Warn once rather than
            // every tick.
            let mut warned_missing_estimated_position = false;

            while let Ok((state, grid, dt)) = rx_state.recv() {
                let action_and_position =
                    Python::attach(|py| -> PyResult<(Action, Option<(f32, f32)>)> {
                        let state = Py::new(py, state)?;
                        let grid = Py::new(py, OccupancyGridView { inner: grid })?;
                        let action: Action = policy
                            .call_method(py, "get_action", (state, grid, dt), None)?
                            .extract(py)?;

                        let estimated_position = match policy.getattr(py, "estimated_position") {
                            Ok(position) => position.extract::<(f32, f32)>(py).ok(),
                            Err(_) => None,
                        };

                        Ok((action, estimated_position))
                    });

                match action_and_position {
                    Ok((action, estimated_position)) => {
                        if !warned_missing_estimated_position && estimated_position.is_none() {
                            warned_missing_estimated_position = true;
                            eprintln!(
                                "Policy has no usable `estimated_position` attribute; skipping the estimated-position marker"
                            );
                        }

                        if let Err(TrySendError::Disconnected(_)) = tx_action.try_send(action) {
                            break; // main thread has exited
                        }
                        if let Some(estimated_position) = estimated_position
                            && let Err(TrySendError::Disconnected(_)) =
                                tx_estimated_position.try_send(estimated_position)
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        // Print the full traceback for the human at the terminal, but record the
                        // one-line exception text, which is what ends up in `GameResult`.
                        eprintln!("Error calling policy: {e:#?}");
                        worker_error.set(e.to_string());
                        break; // exit thread on error
                    }
                }
            }
        });

        Ok(PolicyBridge {
            tx_state,
            rx_action,
            rx_estimated_position,
            error,
        })
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn send_game_states(
    time: Res<Time>,
    mut t: ResMut<PolicyTimer>,
    mut pending: ResMut<PendingPolicyRequest>,
    scores: Res<FlagCaptureCounts>,
    config: Res<MazeConfig>,
    mut sensor_rng: ResMut<SensorRng>,
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
    // Accumulate before the early return so `dt` covers every simulated second between two
    // policy ticks, not just the frame the timer happened to fire on.
    t.elapsed_since_dispatch += time.delta_secs();
    let timer_finished = t.timer.tick(time.delta()).just_finished();

    let Some(bridge) = bridge else {
        return;
    };

    // In lockstep the clock advances exactly one policy interval per frame, so every frame is a
    // policy tick. Going through the timer as well would compare an `f32` duration against an
    // `f64`-derived delta and drop every other tick.
    if !bridge.lockstep && !timer_finished {
        return;
    }

    // Bevy's very first update has a zero delta. Dispatching it would hand the policy a `dt` of 0,
    // which is a division-by-zero waiting to happen in an agent that integrates velocity.
    if t.elapsed_since_dispatch <= 0.0 {
        return;
    }

    let (noisy_agent_state, true_agent_state) =
        collect_agent_state(&config, &mut sensor_rng, &spatial_query, agent, &kinds);

    let noisy_state = GameState {
        agent: noisy_agent_state,
        total_flags: flags.iter().count() as u32,
        captured_flags: scores.0,
        world_width: config.maze_generation.world_width,
        world_height: config.maze_generation.world_height,
    };

    let true_state = GameState {
        agent: true_agent_state,
        total_flags: flags.iter().count() as u32,
        captured_flags: scores.0,
        world_width: config.maze_generation.world_width,
        world_height: config.maze_generation.world_height,
    };

    let dt = t.elapsed_since_dispatch;
    let request = (noisy_state, player_grid.0.clone(), dt);

    if bridge.lockstep {
        // Block: the sim must not advance past a tick the policy has not seen.
        //
        // Mark the request pending either way. On success `apply_actions` waits for the answer; on
        // failure the worker has died, and letting `apply_actions` observe the disconnected channel
        // is what shuts the run down — skipping it here would spin forever.
        pending.0 = true;
        if bridge.agent_bridge.tx_state.send(request).is_ok() {
            t.elapsed_since_dispatch = 0.0;
        }
    } else {
        match bridge.agent_bridge.tx_state.try_send(request) {
            Ok(_) => t.elapsed_since_dispatch = 0.0,
            // Worker still busy; keep accumulating so the next tick it does see reports the
            // full elapsed time rather than a single interval.
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => return,
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

#[allow(clippy::too_many_arguments)]
fn apply_actions(
    bridge: Option<Res<Bridge>>,
    config: Res<MazeConfig>,
    mut pending: ResMut<PendingPolicyRequest>,
    agents: Query<(Entity, &Agent)>,
    mut movement_event_writer: MessageWriter<MovementMessage>,
    mut pickup_event_writer: MessageWriter<FlagPickupMessage>,
    mut drop_event_writer: MessageWriter<FlagDropMessage>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(bridge) = bridge else {
        return;
    };

    let action = if bridge.lockstep {
        if !pending.0 {
            return;
        }
        pending.0 = false;

        match bridge
            .agent_bridge
            .rx_action
            .recv_timeout(LOCKSTEP_POLICY_TIMEOUT)
        {
            Ok(action) => Some(action),
            Err(RecvTimeoutError::Timeout) => {
                eprintln!(
                    "Policy did not respond within {}s; stopping",
                    LOCKSTEP_POLICY_TIMEOUT.as_secs()
                );
                bridge.agent_bridge.error.set(format!(
                    "policy did not respond within {}s",
                    LOCKSTEP_POLICY_TIMEOUT.as_secs()
                ));
                exit.write(AppExit::Success);
                return;
            }
            Err(RecvTimeoutError::Disconnected) => {
                // The worker stopped, which in practice means the policy raised. The error is
                // already recorded in `bridge.agent_bridge.error`.
                exit.write(AppExit::Success);
                return;
            }
        }
    } else {
        let mut latest: Option<Action> = None;
        while let Ok(action) = bridge.agent_bridge.rx_action.try_recv() {
            latest = Some(action);
        }
        latest
    };

    let Some(action) = action else {
        return;
    };

    // Teleop owns movement and flag interactions. The policy still receives
    // observations and updates its map, but cannot issue competing commands.
    if config.teleop {
        return;
    }

    match action {
        Action::Move { agent_id, velocity } => {
            if !check_agent_exists(agent_id, agents) {
                return;
            }
            movement_event_writer.write(MovementMessage::TranslateById(agent_id, velocity.into()));
        }
        Action::PickupFlag { agent_id } => {
            if !check_agent_exists(agent_id, agents) {
                return;
            }
            pickup_event_writer.write(FlagPickupMessage { agent_id });
        }
        Action::DropFlag { agent_id } => {
            if !check_agent_exists(agent_id, agents) {
                return;
            }
            drop_event_writer.write(FlagDropMessage { agent_id });
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
    while let Ok(position) = bridge.agent_bridge.rx_estimated_position.try_recv() {
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

fn check_agent_exists(agent_id: u32, agents: Query<(Entity, &Agent)>) -> bool {
    agents.iter().any(|(e, _a)| e.index() == agent_id)
}
