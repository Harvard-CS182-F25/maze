mod agent;
mod camera;
mod character_controller;
mod core;
mod debug;
mod flag;
mod interaction_range;
mod python;
mod scene;

use avian3d::prelude::*;
use bevy::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3_stub_gen::{define_stub_info_gatherer, derive::gen_stub_pyfunction};

use crate::core::MazeConfig;
use crate::python::policy::PythonPolicyBridgePlugin;

#[gen_stub_pyfunction]
#[pyfunction(name = "parse_config")]
fn parse_config(config_path: &str) -> PyResult<MazeConfig> {
    let config_str = std::fs::read_to_string(config_path)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to read config file: {}", e)))?;

    let config: MazeConfig = serde_yaml::from_str(&config_str)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to parse config file: {}", e)))?;

    Ok(config)
}

#[gen_stub_pyfunction]
#[pyfunction(name = "run")]
fn run(py: Python<'_>, config: MazeConfig, policy: Py<PyAny>) -> PyResult<()> {
    Python::detach(py, || {
        let mut app = App::new();
        app.add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Maze".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            PhysicsPlugins::default(),
        ));

        if config.debug {
            app.add_plugins(debug::DebugPlugin);
        }

        app.add_plugins((
            PythonPolicyBridgePlugin {
                config: config.clone(),
                agent_policy: policy,
                test_harness: None,
            },
            core::MazePlugin {
                config: config.clone(),
            },
        ));

        app.run();
    });

    Ok(())
}

#[pymodule]
fn _core(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run, m)?)?;
    m.add_function(wrap_pyfunction!(parse_config, m)?)?;

    m.add_class::<core::MazeConfig>()?;
    m.add_class::<agent::AgentConfig>()?;
    m.add_class::<flag::FlagConfig>()?;
    m.add_class::<flag::CapturePointConfig>()?;
    m.add_class::<camera::CameraConfig>()?;

    m.add_class::<agent::Action>()?;
    m.add_class::<python::game_state::GameState>()?;
    m.add_class::<python::game_state::AgentState>()?;
    m.add_class::<python::game_state::HitInfo>()?;

    Ok(())
}

define_stub_info_gatherer!(stub_info);
