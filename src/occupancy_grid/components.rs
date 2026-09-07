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
pub struct OccupancyGridCellData {
    pub assignment: Option<EntityType>,
    pub logit_free: f32,
    pub logit_wall: f32,
    pub logit_flag: f32,
    pub logit_capture_point: f32,
}

pub const LOGIT_CLAMP: f32 = 6.0;

impl Default for OccupancyGridCellData {
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

impl OccupancyGridCellData {
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

impl std::fmt::Display for OccupancyGridCellData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (p_free, p_wall, p_flag, p_capture_point) = self.probabilities();

        write!(
            f,
            "OccupancyGridCell(assignment: {:?}, p_free: {:.2}, p_wall: {:.2}, p_flag: {:.2}, p_capture_point: {:.2})",
            self.assignment, p_free, p_wall, p_flag, p_capture_point
        )
    }
}

#[gen_stub_pyclass]
#[pyclass(name = "OccupancyGridCell", str)]
/// Represents a mutable occupancy-grid cell.
pub struct OccupancyGridCellView {
    grid: Py<OccupancyGrid>,
    index: usize,
}

#[gen_stub_pymethods]
#[pymethods]
impl OccupancyGridCellView {
    fn __repr__(&self) -> String {
        self.to_string()
    }

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

    /// Mutable logit for free space. Clamps to `±6`.
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

    /// Mutable logit for a wall. Clamps to `±6`.
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

    /// Mutable logit for a flag. Clamps to `±6`.
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

    /// Mutable logit for a capture point. Clamps to `±6`.
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

impl std::fmt::Display for OccupancyGridCellView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Python::attach(|py| {
            let grid = self.grid.borrow(py);
            let entry = &grid.grid[self.index];
            write!(f, "{}", entry)
        })
    }
}

/// The grid itself. Never handed to Python directly: policies receive an [`OccupancyGridView`],
/// which is what carries the `OccupancyGrid` name on that side. It stays a `#[pyclass]` only so
/// the view can hold it as a `Py<OccupancyGrid>` and hand out cells that borrow from it.
#[pyclass(name = "OccupancyGridStorage")]
#[derive(Debug, Clone, Default, Reflect)]
pub struct OccupancyGrid {
    pub grid: Vec<OccupancyGridCellData>,
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

#[pymethods]
impl OccupancyGrid {
    #[new]
    pub fn new(columns: usize, rows: usize, cell_size: f32) -> Self {
        Self {
            grid: vec![OccupancyGridCellData::default(); columns * rows],
            cell_size,
            columns,
            rows,
        }
    }

    pub fn __getitem__(
        slf: PyRef<Self>,
        py: Python,
        key: Py<PyAny>,
    ) -> PyResult<OccupancyGridCellView> {
        let (x, y): (usize, usize) = key.extract(py)?;

        if x >= slf.columns || y >= slf.rows {
            return Err(PyErr::new::<pyo3::exceptions::PyIndexError, _>(
                "Index out of bounds",
            ));
        }

        let index = x + slf.columns * y;
        let grid = slf.into_pyobject(py)?.unbind();

        Ok(OccupancyGridCellView { grid, index })
    }

    #[getter]
    /// Returns `(columns, rows)`.
    pub fn shape(&self) -> (usize, usize) {
        (self.columns, self.rows)
    }

    /// Returns the `(column, row)` containing `(x, y)`, or `None` if outside grid.
    pub fn cell_at(&self, x: f32, y: f32) -> Option<(usize, usize)> {
        let (half_width, half_height) = self.half_extent();
        let column = ((x + half_width) / self.cell_size).floor();
        let row = ((y + half_height) / self.cell_size).floor();

        if column < 0.0 || row < 0.0 {
            return None;
        }
        let (column, row) = (column as usize, row as usize);
        (column < self.columns && row < self.rows).then_some((column, row))
    }

