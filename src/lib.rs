mod agent;
mod camera;
mod character_controller;
mod core;
mod debug;
mod flag;
mod interaction_range;
mod occupancy_grid;
mod playback;
mod python;
mod scene;
mod teleop;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use avian3d::prelude::*;
use bevy::app::ScheduleRunnerPlugin;
use bevy::asset::AssetPlugin;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::transform::TransformPlugin;
use bevy::window::WindowCreated;
use bevy::winit::WinitWindows;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3_stub_gen::{define_stub_info_gatherer, derive::gen_stub_pyfunction};

use crate::core::{MazeConfig, SIMULATION_HZ};
use crate::occupancy_grid::OccupancyGrid;
use crate::python::game_state::GameState;
use crate::python::metrics::{GameResult, MetricsConfig, MetricsPlugin};
use crate::python::policy::{PythonPolicyBridgePlugin, TestHarnessBridge};
use crate::python::state_queue::StateQueue;

#[gen_stub_pyfunction]
#[pyfunction(name = "parse_config")]
fn parse_config(config_path: &str) -> PyResult<MazeConfig> {
    let config_str = std::fs::read_to_string(config_path)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to read config file: {}", e)))?;

    let config: MazeConfig = serde_yaml::from_str(&config_str)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to parse config file: {}", e)))?;

    // Only the settings themselves: a config that parses may still be finished off in Python, so
    // whether `teleop` and `headless` agree with `policy_hz` is left for `run`/`run_headless`.
    config.validate_settings().map_err(PyValueError::new_err)?;

    Ok(config)
}

/// Whether some app in this process has already installed a tracing subscriber. Bevy's `LogPlugin`
/// installs a process-global one and logs an alarming error rather than failing when it cannot, so
/// an evaluation harness playing several games back to back would otherwise report a logging
/// failure on every run after the first.
static LOGGER_INSTALLED: AtomicBool = AtomicBool::new(false);

fn generate_app(
    config: MazeConfig,
    policy: Py<PyAny>,
    test_harness: Option<TestHarnessBridge>,
) -> App {
    let mut app = App::new();

    let policy_hz = config.agent.active_policy_hz();

    if config.headless {
        // `MinimalPlugins` carries no `LogPlugin`, so without this every `info!` and `warn!` the
        // engine raises — the maze seed, an unplaceable flag, a capped velocity — is discarded in
        // exactly the mode used for grading.
        if !LOGGER_INSTALLED.swap(true, Ordering::Relaxed) {
            app.add_plugins(bevy::log::LogPlugin::default());
        }

        // No window or GPU, but physics and flag parenting still need transforms and scene assets.
        app.add_plugins(
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(std::time::Duration::ZERO)),
        );
        app.add_plugins(TransformPlugin);
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Mesh>();
        app.add_plugins(bevy::scene::ScenePlugin);
        app.add_plugins(InputPlugin);

        // One app update advances exactly one simulation tick.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / SIMULATION_HZ as f64),
        ));
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Maze".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }));

        // `DefaultPlugins` brings its own `LogPlugin`.
        LOGGER_INSTALLED.store(true, Ordering::Relaxed);

        app.add_systems(Update, force_focus);
    }

    app.insert_resource(Time::<Fixed>::from_hz(SIMULATION_HZ as f64));
    app.add_plugins((PhysicsPlugins::default(),));

    // The debug plugin pulls in egui and physics debug rendering, neither of which exists headless.
    if config.debug && !config.headless {
        app.add_plugins(debug::DebugPlugin);
    }

    app.add_plugins((
        PythonPolicyBridgePlugin {
            agent_policy: policy,
            policy_hz,
            test_harness,
        },
        core::MazePlugin {
            config: config.clone(),
        },
    ));

    app
}

#[gen_stub_pyfunction]
#[pyfunction(name = "run")]
fn run(py: Python<'_>, config: MazeConfig, policy: Py<PyAny>) -> PyResult<Option<StateQueue>> {
    config.validate().map_err(PyValueError::new_err)?;

    if !config.headless {
        Python::detach(py, || {
            let mut app = generate_app(config, policy, None);
            app.run();
        });
        Ok(None)
    } else {
        let (tx_state, rx_state) = crossbeam_channel::bounded::<(
            GameState,
            Arc<RwLock<Py<OccupancyGrid>>>,
            Arc<RwLock<Py<OccupancyGrid>>>,
        )>(60);
        let (tx_stop, rx_stop) = crossbeam_channel::bounded::<()>(1);

        let rate_hz = config
            .agent
            .active_policy_hz()
            .expect("headless runs require an active policy");
        let join = std::thread::spawn(move || {
            let mut app = generate_app(
                config,
                policy,
                Some(TestHarnessBridge { tx_state, rx_stop }),
            );
            app.run();
        });

        Ok(Some(StateQueue {
            rx_state,
            tx_stop,
            rate_hz,
            join: Some(join),
        }))
    }
}

