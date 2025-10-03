use bevy::prelude::*;

mod components;
mod messages;
mod systems;

pub use components::*;
#[allow(unused_imports)]
pub use messages::*;

pub struct CharacterControllerPlugin;
impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<messages::MovementMessage>()
            .add_systems(Update, (systems::update_grounded, systems::movement));
    }
}