    /// Returns the center of cell `(column, row)`, or `None` if outside grid.
    pub fn world_center(&self, column: usize, row: usize) -> Option<(f32, f32)> {
        if column >= self.columns || row >= self.rows {
            return None;
        }

        let (half_width, half_height) = self.half_extent();
        Some((
            column as f32 * self.cell_size + self.cell_size * 0.5 - half_width,
            row as f32 * self.cell_size + self.cell_size * 0.5 - half_height,
        ))
    }
}

impl OccupancyGrid {
    /// Returns the cells overlapped by a world-space XZ AABB, clamped to the grid.
    pub fn overlapping_cells(&self, aabb_min: Vec2, aabb_max: Vec2) -> Vec<(u32, u32)> {
        let (half_width, half_height) = self.half_extent();
        let last_column = self.columns as i32 - 1;
        let last_row = self.rows as i32 - 1;

        let min_column =
            (((aabb_min.x + half_width) / self.cell_size).floor() as i32).clamp(0, last_column);
        let min_row =
            (((aabb_min.y + half_height) / self.cell_size).floor() as i32).clamp(0, last_row);
        let max_column = ((((aabb_max.x + half_width) / self.cell_size).ceil() as i32) - 1)
            .clamp(0, last_column);
        let max_row =
            ((((aabb_max.y + half_height) / self.cell_size).ceil() as i32) - 1).clamp(0, last_row);

        if max_column < min_column || max_row < min_row {
            return Vec::new();
        }

        let mut cells =
            Vec::with_capacity(((max_column - min_column + 1) * (max_row - min_row + 1)) as usize);
        for row in min_row..=max_row {
            for column in min_column..=max_column {
                cells.push((column as u32, row as u32));
            }
        }
        cells
    }

    fn half_extent(&self) -> (f32, f32) {
        (
            self.columns as f32 * self.cell_size * 0.5,
            self.rows as f32 * self.cell_size * 0.5,
        )
    }
}

#[gen_stub_pyclass]
#[pyclass(name = "OccupancyGrid")]
/// Represents a mutable occupancy grid, indexed by `grid[column, row]`.
pub struct OccupancyGridView {
    pub inner: Arc<RwLock<Py<OccupancyGrid>>>,
}

#[gen_stub_pymethods]
#[pymethods]
impl OccupancyGridView {
    fn __getitem__(&self, key: Py<PyAny>) -> PyResult<OccupancyGridCellView> {
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

    /// Returns the `(column, row)` containing `(x, y)`, or `None` if outside grid.
    pub fn cell_at(&self, x: f32, y: f32) -> PyResult<Option<(usize, usize)>> {
        Python::attach(|py| {
            let grid = self.inner.read().unwrap();
            let grid_ref = grid.borrow(py);
            Ok(grid_ref.cell_at(x, y))
        })
    }

    /// Returns the center of cell `(column, row)`, or `None` if outside grid.
    pub fn world_center(&self, column: usize, row: usize) -> PyResult<Option<(f32, f32)>> {
        Python::attach(|py| {
            let grid = self.inner.read().unwrap();
            let grid_ref = grid.borrow(py);
            Ok(grid_ref.world_center(column, row))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::OccupancyGrid;

    #[test]
    fn cells_and_world_points_round_trip() {
        let grid = OccupancyGrid::new(100, 60, 2.0);

        for (column, row) in [(0, 0), (50, 30), (99, 59)] {
            let (x, y) = grid.world_center(column, row).unwrap();
            assert_eq!(grid.cell_at(x, y), Some((column, row)));
        }

        // The grid is centred on the origin, so its corners sit at half the extent.
        assert_eq!(grid.world_center(0, 0), Some((-99.0, -59.0)));
        assert_eq!(grid.cell_at(0.0, 0.0), Some((50, 30)));

        assert_eq!(grid.world_center(100, 0), None);
        assert_eq!(grid.world_center(0, 60), None);
        assert_eq!(grid.cell_at(-100.1, 0.0), None);
        assert_eq!(grid.cell_at(100.1, 0.0), None);
        assert_eq!(grid.cell_at(0.0, 60.1), None);
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
