//! Metric collection for headless evaluation runs.
//!
//! Everything here is measured in *simulated* seconds (`Time::elapsed_secs`), not wall-clock, so a
//! run that finishes in two seconds of real time still reports the 300 simulated seconds the
//! assignment budgets.

use crossbeam_channel::Sender;

use bevy::prelude::*;
use pyo3::prelude::*;
use pyo3_stub_gen::derive::{gen_stub_pyclass, gen_stub_pymethods};

use crate::core::MazeConfig;
use crate::flag::{Flag, FlagCaptureCounts};
use crate::occupancy_grid::{PlayerGrid, TrueGrid};
use crate::python::policy::PolicyErrorSlot;
use crate::scene::mapping_error;

/// The outcome of a headless run.
#[gen_stub_pyclass]
#[pyclass(name = "GameResult", frozen, str)]
#[derive(Clone, Debug, Default)]
pub struct GameResult {
    /// Simulated seconds elapsed when the run stopped.
    #[pyo3(get)]
    pub elapsed_seconds: f32,

    /// True if the run stopped because it hit the time limit rather than finishing early.
    #[pyo3(get)]
    pub timed_out: bool,

    /// Fraction of ground-truth cells the agent's map got wrong, in `[0, 1]`.
    #[pyo3(get)]
    pub final_mapping_error: f32,

    /// `(wrong, total)` cell counts behind `final_mapping_error`.
    #[pyo3(get)]
    pub mapping_error_cells: (u32, u32),

    /// For each requested threshold, the first simulated time the mapping error dropped to or
    /// below it, or `None` if it never did. In the order the thresholds were given.
    #[pyo3(get)]
    pub milestones: Vec<(f32, Option<f32>)>,

    /// Simulated time of each flag capture, in order.
    #[pyo3(get)]
    pub flag_capture_times: Vec<f32>,

    #[pyo3(get)]
    pub flags_captured: u32,

    #[pyo3(get)]
    pub total_flags: u32,

    /// The maze seed actually used. Worth recording when the config left `seed` unset, since it is
    /// what makes a run reproducible.
    #[pyo3(get)]
    pub seed: u32,

    /// Set if the Python policy raised or stopped responding, in which case the run ended early
    /// and the other metrics describe only the part that ran.
    #[pyo3(get)]
    pub policy_error: Option<String>,
}

#[gen_stub_pymethods]
#[pymethods]
impl GameResult {
    /// The first simulated time the mapping error reached `threshold`, or `None` if it never did.
    /// Only thresholds that were requested for the run are known.
    pub fn time_to_mapping_error(&self, threshold: f32) -> Option<f32> {
        self.milestones
            .iter()
            .find(|(t, _)| (*t - threshold).abs() < f32::EPSILON)
            .and_then(|(_, time)| *time)
    }

    fn __repr__(&self) -> String {
        self.to_string()
    }
}

impl std::fmt::Display for GameResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (wrong, total) = self.mapping_error_cells;
        writeln!(f, "GameResult (seed {})", self.seed)?;
        writeln!(
            f,
            "  ran for            {:.1}s{}",
            self.elapsed_seconds,
            if self.timed_out { " (timed out)" } else { "" }
        )?;
        writeln!(
            f,
            "  mapping error      {:.1}% ({}/{} cells)",
            self.final_mapping_error * 100.0,
            wrong,
            total
        )?;

        for (threshold, time) in &self.milestones {
            match time {
                Some(time) => writeln!(
                    f,
                    "    <= {:>5.1}%       at {:.1}s",
                    threshold * 100.0,
                    time
                )?,
                None => writeln!(f, "    <= {:>5.1}%       never reached", threshold * 100.0)?,
            }
        }

        writeln!(
            f,
            "  flags captured     {}/{}",
            self.flags_captured, self.total_flags
        )?;
        if !self.flag_capture_times.is_empty() {
            let times = self
                .flag_capture_times
                .iter()
                .map(|t| format!("{t:.1}s"))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(f, "    captured at      {times}")?;
        }

        if let Some(error) = &self.policy_error {
            writeln!(f, "  POLICY ERROR       {error}")?;
        }

        Ok(())
    }
}