/// Play a whole game with no window, as fast as the policy can be evaluated, and return what
/// happened.
///
/// The clock is simulated: `max_seconds` counts simulated seconds, so a 300-second budget matches
/// the assignment's five-minute target regardless of how long the run actually takes. The
/// simulation waits for each `get_action` to return rather than skipping ahead, so a run is
/// reproducible for a given maze seed.
#[gen_stub_pyfunction]
#[pyfunction(name = "run_headless")]
#[pyo3(signature = (config, policy, max_seconds = 300.0, mapping_accuracy_milestones = vec![0.2, 0.4, 0.6, 0.8], stop_on_all_flags_captured = false))]
fn run_headless(
    py: Python<'_>,
    config: MazeConfig,
    policy: Py<PyAny>,
    max_seconds: f32,
    mapping_accuracy_milestones: Vec<f32>,
    stop_on_all_flags_captured: bool,
) -> PyResult<GameResult> {
    if max_seconds <= 0.0 {
        return Err(PyValueError::new_err("max_seconds must be positive"));
    }
    if config.teleop {
        return Err(PyValueError::new_err(
            "teleop requires a window and cannot be used with run_headless",
        ));
    }

    let mut config = config;
    config.headless = true;
    config.validate().map_err(PyValueError::new_err)?;

    let (tx_result, rx_result) = crossbeam_channel::bounded::<GameResult>(1);
    let metrics_config = MetricsConfig {
        max_seconds,
        mapping_accuracy_milestones,
        stop_on_all_flags_captured,
        result_sender: tx_result,
    };

    // Release the GIL for the whole run: the policy worker thread needs to acquire it on every
    // tick, and in lockstep the sim cannot advance until it does.
    py.detach(move || {
        // The `App` is not `Send`, so it has to be built inside the thread that runs it.
        let join = std::thread::spawn(move || {
            let mut app = generate_app(config, policy, None);
            app.insert_resource(metrics_config);
            app.add_plugins(MetricsPlugin);
            app.run();
        });

        let result = rx_result.recv();
        let _ = join.join();

        result.map_err(|_| {
            PyRuntimeError::new_err("headless run ended without producing a result".to_string())
        })
    })
}

fn force_focus(
    winit_windows: Option<NonSend<WinitWindows>>,
    mut created: MessageReader<WindowCreated>,
) {
    let Some(winit_windows) = winit_windows else {
        return;
    };

    for ev in created.read() {
        if let Some(win) = winit_windows.get_window(ev.window) {
            win.focus_window();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::prelude::*;
    use bevy::time::TimeUpdateStrategy;

    #[derive(Resource, Default)]
    struct TickLog(Vec<f32>);

    fn record_tick(time: Res<Time<Fixed>>, mut ticks: ResMut<TickLog>) {
        ticks.0.push(time.delta_secs());
    }

    fn app_with_frame_duration(frame_duration: Duration) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Time::<Fixed>::from_hz(60.0));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(frame_duration));
        app.init_resource::<TickLog>();
        app.add_systems(FixedUpdate, record_tick);
        app
    }

    #[test]
    fn fixed_ticks_are_independent_of_frame_batching() {
        let tick = Duration::from_secs_f64(1.0 / 60.0);
        let mut normal = app_with_frame_duration(tick);
        let mut batched = app_with_frame_duration(tick * 4);

        for _ in 0..13 {
            normal.update();
        }
        for _ in 0..4 {
            batched.update();
        }

        let normal_ticks = normal.world().resource::<TickLog>();
        let batched_ticks = batched.world().resource::<TickLog>();
        assert_eq!(normal_ticks.0.len(), 12);
        assert_eq!(normal_ticks.0, batched_ticks.0);
        assert!(
            normal_ticks
                .0
                .iter()
                .all(|dt| (*dt - 1.0 / 60.0).abs() < f32::EPSILON)
        );
    }
}

#[pymodule]
fn _core(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run, m)?)?;
    m.add_function(wrap_pyfunction!(run_headless, m)?)?;
    m.add_function(wrap_pyfunction!(parse_config, m)?)?;

    m.add_class::<core::MazeConfig>()?;
    m.add_class::<agent::AgentConfig>()?;
    m.add_class::<flag::FlagConfig>()?;

    m.add_class::<agent::Action>()?;
    m.add_class::<python::game_state::GameState>()?;
    m.add_class::<python::game_state::AgentState>()?;
    m.add_class::<python::game_state::Raycast>()?;
    m.add_class::<python::game_state::EntityType>()?;
    m.add_class::<occupancy_grid::OccupancyGridView>()?;
    m.add_class::<occupancy_grid::OccupancyGridCellView>()?;
    m.add_class::<python::game_state::SensorConfidence>()?;
    m.add_class::<python::metrics::GameResult>()?;

    Ok(())
}

define_stub_info_gatherer!(stub_info);
