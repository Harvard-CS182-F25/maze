use bevy::prelude::*;

use crate::core::SimulationSets;

mod components;
mod messages;
mod systems;

pub use components::*;
#[allow(unused_imports)]
pub use messages::*;

pub struct CharacterControllerPlugin;
impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<messages::MovementMessage>().add_systems(
            FixedUpdate,
            (systems::update_grounded, systems::movement).in_set(SimulationSets::Controller),
        );
    }
}
