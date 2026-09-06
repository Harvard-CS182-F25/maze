use bevy::prelude::*;

#[derive(Resource)]
pub struct FlagGraphicsAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

impl FromWorld for FlagGraphicsAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let mesh = meshes.add(Torus::new(0.0, 1.0));

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let material = materials.add(Color::srgb(0.6, 0.4, 0.1));

        Self { mesh, material }
    }
}

#[derive(Resource)]
pub struct CapturePointGraphicsAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

impl FromWorld for CapturePointGraphicsAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let mesh = meshes.add(Torus::new(0.66, 1.33));

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let material = materials.add(Color::srgb(0.85, 0.75, 0.05));

        Self { mesh, material }
    }
}
