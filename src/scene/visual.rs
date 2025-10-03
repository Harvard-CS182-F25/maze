use bevy::prelude::*;

#[derive(Resource)]
pub struct WallGraphicsAssets {
    pub material: Handle<StandardMaterial>,
}

impl FromWorld for WallGraphicsAssets {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let material: Handle<StandardMaterial> = materials.add(Color::srgb(0.0, 0.0, 0.0));

        Self { material }
    }
}
