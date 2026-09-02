//!
//! Everything here drives [`Time<Virtual>`] rather than `Time<Physics>`. That is what makes the
//! controls correct rather than merely convenient: avian's physics clock and Bevy's default
//! `Res<Time>` — which gates the policy tick in `send_game_states` — both derive from virtual
//! time, so physics, the rate `get_action` is called at, the `dt` handed to it, and the HUD clock
//! all pause and scale together.

use bevy::prelude::*;

use crate::core::MazeConfig;

#[derive(Resource)]
pub struct PlaybackSpeed {
    index: usize,
}

impl Default for PlaybackSpeed {
    fn default() -> Self {
        Self { index: 3 }
    }
}

impl PlaybackSpeed {
    const MULTIPLIERS: [f32; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 4.0];

    pub fn multiplier(&self) -> f32 {
        Self::MULTIPLIERS[self.index]
    }

    fn slower(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    fn faster(&mut self) {
        self.index = (self.index + 1).min(Self::MULTIPLIERS.len() - 1);
    }
}

#[derive(Component)]
struct PauseIndicatorBadge;

#[derive(Component)]
struct PauseIndicatorText;

#[derive(Component)]
struct PlaybackSpeedText;

pub struct PlaybackPlugin;

impl Plugin for PlaybackPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaybackSpeed>();
        app.add_systems(
            Startup,
            spawn_indicators.run_if(|config: Res<MazeConfig>| !config.headless && !config.teleop),
        );
        app.add_systems(
            Update,
            (
                adjust_speed,
                toggle_pause,
                update_pause_indicator,
                update_speed_indicator,
            )
                .chain()
                .run_if(|config: Res<MazeConfig>| !config.headless && !config.teleop),
        );
    }
}

/// Space pauses/plays automatic simulation. In teleop mode, Space retains
/// its existing pickup/drop meaning.
fn toggle_pause(keys: Res<ButtonInput<KeyCode>>, mut time: ResMut<Time<Virtual>>) {
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }

    if time.is_paused() {
        time.unpause();
    } else {
        time.pause();
    }
}

/// Changes automatic simulation speed.
fn adjust_speed(
    keys: Res<ButtonInput<KeyCode>>,
    mut time: ResMut<Time<Virtual>>,
    mut speed: ResMut<PlaybackSpeed>,
) {
    if keys.just_pressed(KeyCode::BracketLeft) {
        speed.slower();
    } else if keys.just_pressed(KeyCode::BracketRight) {
        speed.faster();
    } else {
        return;
    }

    time.set_relative_speed(speed.multiplier());
}

/// Maze reads pause state from the virtual clock because that is the authoritative
/// clock for both physics and policy evaluation here.
fn spawn_indicators(mut commands: Commands, speed: Res<PlaybackSpeed>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(5.0),
                left: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.1, 0.75, 0.1, 0.85)),
            PauseIndicatorBadge,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("PLAYING"),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                PauseIndicatorText,
            ));
        });

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(36.0),
                left: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.3, 0.3, 0.6, 0.85)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(format!("SPEED {}x", speed.multiplier())),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                PlaybackSpeedText,
            ));
        });
}

fn update_pause_indicator(
    time: Res<Time<Virtual>>,
    mut badges: Query<&mut BackgroundColor, With<PauseIndicatorBadge>>,
    mut texts: Query<&mut Text, With<PauseIndicatorText>>,
    mut previous_state: Local<Option<bool>>,
) {
    let paused = time.is_paused();
    if *previous_state == Some(paused) {
        return;
    }
    *previous_state = Some(paused);

    let (color, label) = if paused {
        (Color::srgba(0.75, 0.1, 0.1, 0.85), "PAUSED")
    } else {
        (Color::srgba(0.1, 0.75, 0.1, 0.85), "PLAYING")
    };

    for mut bg in &mut badges {
        *bg = BackgroundColor(color);
    }
    for mut text in &mut texts {
        text.0 = label.to_string();
    }
}

fn update_speed_indicator(
    speed: Res<PlaybackSpeed>,
    mut texts: Query<&mut Text, With<PlaybackSpeedText>>,
) {
    if !speed.is_changed() {
        return;
    }

    for mut text in &mut texts {
        text.0 = format!("SPEED {}x", speed.multiplier());
    }
}
