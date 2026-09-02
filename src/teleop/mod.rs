//! Keyboard control of the agent, implemented natively so it needs no OS-level input permissions
//! and only reacts while the game window has focus.
//!
//! `Arrows` or `WASD` move; `Space` picks up and drops flags.
//!
//! The Python policy still runs every tick while teleop is enabled — a mapping agent keeps
//! building its occupancy grid while a human drives — but `apply_actions` drops the `Action::Move`
//! it returns so the two never fight over the agent's velocity.

use bevy::prelude::*;

use crate::{
    agent::Agent,
    character_controller::{MaxLinearSpeed, MovementMessage},
    core::MazeConfig,
    flag::Flag,
    interaction_range::{FlagDropMessage, FlagPickupMessage},
};

pub struct TeleopPlugin;

impl Plugin for TeleopPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            teleop_input.run_if(|config: Res<MazeConfig>| config.teleop && !config.headless),
        );
    }
}

fn teleop_input(
    keys: Res<ButtonInput<KeyCode>>,
    agents: Query<(Entity, &MaxLinearSpeed, Option<&Children>), With<Agent>>,
    flags: Query<&Flag>,
    mut movement: MessageWriter<MovementMessage>,
    mut pickup: MessageWriter<FlagPickupMessage>,
    mut drop: MessageWriter<FlagDropMessage>,
) {
    let Ok((entity, max_speed, children)) = agents.single() else {
        return;
    };
    let id = entity.index();

    // Camera panning uses Shift+drag, so both conventional movement key sets are available.
    let right = keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight);
    let left = keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft);
    let up = keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp);
    let down = keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown);

    // +z points down the screen, so "up" on the keyboard is -z.
    let direction = Vec2::new(
        (right as i32 - left as i32) as f32,
        (down as i32 - up as i32) as f32,
    );

    // Unlike a turn-based game, "no key held" has to mean a concrete zero velocity — otherwise the
    // agent coasts on whatever it was last given.
    let velocity = direction.normalize_or_zero() * max_speed.0;
    movement.write(MovementMessage::TranslateById(id, velocity));

    if keys.just_pressed(KeyCode::Space) {
        let carrying_flag =
            children.is_some_and(|children| children.iter().any(|child| flags.get(child).is_ok()));

        if carrying_flag {
            drop.write(FlagDropMessage { agent_id: id });
        } else {
            pickup.write(FlagPickupMessage { agent_id: id });
        }
    }
}
