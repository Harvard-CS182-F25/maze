use std::sync::{Arc, RwLock};

use bevy::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

use crate::python::game_state::EntityType;

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct HoverCell {
    pub cell: Option<UVec2>, // (col, row)
    pub world_hit: Option<Vec3>,
}

#[derive(Component)]
pub struct HoverBox<T> {
    pub(super) _marker: std::marker::PhantomData<T>,
}

#[derive(Component)]
pub struct HoverBoxText;

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct OccupancyGridEntry {
    pub assignment: Option<EntityType>,
    pub logit_free: f32,
    pub logit_wall: f32,
    pub logit_flag: f32,
    pub logit_capture_point: f32,
}

pub const LOGIT_CLAMP: f32 = 6.0;

impl Default for OccupancyGridEntry {
    fn default() -> Self {
        Self {
            assignment: None,
            logit_free: 0.0,
            logit_wall: 0.0,
            logit_flag: 0.0,
            logit_capture_point: 0.0,
        }
    }
}

impl OccupancyGridEntry {
    pub fn probabilities(&self) -> (f32, f32, f32, f32) {
        let exp_free = self.logit_free.exp();
        let exp_wall = self.logit_wall.exp();
        let exp_flag = self.logit_flag.exp();
        let exp_capture_point = self.logit_capture_point.exp();

        let sum = exp_free + exp_wall + exp_flag + exp_capture_point;

        (
            exp_free / sum,
            exp_wall / sum,
            exp_flag / sum,
            exp_capture_point / sum,
        )
    }
}

impl std::fmt::Display for OccupancyGridEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (p_free, p_wall, p_flag, p_capture_point) = self.probabilities();

        write!(
            f,
            "OccupancyGridEntry(assignment: {:?}, p_free: {:.2}, p_wall: {:.2}, p_flag: {:.2}, p_capture_point: {:.2})",
            self.assignment, p_free, p_wall, p_flag, p_capture_point
        )
    }
}

#[gen_stub_pyclass]
#[pyclass(name = "OccupancyGridEntry", str)]
/// Represents a mutable occupancy-grid cell.
pub struct OccupancyCellView {
    grid: Py<OccupancyGrid>,
    index: usize,
}

#[gen_stub_pymethods]
#[pymethods]
impl OccupancyCellView {
    /// Mutable cell type assignment. Must be updated manually.
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

    /// Mutable logit for free space.
    #[getter]
    pub fn logit_free(&self, py: Python) -> PyResult<f32> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.logit_free)
    }

    #[setter]
    pub fn set_logit_free(&self, value: f32) -> PyResult<()> {
        Python::attach(|py| {
            let mut grid = self.grid.borrow_mut(py);
            let entry = &mut grid.grid[self.index];
            entry.logit_free = value.clamp(-LOGIT_CLAMP, LOGIT_CLAMP);
            Ok(())
        })
    }

    /// Mutable logit for a wall.
    #[getter]
    pub fn logit_wall(&self, py: Python) -> PyResult<f32> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.logit_wall)
    }

    #[setter]
    pub fn set_logit_wall(&self, value: f32) -> PyResult<()> {
        Python::attach(|py| {
            let mut grid = self.grid.borrow_mut(py);
            let entry = &mut grid.grid[self.index];
            entry.logit_wall = value.clamp(-LOGIT_CLAMP, LOGIT_CLAMP);
            Ok(())
        })
    }

    /// Mutable logit for a flag.
    #[getter]
    pub fn logit_flag(&self, py: Python) -> PyResult<f32> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.logit_flag)
    }

    #[setter]
    pub fn set_logit_flag(&self, value: f32) -> PyResult<()> {
        Python::attach(|py| {
            let mut grid = self.grid.borrow_mut(py);
            let entry = &mut grid.grid[self.index];
            entry.logit_flag = value.clamp(-LOGIT_CLAMP, LOGIT_CLAMP);
            Ok(())
        })
    }

    /// Mutable logit for a capture point.
    #[getter]
    pub fn logit_capture_point(&self, py: Python) -> PyResult<f32> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.logit_capture_point)
    }

    #[setter]
    pub fn set_logit_capture_point(&self, value: f32) -> PyResult<()> {
        Python::attach(|py| {
            let mut grid = self.grid.borrow_mut(py);
            let entry = &mut grid.grid[self.index];
            entry.logit_capture_point = value.clamp(-LOGIT_CLAMP, LOGIT_CLAMP);
            Ok(())
        })
    }

    /// Returns softmax probabilities `(free, wall, flag, capture_point)`.
    pub fn probabilities(&self, py: Python) -> PyResult<(f32, f32, f32, f32)> {
        let grid = self.grid.borrow(py);
        let entry = &grid.grid[self.index];
        Ok(entry.probabilities())
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
/// Represents a mutable occupancy grid, indexed by `grid[column, row]`.
pub struct OccupancyGrid {
    pub grid: Vec<OccupancyGridEntry>,
    /// Returns the edge length of each cell.
    #[pyo3(get)]
    pub cell_size: f32,

    /// Returns the number of columns.
    #[pyo3(get)]
    pub columns: usize,

    /// Returns the number of rows.
    #[pyo3(get)]
    pub rows: usize,
}

#[gen_stub_pymethods]
#[pymethods]
impl OccupancyGrid {
    #[new]
    pub fn new(columns: usize, rows: usize, cell_size: f32) -> Self {
        Self {
            grid: vec![OccupancyGridEntry::default(); columns * rows],
            cell_size,
            columns,
            rows,
        }
    }

    pub fn __getitem__(
        slf: PyRef<Self>,
        py: Python,
        key: Py<PyAny>,
    ) -> PyResult<OccupancyCellView> {
        let (x, y): (usize, usize) = key.extract(py)?;

        if x >= slf.columns || y >= slf.rows {
            return Err(PyErr::new::<pyo3::exceptions::PyIndexError, _>(
                "Index out of bounds",
            ));
        }

        let index = x + slf.columns * y;
        let grid = slf.into_pyobject(py)?.unbind();

        Ok(OccupancyCellView { grid, index })
    }

    #[getter]
    /// Returns `(columns, rows)`.
    pub fn shape(&self) -> (usize, usize) {
        (self.columns, self.rows)
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
    pub fn cell_size(&self) -> PyResult<f32> {
        Python::attach(|py| {
            let grid = self.inner.read().unwrap();
            let grid_ref = grid.borrow(py);
            Ok(grid_ref.cell_size)
        })
    }

    #[getter]
    pub fn columns(&self) -> PyResult<usize> {
        Python::attach(|py| {
            let grid = self.inner.read().unwrap();
            let grid_ref = grid.borrow(py);
            Ok(grid_ref.columns)
        })
    }

    #[getter]
    pub fn rows(&self) -> PyResult<usize> {
        Python::attach(|py| {
            let grid = self.inner.read().unwrap();
            let grid_ref = grid.borrow(py);
            Ok(grid_ref.rows)
        })
    }

    #[getter]
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
        "PlayerGrid"
    }
}
impl PyGridProvider for TrueGrid {
    fn arc(&self) -> &Arc<RwLock<Py<OccupancyGrid>>> {
        &self.0
    }
    fn name() -> &'static str {
        "TruthGrid"
    }
}
