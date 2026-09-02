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

use crate::core::MazeConfig;
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

    Ok(config)
}

fn generate_app(
    config: MazeConfig,
    policy: Py<PyAny>,
    test_harness: Option<TestHarnessBridge>,
) -> App {
    let mut app = App::new();

    if config.headless {
        // No window and no GPU: just the scheduler plus the pieces the sim genuinely needs.
        // `TransformPlugin` is not part of `MinimalPlugins`, but avian and the flag-parenting
        // hierarchy both depend on transform propagation.
        app.add_plugins(
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(std::time::Duration::ZERO)),
        );
        app.add_plugins(TransformPlugin);
        // Nothing loads assets headlessly, but avian's collider cache watches `AssetEvent<Mesh>`
        // and a system whose message type was never registered is a hard error, not a skip.
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Mesh>();
        // Avian's collider-constructor systems also want `SceneSpawner`.
        app.add_plugins(bevy::scene::ScenePlugin);
        // Likewise, a run condition that reads a missing `ButtonInput<KeyCode>` aborts the app
        // rather than evaluating false. With no window nothing ever writes input events, so every
        // key-gated system simply never fires.
        app.add_plugins(InputPlugin);

        // Advance the clock by a fixed step per frame instead of tracking wall-clock time. This is
        // what lets a 300 simulated-second run finish in seconds, and what makes two runs with the
        // same seed identical.
        let policy_hz = config.agent.effective_policy_hz();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_secs_f64(1.0 / policy_hz as f64),
        ));
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Maze".to_string(),
                ..Default::default()
            }),
            ..Default::default()
        }));

        app.add_systems(Update, force_focus);
    }

    app.add_plugins((PhysicsPlugins::default(),));

    // The debug plugin pulls in egui and physics debug rendering, neither of which exists headless.
    if config.debug && !config.headless {
        app.add_plugins(debug::DebugPlugin);
    }

    app.add_plugins((
        PythonPolicyBridgePlugin {
            config: config.clone(),
            agent_policy: policy,
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

        let rate_hz = config.agent.effective_policy_hz();
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
/// the assignment's five-minute target regardless of how long the run actually takes. Every
/// simulated tick calls `get_action` exactly once — the sim waits for the policy rather than
/// skipping ahead — so a run is reproducible for a given maze seed.
#[gen_stub_pyfunction]
#[pyfunction(name = "run_headless")]
#[pyo3(signature = (config, policy, max_seconds = 300.0, mapping_error_milestones = vec![0.8, 0.6, 0.4, 0.2], stop_on_all_flags_captured = false))]
fn run_headless(
    py: Python<'_>,
    config: MazeConfig,
    policy: Py<PyAny>,
    max_seconds: f32,
    mapping_error_milestones: Vec<f32>,
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

    let (tx_result, rx_result) = crossbeam_channel::bounded::<GameResult>(1);
    let metrics_config = MetricsConfig {
        max_seconds,
        milestones: mapping_error_milestones,
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
    m.add_class::<python::game_state::HitInfo>()?;
    m.add_class::<python::game_state::EntityType>()?;
    m.add_class::<occupancy_grid::OccupancyGrid>()?;
    m.add_class::<occupancy_grid::OccupancyCellView>()?;
    m.add_class::<python::game_state::SensorConfidence>()?;
    m.add_class::<python::metrics::GameResult>()?;

    Ok(())
}

define_stub_info_gatherer!(stub_info);