/// Configuration for a headless evaluation run.
#[derive(Resource, Clone)]
pub struct MetricsConfig {
    /// Simulated seconds to run for.
    pub max_seconds: f32,
    /// Mapping-error thresholds to time, as fractions in `[0, 1]`.
    pub milestones: Vec<f32>,
    /// Stop as soon as every flag has been captured, rather than using the full time budget.
    pub stop_on_all_flags_captured: bool,
    pub result_sender: Sender<GameResult>,
}

/// Metrics accumulated so far.
#[derive(Resource, Default)]
pub struct MetricsState {
    milestone_times: Vec<Option<f32>>,
    flag_capture_times: Vec<f32>,
    last_capture_count: u32,
    mapping_error_cells: (u32, u32),
    final_mapping_error: f32,
    elapsed_seconds: f32,
    timed_out: bool,
    seed: u32,
    total_flags: u32,
    /// Guards against sending the result twice, since `AppExit` can be observed on more than one
    /// frame before the loop actually stops.
    reported: bool,
}

pub struct MetricsPlugin;

impl Plugin for MetricsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MetricsState>();
        app.add_systems(Startup, init_metrics);
        app.add_systems(Update, (record_metrics, check_stop_conditions).chain());
        // Must run before `shutdown_workers_on_exit`, which is also in `Last`.
        app.add_systems(Last, report_result);
    }
}

fn init_metrics(
    mut state: ResMut<MetricsState>,
    metrics_config: Res<MetricsConfig>,
    config: Res<MazeConfig>,
) {
    state.milestone_times = vec![None; metrics_config.milestones.len()];
    state.seed = config.maze_generation.seed.unwrap_or(0);
}

fn record_metrics(
    time: Res<Time>,
    mut state: ResMut<MetricsState>,
    metrics_config: Res<MetricsConfig>,
    player_grid: Res<PlayerGrid>,
    true_grid: Res<TrueGrid>,
    captures: Res<FlagCaptureCounts>,
    flags: Query<&Flag>,
) {
    let now = time.elapsed_secs();
    state.elapsed_seconds = now;
    // Counted here rather than at startup so the flag entities are guaranteed to exist.
    state.total_flags = flags.iter().count() as u32;

    let (wrong, total) = mapping_error(&player_grid, &true_grid);
    let error = (wrong as f32) / total.max(1) as f32;
    state.mapping_error_cells = (wrong, total);
    state.final_mapping_error = error;

    for (index, threshold) in metrics_config.milestones.iter().enumerate() {
        if state.milestone_times[index].is_none() && error <= *threshold {
            state.milestone_times[index] = Some(now);
        }
    }

    while state.last_capture_count < captures.0 {
        state.last_capture_count += 1;
        state.flag_capture_times.push(now);
    }
}

fn check_stop_conditions(
    time: Res<Time>,
    mut state: ResMut<MetricsState>,
    metrics_config: Res<MetricsConfig>,
    mut exit: MessageWriter<AppExit>,
) {
    if time.elapsed_secs() >= metrics_config.max_seconds {
        state.timed_out = true;
        exit.write(AppExit::Success);
        return;
    }

    if metrics_config.stop_on_all_flags_captured
        && state.total_flags > 0
        && state.last_capture_count >= state.total_flags
    {
        exit.write(AppExit::Success);
    }
}

fn report_result(
    mut exit_ev: MessageReader<AppExit>,
    mut state: ResMut<MetricsState>,
    metrics_config: Res<MetricsConfig>,
    policy_error: Option<Res<PolicyErrorSlot>>,
) {
    if exit_ev.read().next().is_none() || state.reported {
        return;
    }
    state.reported = true;

    let result = GameResult {
        elapsed_seconds: state.elapsed_seconds,
        timed_out: state.timed_out,
        final_mapping_error: state.final_mapping_error,
        mapping_error_cells: state.mapping_error_cells,
        milestones: metrics_config
            .milestones
            .iter()
            .copied()
            .zip(state.milestone_times.iter().copied())
            .collect(),
        flag_capture_times: state.flag_capture_times.clone(),
        flags_captured: state.last_capture_count,
        total_flags: state.total_flags,
        seed: state.seed,
        policy_error: policy_error.and_then(|slot| slot.get()),
    };

    let _ = metrics_config.result_sender.send(result);
}
