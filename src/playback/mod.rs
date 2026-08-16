//! Pause and speed control for the windowed game.
//!
//! Everything here drives [`Time<Virtual>`] rather than `Time<Physics>`. That is what makes the
//! controls correct rather than merely convenient: avian's physics clock and Bevy's default
//! `Res<Time>` — which gates the policy tick in `send_game_states` — both derive from virtual
//! time, so physics, the rate `get_action` is called at, the `dt` handed to it, and the HUD clock
//! all pause and scale together.

use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;

use crate::core::MazeConfig;

/// Speeds cycled through by [`SPEED_KEY`], in order.
const SPEEDS: [f32; 3] = [1.0, 2.0, 4.0];

const PAUSE_KEY: KeyCode = KeyCode::KeyP;
/// `Space` is deliberately left to teleop for flag pickup/drop.
const SPEED_KEY: KeyCode = KeyCode::Period;

/// Index into [`SPEEDS`] of the current playback speed.
#[derive(Resource, Default)]
pub struct PlaybackSpeed(usize);

impl PlaybackSpeed {
    pub fn multiplier(&self) -> f32 {
        SPEEDS[self.0]
    }
}

pub struct PlaybackPlugin;

impl Plugin for PlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaybackSpeed>();
        app.add_systems(
            Update,
            (
                toggle_pause.run_if(input_just_pressed(PAUSE_KEY)),
                cycle_speed.run_if(input_just_pressed(SPEED_KEY)),
            )
                .run_if(|config: Res<MazeConfig>| !config.headless),
        );
    }
}

fn toggle_pause(mut time: ResMut<Time<Virtual>>) {
    if time.is_paused() {
        time.unpause();
    } else {
        time.pause();
    }
}

fn cycle_speed(mut time: ResMut<Time<Virtual>>, mut speed: ResMut<PlaybackSpeed>) {
    speed.0 = (speed.0 + 1) % SPEEDS.len();
    time.set_relative_speed(speed.multiplier());
}
