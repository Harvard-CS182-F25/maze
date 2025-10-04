mod components;
mod messages;
mod systems;
mod visual;

use bevy::prelude::*;

use crate::core::MazeConfig;

pub use crate::interaction_range::components::*;
pub use crate::interaction_range::messages::*;

pub struct InteractionRangePlugin;
impl Plugin for InteractionRangePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<messages::FlagPickupMessage>();
        app.add_message::<messages::FlagDropMessage>();

        app.add_systems(
            PreStartup,
            init_ring_assets.run_if(|c: Res<MazeConfig>| !c.headless),
        );
        app.add_systems(
            Update,
            (
                systems::update_ring_scale_on_radius_change,
                systems::attach_interaction_range,
                systems::remove_ring_on_radius_removal,
            )
                .run_if(|c: Res<MazeConfig>| !c.headless),
        );

        app.add_systems(
            Update,
            (
                systems::handle_flag_pickups,
                systems::handle_flag_drop,
                systems::handle_flag_capture,
            )
                .chain(),
        );
    }
}

fn init_ring_assets(mut commands: Commands) {
    commands.init_resource::<visual::RingAssets>();
}
