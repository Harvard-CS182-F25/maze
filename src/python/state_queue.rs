use crossbeam_channel::{Receiver, RecvTimeoutError};
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};
use std::time::Duration;

use crate::python::game_state::GameState;

#[gen_stub_pyclass]
#[pyclass]
pub struct StateQueue {
    pub rx_state: Receiver<GameState>,
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
    fn get<'py>(
        &self,
        py: Python<'py>,
        timeout_ms: Option<u64>,
    ) -> PyResult<Option<Py<GameState>>> {
        py.detach(|| match timeout_ms {
            Some(ms) => match self.rx_state.recv_timeout(Duration::from_millis(ms)) {
                Ok(mut gs) => {
                    while let Ok(next) = self.rx_state.try_recv() {
                        gs = next;
                    }
                    Python::attach(|py| Ok(Some(Py::new(py, gs)?)))
                }
                Err(RecvTimeoutError::Timeout) => Ok(None),
                Err(RecvTimeoutError::Disconnected) => Ok(None),
            },
            None => match self.rx_state.recv() {
                Ok(mut gs) => {
                    while let Ok(next) = self.rx_state.try_recv() {
                        gs = next;
                    }
                    Python::attach(|py| Ok(Some(Py::new(py, gs)?)))
                }
                Err(_) => Ok(None),
            },
        })
    }

    /// Try to get without blocking.
    fn try_get(&self, py: Python) -> PyResult<Option<Py<GameState>>> {
        match self.rx_state.try_recv() {
            Ok(gs) => Ok(Some(Py::new(py, gs)?)),
            Err(_) => Ok(None),
        }
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
