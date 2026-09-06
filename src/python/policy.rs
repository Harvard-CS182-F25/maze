use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use avian3d::prelude::SpatialQuery;
use bevy::prelude::*;
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TrySendError};
use pyo3::exceptions::PyAttributeError;
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
    core::{MazeConfig, SimulationSets},
    python::game_state::GameState,
};

/// How long a lockstep tick waits for the Python policy before giving up on it. Generous, because
/// a student policy doing heavy numpy work on a big occupancy grid can legitimately be slow.
const LOCKSTEP_POLICY_TIMEOUT: Duration = Duration::from_secs(60);

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
}

struct PolicyBridge {
    pub tx_state: Sender<PolicyRequest>,
    pub rx_action: Receiver<Action>,
    pub rx_estimated_position: Receiver<(f32, f32)>,
    /// Shares the `PolicyErrorSlot` resource, so the worker thread can report why it stopped.
    pub error: PolicyErrorSlot,
}

/// One policy tick: the observation, the grid the agent writes into, and elapsed simulated time.
type PolicyRequest = (GameState, Arc<RwLock<Py<OccupancyGrid>>>, f32);

/// How long the simulation spends waiting on one `get_action` call, smoothed. In lockstep this is
/// the policy's own cost, and it is what decides whether the game can keep up with real time.
#[derive(Resource, Default)]
pub struct PolicyCost {
    pub smoothed_seconds: f32,
}

/// The `estimated_position` a policy reported, if it reported a usable one. Not defining the
/// attribute is allowed — a teleop shim has no position estimate — but defining one that cannot be
/// read is a bug in the policy rather than a choice, so the two are kept apart.
enum EstimatedPosition {
    Reported((f32, f32)),
    NotDefined,
    Unreadable(String),
}

fn read_estimated_position(py: Python<'_>, policy: &Py<PyAny>) -> EstimatedPosition {
    match policy.getattr(py, "estimated_position") {
        Ok(position) => match position.extract::<(f32, f32)>(py) {
            Ok(position) => EstimatedPosition::Reported(position),
            Err(err) => EstimatedPosition::Unreadable(err.to_string()),
        },
        Err(err) if err.is_instance_of::<PyAttributeError>(py) => EstimatedPosition::NotDefined,
        Err(err) => EstimatedPosition::Unreadable(err.to_string()),
    }
}

