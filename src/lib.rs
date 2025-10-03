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
use pyo3::{prelude::*, types::PyDict};
use pyo3_stub_gen::{define_stub_info_gatherer, derive::gen_stub_pyfunction};
use pythonize::depythonize;

use crate::core::MazeConfig;
use crate::python::policy::PythonPolicyBridgePlugin;

#[gen_stub_pyfunction]
#[pyfunction(name = "run")]
fn run(py: Python<'_>, config: Py<PyDict>, policy: Py<PyAny>) -> PyResult<()> {
    let config: MazeConfig = depythonize(config.bind(py))?;

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
    m.add_class::<agent::Action>()?;
    m.add_class::<python::game_state::GameState>()?;
    m.add_class::<python::game_state::AgentState>()?;
    m.add_class::<python::game_state::HitInfo>()?;

    Ok(())
}

define_stub_info_gatherer!(stub_info);
