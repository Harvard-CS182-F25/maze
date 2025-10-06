use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

use crate::python::game_state::EntityType;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OccupancyGridEntry {
    assignment: Option<EntityType>,
    p_free: f32,
    p_wall: f32,
    p_flag: f32,
    p_capture_point: f32,
    p_unknown: f32,
}

impl Default for OccupancyGridEntry {
    fn default() -> Self {
        Self {
            assignment: None,
            p_free: 0.0,
            p_wall: 0.0,
            p_flag: 0.0,
            p_capture_point: 0.0,
            p_unknown: 1.0,
        }
    }
}

impl std::fmt::Display for OccupancyGridEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "OccupancyGridEntry(assignment: {:?}, p_free: {:.2}, p_wall: {:.2}, p_flag: {:.2}, p_capture_point: {:.2}, p_unknown: {:.2})",
            self.assignment,
            self.p_free,
            self.p_wall,
            self.p_flag,
            self.p_capture_point,
            self.p_unknown
        )
    }
}

#[gen_stub_pyclass]
#[pyclass(name = "OccupancyGridEntry", str)]
pub struct OccupancyCellView {
    grid: Py<OccupancyGrid>,
    index: usize,
}

#[gen_stub_pymethods]
#[pymethods]
impl OccupancyCellView {
    #[getter]
    pub fn assignment(&self, py: Python) -> PyResult<Option<EntityType>> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.assignment)
    }

    #[getter]
    pub fn p_free(&self, py: Python) -> PyResult<f32> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.p_free)
    }

    #[setter]
    pub fn set_p_free(&self, value: f32) -> PyResult<()> {
        Python::attach(|py| {
            let mut grid = self.grid.borrow_mut(py);
            let entry = &mut grid.grid[self.index];
            entry.p_free = value;
            Ok(())
        })
    }

    #[getter]
    pub fn p_wall(&self, py: Python) -> PyResult<f32> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.p_wall)
    }

    #[setter]
    pub fn set_p_wall(&self, value: f32) -> PyResult<()> {
        Python::attach(|py| {
            let mut grid = self.grid.borrow_mut(py);
            let entry = &mut grid.grid[self.index];
            entry.p_wall = value;
            Ok(())
        })
    }

    #[getter]
    pub fn p_flag(&self, py: Python) -> PyResult<f32> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.p_flag)
    }

    #[setter]
    pub fn set_p_flag(&self, value: f32) -> PyResult<()> {
        Python::attach(|py| {
            let mut grid = self.grid.borrow_mut(py);
            let entry = &mut grid.grid[self.index];
            entry.p_flag = value;
            Ok(())
        })
    }

    #[getter]
    pub fn p_capture_point(&self, py: Python) -> PyResult<f32> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.p_capture_point)
    }

    #[setter]
    pub fn set_p_capture_point(&self, value: f32) -> PyResult<()> {
        Python::attach(|py| {
            let mut grid = self.grid.borrow_mut(py);
            let entry = &mut grid.grid[self.index];
            entry.p_capture_point = value;
            Ok(())
        })
    }

    #[getter]
    pub fn p_unknown(&self, py: Python) -> PyResult<f32> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.p_unknown)
    }

    #[setter]
    pub fn set_p_unknown(&self, value: f32) -> PyResult<()> {
        Python::attach(|py| {
            let mut grid = self.grid.borrow_mut(py);
            let entry = &mut grid.grid[self.index];
            entry.p_unknown = value;
            Ok(())
        })
    }
}

impl std::fmt::Display for OccupancyCellView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Python::attach(|py| {
            let grid = self.grid.borrow(py);
            let entry = &grid.grid[self.index];
            write!(f, "{}", entry)
        })
    }
}

#[gen_stub_pyclass]
#[pyclass(name = "OccupancyGrid")]
pub struct OccupancyGrid {
    pub grid: Vec<OccupancyGridEntry>,
    pub width: usize,
    pub height: usize,
}

#[gen_stub_pymethods]
#[pymethods]
impl OccupancyGrid {
    #[new]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            grid: vec![OccupancyGridEntry::default(); width * height],
            width,
            height,
        }
    }

    pub fn __getitem__(
        slf: PyRef<Self>,
        py: Python,
        key: Py<PyAny>,
    ) -> PyResult<OccupancyCellView> {
        let (x, y): (usize, usize) = key.extract(py)?;

        if x >= slf.width || y >= slf.height {
            return Err(PyErr::new::<pyo3::exceptions::PyIndexError, _>(
                "Index out of bounds",
            ));
        }

        let index = x + slf.width * y;
        let grid = slf.into_pyobject(py)?.unbind();

        Ok(OccupancyCellView { grid, index })
    }
}