/// When the policy is next due, in simulated time. The simulation ticks at a fixed rate; the
/// policy runs on whichever of those ticks its own `policy_hz` lands on.
#[derive(Resource)]
struct PolicySchedule {
    interval_secs: f32,
    /// Simulated seconds until the next query, counted down by each fixed tick.
    until_next_query: f32,
    /// Simulated seconds since the last query, which is the `dt` the policy is handed.
    elapsed_since_dispatch: f32,
    /// A request is in flight, so `apply_actions` still owes it a matching receive.
    awaiting_action: bool,
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

pub struct PythonPolicyBridgePlugin {
    pub agent_policy: Py<PyAny>,
    pub policy_hz: Option<f32>,
    pub test_harness: Option<TestHarnessBridge>,
}

impl Plugin for PythonPolicyBridgePlugin {
    fn build(&self, app: &mut App) {
        let Some(policy_hz) = self.policy_hz else {
            // Worth saying out loud: without it, a finished agent whose `get_action` is simply
            // never called looks like a bug in the agent rather than a setting in the config.
            info!("agent.policy_hz is 0, so the policy is disabled and will never be queried");
            return;
        };

        let error_slot = PolicyErrorSlot::default();
        let agent_bridge = Python::attach(|py| {
            PolicyBridge::start(self.agent_policy.clone_ref(py), error_slot.clone())
                .expect("Failed to start agent policy")
        });
        app.insert_resource(error_slot);

        app.insert_resource(Bridge {
            agent_bridge,
            test_bridge: self.test_harness.clone(),
        });
        let interval_secs = policy_hz.recip();
        app.insert_resource(PolicySchedule {
            interval_secs,
            until_next_query: interval_secs,
            elapsed_since_dispatch: 0.0,
            awaiting_action: false,
        });
        app.init_resource::<PolicyCost>();

        // A policy tick observes the world, then applies its matching action.
        app.add_systems(
            FixedUpdate,
            (send_game_states, apply_actions)
                .chain()
                .in_set(SimulationSets::Policy),
        );
        app.add_systems(
            FixedUpdate,
            on_test_harness_stop.before(SimulationSets::Policy),
        );
        app.add_systems(
            Update,
            update_estimated_position_text.run_if(|c: Res<MazeConfig>| !c.headless),
        );

        app.add_systems(Last, shutdown_workers_on_exit);
    }
}

impl PolicyBridge {
    pub fn start(policy: Py<PyAny>, error: PolicyErrorSlot) -> anyhow::Result<Self> {
        let capacity = 1;

        let (tx_state, rx_state) = crossbeam_channel::bounded::<PolicyRequest>(capacity);
        let (tx_action, rx_action) = crossbeam_channel::bounded::<Action>(capacity);
        let (tx_estimated_position, rx_estimated_position) =
            crossbeam_channel::bounded::<(f32, f32)>(capacity);

        let worker_error = error.clone();

        std::thread::spawn(move || {
            let mut announced_missing = false;
            let mut announced_unreadable = false;

            while let Ok((state, grid, dt)) = rx_state.recv() {
                let action_and_position =
                    Python::attach(|py| -> PyResult<(Action, EstimatedPosition)> {
                        let state = Py::new(py, state)?;
                        let grid = Py::new(py, OccupancyGridView { inner: grid })?;
                        let action: Action = policy
                            .call_method(py, "get_action", (state, grid, dt), None)?
                            .extract(py)?;

                        Ok((action, read_estimated_position(py, &policy)))
                    });

                match action_and_position {
                    Ok((action, estimated_position)) => {
                        match &estimated_position {
                            EstimatedPosition::Reported(_) => {}
                            EstimatedPosition::NotDefined if !announced_missing => {
                                announced_missing = true;
                                warn!(
                                    "Policy defines no `estimated_position`; skipping the estimated-position marker"
                                );
                            }
                            EstimatedPosition::Unreadable(why) if !announced_unreadable => {
                                announced_unreadable = true;
                                warn!(
                                    "Policy has an `estimated_position` that could not be read, so the \
                                     estimated-position marker is being skipped: {why}"
                                );
                            }
                            _ => {}
                        }

                        if let Err(TrySendError::Disconnected(_)) = tx_action.try_send(action) {
                            break; // main thread has exited
                        }
                        if let EstimatedPosition::Reported(estimated_position) = estimated_position
                            && let Err(TrySendError::Disconnected(_)) =
                                tx_estimated_position.try_send(estimated_position)
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        // Let Python print the traceback in the format the student already reads;
                        // formatting a `PyErr` from Rust escapes the whole thing onto one line.
                        Python::attach(|py| e.display(py));
                        error!("Policy raised {e}; stopping the run");
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
    time: Res<Time<Fixed>>,
    mut schedule: ResMut<PolicySchedule>,
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
    let fixed_dt = time.delta_secs();
    schedule.elapsed_since_dispatch += fixed_dt;
    schedule.until_next_query -= fixed_dt;
    // The tolerance absorbs float error in the countdown: when `policy_hz` is the simulation rate
    // the remainder lands a few ulps above zero, and a bare `> 0.0` would drop those queries.
    if schedule.until_next_query > f32::EPSILON {
        return;
    }
    // Keep the fractional remainder so rates that do not divide 60 Hz stay on cadence.
    schedule.until_next_query += schedule.interval_secs;

    let Some(bridge) = bridge else {
        return;
    };

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

    let dt = schedule.elapsed_since_dispatch;
    let request = (noisy_state, player_grid.0.clone(), dt);

    // A disconnected worker still needs `apply_actions` to observe the failure and stop the game.
    schedule.awaiting_action = true;
    if bridge.agent_bridge.tx_state.send(request).is_err() {
        return;
    }
    schedule.elapsed_since_dispatch = 0.0;

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
    mut schedule: ResMut<PolicySchedule>,
    mut policy_cost: ResMut<PolicyCost>,
    agents: Query<(Entity, &Agent)>,
    mut movement_event_writer: MessageWriter<MovementMessage>,
    mut pickup_event_writer: MessageWriter<FlagPickupMessage>,
    mut drop_event_writer: MessageWriter<FlagDropMessage>,
    mut exit: MessageWriter<AppExit>,
) {
    if !schedule.awaiting_action {
        return;
    }
    schedule.awaiting_action = false;

    let Some(bridge) = bridge else {
        return;
    };

    let waited_from = std::time::Instant::now();
    let action = match bridge
        .agent_bridge
        .rx_action
        .recv_timeout(LOCKSTEP_POLICY_TIMEOUT)
    {
        Ok(action) => {
            let waited = waited_from.elapsed().as_secs_f32();
            policy_cost.smoothed_seconds += (waited - policy_cost.smoothed_seconds) * 0.1;
            action
        }
        Err(RecvTimeoutError::Timeout) => {
            error!(
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
            exit.write(AppExit::Success);
            return;
        }
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

#[allow(clippy::type_complexity)]
fn update_estimated_position_text(
    bridge: Option<Res<Bridge>>,
    mut ghost_agent: Query<(&mut Transform, &mut Visibility), (With<GhostAgent>, Without<Agent>)>,
    mut query: Query<&mut Text, With<EstimatedPositionText>>,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some((mut ghost_transform, mut ghost_visibility)) = ghost_agent.single_mut().ok() else {
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

    text.0 = format!("Estimated Position: ({x:.2}, {y:.2})");
    ghost_transform.translation = Vec3::new(x, 0.0, y);
    *ghost_visibility = Visibility::Visible;
}

fn on_test_harness_stop(bridge: Option<Res<Bridge>>, mut exit: MessageWriter<AppExit>) {
    let Some(bridge) = bridge else {
        return;
    };
    if let Some(test) = &bridge.test_bridge
        && test.rx_stop.try_recv().is_ok()
    {
        info!("Test harness requested stop; exiting");

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
