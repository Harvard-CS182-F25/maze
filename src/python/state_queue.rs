use crossbeam_channel::{Receiver, RecvTimeoutError};
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::python::{
    game_state::GameState,
    occupancy_grid::{OccupancyGrid, OccupancyGridView},
};

#[gen_stub_pyclass]
#[pyclass]
#[allow(clippy::type_complexity)]
pub struct StateQueue {
    pub rx_state: Receiver<(
        GameState,
        Arc<RwLock<Py<OccupancyGrid>>>,
        Arc<RwLock<Py<OccupancyGrid>>>,
    )>,
    pub tx_stop: crossbeam_channel::Sender<()>,
    pub join: Option<std::thread::JoinHandle<()>>,
    pub rate_hz: f32,
}

#[gen_stub_pymethods]
#[pymethods]
impl StateQueue {
    #[getter]
    fn rate_hz(&self) -> f32 {
        self.rate_hz
    }

    /// Wait for next GameState (timeout ms optional). Returns None on timeout.
    #[allow(clippy::type_complexity)]
    fn get<'py>(
        &self,
        py: Python<'py>,
        timeout_ms: Option<u64>,
    ) -> PyResult<Option<(Py<GameState>, Py<OccupancyGridView>, Py<OccupancyGridView>)>> {
        py.detach(|| match timeout_ms {
            Some(ms) => match self.rx_state.recv_timeout(Duration::from_millis(ms)) {
                Ok((mut state, mut true_grid, mut player_grid)) => {
                    while let Ok(next) = self.rx_state.try_recv() {
                        (state, true_grid, player_grid) = next;
                    }
                    let state = Python::attach(|py| Py::new(py, state))?;
                    let true_grid =
                        Python::attach(|py| Py::new(py, OccupancyGridView { inner: true_grid }))?;
                    let player_grid =
                        Python::attach(|py| Py::new(py, OccupancyGridView { inner: player_grid }))?;
                    Ok(Some((state, true_grid, player_grid)))
                }
                Err(RecvTimeoutError::Timeout) => Ok(None),
                Err(RecvTimeoutError::Disconnected) => Ok(None),
            },
            None => match self.rx_state.recv() {
                Ok((mut state, mut true_grid, mut player_grid)) => {
                    while let Ok(next) = self.rx_state.try_recv() {
                        (state, true_grid, player_grid) = next;
                    }
                    let state = Python::attach(|py| Py::new(py, state))?;
                    let true_grid =
                        Python::attach(|py| Py::new(py, OccupancyGridView { inner: true_grid }))?;
                    let player_grid =
                        Python::attach(|py| Py::new(py, OccupancyGridView { inner: player_grid }))?;
                    Ok(Some((state, true_grid, player_grid)))
                }
                Err(_) => Ok(None),
            },
        })
    }

    /// Ask the sim to stop.
    fn stop(&self) {
        let _ = self.tx_stop.send(());
    }

    /// Join the sim thread.
    fn join(&mut self) {
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for StateQueue {
    fn drop(&mut self) {
        let _ = self.tx_stop.send(());
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
