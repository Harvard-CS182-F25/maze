use std::sync::{Arc, RwLock};

use bevy::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

use crate::python::game_state::EntityType;

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct OccupancyGridEntry {
    pub assignment: Option<EntityType>,
    pub p_free: f32,
    pub p_wall: f32,
    pub p_flag: f32,
    pub p_capture_point: f32,
}

impl Default for OccupancyGridEntry {
    fn default() -> Self {
        Self {
            assignment: None,
            p_free: 0.0,
            p_wall: 0.0,
            p_flag: 0.0,
            p_capture_point: 0.0,
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
            1.0 - (self.p_free + self.p_wall + self.p_flag + self.p_capture_point)
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

    #[setter]
    pub fn set_assignment(&self, value: Option<EntityType>) -> PyResult<()> {
        Python::attach(|py| {
            let mut grid = self.grid.borrow_mut(py);
            let entry = &mut grid.grid[self.index];
            entry.assignment = value;
            Ok(())
        })
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
#[derive(Debug, Clone, Default, Reflect)]
pub struct OccupancyGrid {
    pub grid: Vec<OccupancyGridEntry>,
    #[pyo3(get)]
    pub width: usize,
    #[pyo3(get)]
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

    pub fn shape(&self) -> (usize, usize) {
        (self.height, self.width)
    }
}

#[gen_stub_pyclass]
#[pyclass]
pub struct OccupancyGridView {
    pub inner: Arc<RwLock<Py<OccupancyGrid>>>,
}

#[gen_stub_pymethods]
#[pymethods]
impl OccupancyGridView {
    fn __getitem__(&self, key: Py<PyAny>) -> PyResult<OccupancyCellView> {
        Python::attach(|py| {
            let grid = self.inner.read().unwrap();
            let grid_ref = grid.borrow(py);
            OccupancyGrid::__getitem__(grid_ref, py, key)
        })
    }

    #[getter]
    pub fn width(&self) -> PyResult<usize> {
        Python::attach(|py| {
            let grid = self.inner.read().unwrap();
            let grid_ref = grid.borrow(py);
            Ok(grid_ref.width)
        })
    }

    #[getter]
    pub fn height(&self) -> PyResult<usize> {
        Python::attach(|py| {
            let grid = self.inner.read().unwrap();
            let grid_ref = grid.borrow(py);
            Ok(grid_ref.height)
        })
    }

    pub fn shape(&self) -> PyResult<(usize, usize)> {
        Python::attach(|py| {
            let grid = self.inner.read().unwrap();
            let grid_ref = grid.borrow(py);
            Ok(grid_ref.shape())
        })
    }
}

#[derive(Resource, Clone)]
pub struct PlayerGrid(pub Arc<RwLock<Py<OccupancyGrid>>>);

#[derive(Resource, Clone)]
pub struct TrueGrid(pub Arc<RwLock<Py<OccupancyGrid>>>);

#[derive(Component)]
pub struct GridPlane<T>(pub std::marker::PhantomData<T>);

#[derive(Resource)]
pub struct GridVisualization<T> {
    pub handle: Handle<Image>,
    pub material: Handle<StandardMaterial>,
    pub(super) _marker: std::marker::PhantomData<T>,
}

pub trait PyGridProvider: Resource + Clone + Send + Sync + 'static {
    fn arc(&self) -> &Arc<RwLock<Py<OccupancyGrid>>>;
    fn name() -> &'static str;
}
impl PyGridProvider for PlayerGrid {
    fn arc(&self) -> &Arc<RwLock<Py<OccupancyGrid>>> {
        &self.0
    }
    fn name() -> &'static str {
        "player"
    }
}
impl PyGridProvider for TrueGrid {
    fn arc(&self) -> &Arc<RwLock<Py<OccupancyGrid>>> {
        &self.0
    }
    fn name() -> &'static str {
        "truth"
    }
}
